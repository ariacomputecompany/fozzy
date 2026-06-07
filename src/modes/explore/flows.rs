use uuid::Uuid;

use std::time::{Duration, Instant};

use crate::finalize::{write_reporter_artifacts, write_summary_report};
use crate::{
    Config, ExitStatus, Finding, FindingKind, FozzyError, FozzyResult, MemoryRunReport,
    RunIdentity, RunMode, RunSummary, ScenarioPath, TraceFile, heap_budget_findings_from_trace,
    should_emit_profile_artifacts, wall_time_iso_utc, write_memory_artifacts,
    write_profile_artifacts_from_trace_with_source,
};

use super::exec::{run_explore_inner, run_explore_replay_inner};
use super::scenario::{
    apply_checker_override, apply_faults_preset, load_explore_scenario, shrink_trial_duration,
    shrinkable_setup_step,
};
use super::types::{ExploreOptions, ExploreTrace, ScenarioV1Explore};
use super::utils::{
    gen_seed, heap_budget_policy, should_emit_full_profile, should_emit_heavy_artifacts,
};

pub fn explore(
    config: &Config,
    scenario_path: ScenarioPath,
    opt: &ExploreOptions,
) -> FozzyResult<crate::RunResult> {
    let seed = opt.seed.unwrap_or_else(gen_seed);
    let run_id = Uuid::new_v4().to_string();
    let started_at = wall_time_iso_utc();
    let started = Instant::now();

    let artifacts_dir = config.runs_dir().join(&run_id);
    std::fs::create_dir_all(&artifacts_dir)?;

    let mut scenario = load_explore_scenario(&scenario_path, opt.nodes)?;
    apply_faults_preset(&mut scenario, opt.faults.as_deref())?;
    apply_checker_override(&mut scenario, opt.checker.as_deref())?;
    let (status, findings, events, delivered, decisions) =
        run_explore_inner(&scenario, seed, opt.schedule, opt.steps, opt.time)?;
    let _ = delivered;
    let memory_report: Option<MemoryRunReport> = None;

    let finished_at = wall_time_iso_utc();
    let (duration_ms, duration_ns) = crate::duration_fields(started.elapsed());
    let report_path = artifacts_dir.join("report.json");

    let mut summary = RunSummary {
        status,
        mode: RunMode::Explore,
        identity: RunIdentity {
            run_id: run_id.clone(),
            seed,
            trace_path: None,
            report_path: Some(report_path.to_string_lossy().to_string()),
            artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
        },
        started_at,
        finished_at,
        duration_ms,
        duration_ns,
        tests: None,
        memory: memory_report.as_ref().map(|m| m.summary.clone()),
        findings: findings.clone(),
    };
    let mut profile_trace = TraceFile::new_explore(
        ExploreTrace {
            scenario_path: scenario_path.as_path().to_string_lossy().to_string(),
            scenario: scenario.clone(),
            schedule: opt.schedule,
        },
        decisions.clone(),
        events.clone(),
        summary.clone(),
    );
    profile_trace.memory = memory_report.as_ref().map(|m| m.to_trace());
    let heap_findings =
        heap_budget_findings_from_trace(&profile_trace, &heap_budget_policy(config));
    if !heap_findings.is_empty() {
        summary.findings.extend(heap_findings);
        summary.findings = crate::collapse_findings(summary.findings.clone());
    }

    if matches!(opt.reporter, crate::Reporter::Junit) {
        std::fs::write(
            artifacts_dir.join("junit.xml"),
            crate::render_junit_xml(&summary),
        )?;
    }
    if matches!(opt.reporter, crate::Reporter::Html) {
        std::fs::write(
            artifacts_dir.join("report.html"),
            crate::render_html(&summary),
        )?;
    }

    let should_record = opt.record_trace_to.is_some() || status != ExitStatus::Pass;
    if should_record {
        let requested = opt
            .record_trace_to
            .clone()
            .unwrap_or_else(|| artifacts_dir.join("trace.fozzy"));
        let out = crate::resolve_record_target(&requested, opt.record_collision)?;
        summary.identity.trace_path = Some(out.to_string_lossy().to_string());
        let mut trace = TraceFile::new_explore(
            ExploreTrace {
                scenario_path: scenario_path.as_path().to_string_lossy().to_string(),
                scenario: scenario.clone(),
                schedule: opt.schedule,
            },
            decisions.clone(),
            events.clone(),
            summary.clone(),
        );
        trace.memory = memory_report.as_ref().map(|m| m.to_trace());
        crate::write_trace_to_target(&trace, &out)?;
    }
    let emit_heavy = should_emit_heavy_artifacts(status, should_record)
        || should_emit_full_profile(opt.profile_capture);
    if emit_heavy {
        std::fs::write(
            artifacts_dir.join("events.json"),
            serde_json::to_vec(&events)?,
        )?;
        crate::write_timeline(&events, &artifacts_dir.join("timeline.json"))?;
        if let Some(mem) = memory_report.as_ref()
            && mem.options.artifacts
        {
            write_memory_artifacts(mem, &artifacts_dir)?;
        }
    }
    let mut profile_metadata = None;
    if should_emit_profile_artifacts(opt.profile_capture, status, should_record) {
        profile_trace.summary = summary.clone();
        profile_metadata = Some(write_profile_artifacts_from_trace_with_source(
            &profile_trace,
            summary
                .identity
                .trace_path
                .as_deref()
                .map(std::path::Path::new),
            &artifacts_dir,
        )?);
    }
    write_summary_report(
        &summary,
        &report_path,
        &artifacts_dir,
        profile_metadata.as_ref(),
    )?;

    Ok(crate::RunResult { summary })
}

pub fn replay_explore_trace(
    config: &Config,
    trace: &TraceFile,
    trace_path: &std::path::Path,
    opt: &crate::ReplayOptions,
) -> FozzyResult<crate::RunResult> {
    let Some(explore) = trace.explore.as_ref() else {
        return Err(FozzyError::Trace("not an explore trace".to_string()));
    };
    let seed = trace.summary.identity.seed;
    let run_id = Uuid::new_v4().to_string();
    let started_at = wall_time_iso_utc();
    let started = Instant::now();

    let (status, findings, events, _delivered, _decisions) =
        run_explore_replay_inner(&explore.scenario, seed, explore.schedule, &trace.decisions)?;
    let finished_at = wall_time_iso_utc();
    let (duration_ms, duration_ns) = crate::duration_fields(started.elapsed());
    let memory_report = trace.memory.as_ref().map(|m| MemoryRunReport {
        schema_version: "fozzy.memory_report.v1".to_string(),
        options: m.options.clone(),
        summary: m.summary.clone(),
        leaks: m.leaks.clone(),
        timeline: Vec::new(),
        graph: crate::MemoryGraph::default(),
    });

    let artifacts_dir = config.runs_dir().join(&run_id);
    std::fs::create_dir_all(&artifacts_dir)?;
    let report_path = artifacts_dir.join("report.json");

    let mut findings = findings.clone();
    for warning in crate::trace_schema_warnings(trace.version) {
        findings.push(Finding {
            kind: FindingKind::Checker,
            title: "stale_trace_schema".to_string(),
            message: warning,
            location: None,
        });
    }

    let mut summary = RunSummary {
        status,
        mode: RunMode::Replay,
        identity: RunIdentity {
            run_id,
            seed,
            trace_path: Some(trace_path.to_string_lossy().to_string()),
            report_path: Some(report_path.to_string_lossy().to_string()),
            artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
        },
        started_at,
        finished_at,
        duration_ms,
        duration_ns,
        tests: None,
        memory: trace.memory.as_ref().map(|m| m.summary.clone()),
        findings,
    };
    let mut profile_trace = TraceFile::new_explore(
        explore.clone(),
        trace.decisions.clone(),
        events.clone(),
        summary.clone(),
    );
    profile_trace.memory = memory_report.as_ref().map(|m| m.to_trace());
    let heap_findings =
        heap_budget_findings_from_trace(&profile_trace, &heap_budget_policy(config));
    if !heap_findings.is_empty() {
        summary.findings.extend(heap_findings);
        summary.findings = crate::collapse_findings(summary.findings.clone());
    }

    let explicit_capture = opt.dump_events;
    let emit_heavy = should_emit_heavy_artifacts(status, explicit_capture)
        || should_emit_full_profile(opt.profile_capture);
    if emit_heavy {
        std::fs::write(
            artifacts_dir.join("events.json"),
            serde_json::to_vec(&events)?,
        )?;
        crate::write_timeline(&events, &artifacts_dir.join("timeline.json"))?;
        if let Some(mem) = memory_report.as_ref()
            && mem.options.artifacts
        {
            write_memory_artifacts(mem, &artifacts_dir)?;
        }
    }
    let mut profile_metadata = None;
    if should_emit_profile_artifacts(opt.profile_capture, status, explicit_capture) {
        profile_trace.summary = summary.clone();
        profile_metadata = Some(write_profile_artifacts_from_trace_with_source(
            &profile_trace,
            None,
            &artifacts_dir,
        )?);
    }
    write_reporter_artifacts(&summary, &artifacts_dir, opt.reporter)?;
    write_summary_report(
        &summary,
        &report_path,
        &artifacts_dir,
        profile_metadata.as_ref(),
    )?;

    Ok(crate::RunResult { summary })
}

pub fn shrink_explore_trace(
    _config: &Config,
    trace_path: crate::TracePath,
    opt: &crate::ShrinkOptions,
) -> FozzyResult<crate::ShrinkResult> {
    let trace = TraceFile::read_json(trace_path.as_path())?;
    let target_status = trace.summary.status;
    let Some(explore) = trace.explore.as_ref() else {
        return Err(FozzyError::Trace("not an explore trace".to_string()));
    };
    if opt.minimize != crate::ShrinkMinimize::All && opt.minimize != crate::ShrinkMinimize::Schedule
    {
        return Err(FozzyError::InvalidArgument(
            "explore shrink only supports --minimize schedule|all (v0.2)".to_string(),
        ));
    }

    let seed = trace.summary.identity.seed;
    let mut best_decisions = trace.decisions.clone();
    let mut candidate = best_decisions.clone();
    let budget = opt.budget.unwrap_or(Duration::from_secs(15));
    let deadline = Instant::now() + budget;

    if opt.minimize == crate::ShrinkMinimize::Schedule || opt.minimize == crate::ShrinkMinimize::All
    {
        let mut chunk = candidate.len().max(1).div_ceil(2);
        while chunk > 0 && Instant::now() < deadline && candidate.len() > 1 {
            let mut improved = false;
            let mut i = 0usize;
            while i < candidate.len() && Instant::now() < deadline {
                let mut trial = candidate.clone();
                let end = (i + chunk).min(trial.len());
                trial.drain(i..end);
                if trial.is_empty() {
                    i += chunk;
                    continue;
                }

                let (status, _findings, _events, _delivered, _decisions) =
                    run_explore_replay_inner(&explore.scenario, seed, explore.schedule, &trial)?;
                if crate::shrink_status_matches(target_status, status) {
                    candidate = trial;
                    improved = true;
                    continue;
                }
                i += chunk;
            }

            if !improved {
                if chunk == 1 {
                    break;
                }
                chunk = chunk.div_ceil(2);
            }
        }
        best_decisions = candidate;
    }

    let mut shrunk_scenario = explore.scenario.clone();
    if opt.minimize == crate::ShrinkMinimize::All {
        let mut steps = shrunk_scenario.steps.clone();
        let mut chunk = steps.len().max(1).div_ceil(2);
        while chunk > 0 && Instant::now() < deadline && steps.len() > 1 {
            let mut improved = false;
            let mut i = 0usize;
            while i < steps.len() && Instant::now() < deadline {
                let end = (i + chunk).min(steps.len());
                if !steps[i..end].iter().all(shrinkable_setup_step) {
                    i += chunk;
                    continue;
                }
                let mut trial = steps.clone();
                trial.drain(i..end);
                if trial.is_empty() {
                    i += chunk;
                    continue;
                }
                let trial_scenario = ScenarioV1Explore {
                    version: shrunk_scenario.version,
                    name: shrunk_scenario.name.clone(),
                    nodes: shrunk_scenario.nodes.clone(),
                    steps: trial.clone(),
                    invariants: shrunk_scenario.invariants.clone(),
                };
                let (status, _findings, _events, _delivered, _decisions) = run_explore_inner(
                    &trial_scenario,
                    seed,
                    explore.schedule,
                    None,
                    Some(shrink_trial_duration()),
                )?;
                if crate::shrink_status_matches(target_status, status) {
                    steps = trial;
                    improved = true;
                    continue;
                }
                i += chunk;
            }
            if !improved {
                if chunk == 1 {
                    break;
                }
                chunk = chunk.div_ceil(2);
            }
        }
        shrunk_scenario.steps = steps;
    }

    let out_path = opt
        .out_trace_path
        .clone()
        .unwrap_or_else(|| crate::default_min_trace_path(trace_path.as_path()));

    let replay_started_at = wall_time_iso_utc();
    let replay_started = Instant::now();
    let (status, findings, events, _delivered, out_decisions) = if opt.minimize
        == crate::ShrinkMinimize::All
    {
        let trial = run_explore_inner(
            &shrunk_scenario,
            seed,
            explore.schedule,
            None,
            Some(shrink_trial_duration()),
        )?;
        if crate::shrink_status_matches(target_status, trial.0) {
            trial
        } else {
            run_explore_replay_inner(&explore.scenario, seed, explore.schedule, &best_decisions)?
        }
    } else {
        run_explore_replay_inner(&explore.scenario, seed, explore.schedule, &best_decisions)?
    };
    let replay_finished_at = wall_time_iso_utc();
    let (duration_ms, duration_ns) = crate::duration_fields(replay_started.elapsed());

    let summary = RunSummary {
        status,
        mode: RunMode::Explore,
        identity: RunIdentity {
            run_id: Uuid::new_v4().to_string(),
            seed,
            trace_path: Some(out_path.to_string_lossy().to_string()),
            report_path: None,
            artifacts_dir: None,
        },
        started_at: replay_started_at,
        finished_at: replay_finished_at,
        duration_ms,
        duration_ns,
        tests: None,
        memory: trace.memory.as_ref().map(|m| m.summary.clone()),
        findings,
    };

    let out_explore = if opt.minimize == crate::ShrinkMinimize::All {
        ExploreTrace {
            scenario_path: explore.scenario_path.clone(),
            scenario: shrunk_scenario,
            schedule: explore.schedule,
        }
    } else {
        explore.clone()
    };

    let trace_out = TraceFile::new_explore(out_explore, out_decisions, events, summary.clone());
    trace_out.write_json(&out_path).map_err(|err| {
        FozzyError::Trace(format!(
            "failed to write shrunk explore trace to {}: {err}",
            out_path.display()
        ))
    })?;

    Ok(crate::ShrinkResult {
        out_trace_path: out_path.to_string_lossy().to_string(),
        result: crate::RunResult { summary },
    })
}
