use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;
use rand_core::RngCore as _;
use uuid::Uuid;

use crate::finalize::{write_reporter_artifacts, write_summary_report};
use crate::{
    Config, ExitStatus, Finding, FindingKind, MemoryState, ProfileCaptureLevel, RunIdentity,
    RunMode, RunSummary, TraceFile, should_emit_profile_artifacts, wall_time_iso_utc,
    write_memory_artifacts, write_profile_artifacts_from_trace_with_source,
};
use crate::{FozzyError, FozzyResult};
use crate::heap_budget_findings_from_trace;

use super::{
    FuzzCoverageStats, FuzzMode, FuzzOptions, FuzzTarget, crash_trace_output_path,
    execute_target, fuzz_exec_memory, fuzz_trace_memory_options, gen_seed, heap_budget_policy,
    hex_decode, load_corpus, minimize_input, mutate_bytes, persist_corpus_input,
    persist_crash_input, persist_crash_min_input, rng_from_seed, should_emit_heavy_artifacts,
    target_string,
};

type LastExec = (
    Vec<u8>,
    Vec<crate::TraceEvent>,
    ExitStatus,
    Vec<Finding>,
    Option<crate::MemoryTrace>,
);

pub fn fuzz(
    config: &Config,
    target: &FuzzTarget,
    opt: &FuzzOptions,
) -> FozzyResult<crate::RunResult> {
    if opt.det && opt.time.is_some() {
        return Err(FozzyError::InvalidArgument(
            "fuzz --det requires --runs and does not support wall-clock --time".to_string(),
        ));
    }
    if opt.det && opt.runs.is_none() {
        return Err(FozzyError::InvalidArgument(
            "fuzz --det requires an explicit --runs value".to_string(),
        ));
    }

    let seed = if opt.det {
        opt.seed.unwrap_or(0)
    } else {
        opt.seed.unwrap_or_else(gen_seed)
    };
    let run_id = Uuid::new_v4().to_string();
    let started_at = wall_time_iso_utc();
    let started = Instant::now();

    let artifacts_dir = config.runs_dir().join(&run_id);
    std::fs::create_dir_all(&artifacts_dir)?;

    let corpus_dir = opt
        .corpus_dir
        .clone()
        .unwrap_or_else(|| config.corpora_dir().join("default"));
    std::fs::create_dir_all(&corpus_dir)?;
    std::fs::create_dir_all(corpus_dir.join("crashes"))?;

    let mut rng = rng_from_seed(seed);
    let deadline = if opt.det {
        None
    } else {
        opt.time.map(|time| started + time)
    };
    let max_runs = opt.runs.unwrap_or(u64::MAX);

    let mut corpus = load_corpus(&corpus_dir)?;
    if corpus.is_empty() {
        corpus.push(Vec::new());
        corpus.push(vec![0u8]);
        corpus.push(vec![1u8, 2u8, 3u8]);
    }

    let mut global_coverage: HashSet<u64> = HashSet::new();
    let mut discovered_edges_total = 0u64;
    let mut max_new_edges_per_input = 0u64;
    let mut findings = Vec::new();
    let mut crash_trace_path: Option<PathBuf> = None;
    let mut crash_count = 0u64;
    let mut last_exec: Option<LastExec> = None;
    let mut memory_state = if opt.memory.track {
        Some(MemoryState::new(opt.memory.clone()))
    } else {
        None
    };

    let mut executed = 0u64;
    while executed < max_runs {
        if let Some(deadline) = deadline
            && Instant::now() >= deadline
        {
            break;
        }

        let base = &corpus[(rng.next_u64() as usize) % corpus.len()];
        let mut input = base.clone();
        mutate_bytes(&mut input, &mut rng, opt.max_input_bytes);

        let mut exec = execute_target(config, target, &input, &opt.memory)?;
        if let Some(mem) = memory_state.as_mut() {
            let outcome = mem.allocate(
                input.len() as u64,
                Some("fuzz_input".to_string()),
                "fuzz_loop",
                executed,
            );
            if let Some(reason) = outcome.failed_reason {
                exec.status = ExitStatus::Fail;
                exec.findings.push(Finding {
                    kind: FindingKind::Checker,
                    title: "memory_alloc_failed".to_string(),
                    message: format!(
                        "memory allocation failed during fuzz input execution: {reason}"
                    ),
                    location: None,
                });
            } else if let Some(id) = outcome.alloc_id {
                let _ = mem.free(id, executed);
            }
        }
        last_exec = Some((
            input.clone(),
            exec.events.clone(),
            exec.status,
            exec.findings.clone(),
            exec.memory.clone(),
        ));
        executed += 1;

        let new_edges: Vec<u64> = exec
            .coverage
            .iter()
            .copied()
            .filter(|edge| !global_coverage.contains(edge))
            .collect();
        if !new_edges.is_empty() {
            discovered_edges_total = discovered_edges_total.saturating_add(new_edges.len() as u64);
            max_new_edges_per_input = max_new_edges_per_input.max(new_edges.len() as u64);
            for edge in &new_edges {
                global_coverage.insert(*edge);
            }
            if matches!(opt.mode, FuzzMode::Coverage) {
                corpus.push(input.clone());
                persist_corpus_input(&corpus_dir, &input)?;
            }
        }

        if exec.status != ExitStatus::Pass {
            crash_count += 1;
            findings.extend(exec.findings.clone());

            let _crash_path = persist_crash_input(&corpus_dir, &input)?;
            let report_path = artifacts_dir.join("report.json");
            let finished_at = wall_time_iso_utc();
            let (duration_ms, duration_ns) = crate::duration_fields(started.elapsed());

            let harness_memory = memory_state.as_ref().map(|memory| memory.clone().finalize());
            let crash_memory = fuzz_exec_memory(exec.memory.as_ref(), harness_memory.as_ref());
            let mut summary = RunSummary {
                status: exec.status,
                mode: RunMode::Fuzz,
                identity: RunIdentity {
                    run_id: run_id.clone(),
                    seed,
                    trace_path: None,
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
                },
                started_at: started_at.clone(),
                finished_at,
                duration_ms,
                duration_ns,
                tests: None,
                memory: crash_memory.as_ref().map(|memory| memory.summary.clone()),
                findings: exec.findings.clone(),
            };
            let mut budget_trace = TraceFile::new_fuzz(
                target_string(target),
                &input,
                exec.events.clone(),
                summary.clone(),
            );
            budget_trace.memory = crash_memory.as_ref().map(|memory| memory.to_trace());
            let heap_findings =
                heap_budget_findings_from_trace(&budget_trace, &heap_budget_policy(config));
            if !heap_findings.is_empty() {
                summary.findings.extend(heap_findings);
                summary.findings = crate::collapse_findings(summary.findings.clone());
            }

            write_reporter_artifacts(&summary, &artifacts_dir, opt.reporter)?;

            let requested_trace_out = crash_trace_output_path(
                opt.record_trace_to.as_deref(),
                &artifacts_dir,
                crash_count,
            );
            let trace_out =
                crate::resolve_record_target(&requested_trace_out, opt.record_collision)?;
            summary.identity.trace_path = Some(trace_out.to_string_lossy().to_string());
            let mut trace = TraceFile::new_fuzz(
                target_string(target),
                &input,
                exec.events.clone(),
                summary.clone(),
            );
            trace.memory = crash_memory.as_ref().map(|memory| memory.to_trace());
            crate::write_trace_to_target(&trace, &trace_out)?;
            crash_trace_path = Some(trace_out.clone());
            let emit_heavy = should_emit_heavy_artifacts(exec.status, true)
                || matches!(opt.profile_capture, ProfileCaptureLevel::Full);
            if emit_heavy {
                std::fs::write(
                    artifacts_dir.join("events.json"),
                    serde_json::to_vec(&exec.events)?,
                )?;
                crate::write_timeline(&exec.events, &artifacts_dir.join("timeline.json"))?;
            }
            let mut profile_metadata = None;
            if should_emit_profile_artifacts(opt.profile_capture, exec.status, true) {
                let mut profile_trace = budget_trace;
                profile_trace.summary = summary.clone();
                profile_metadata = Some(write_profile_artifacts_from_trace_with_source(
                    &profile_trace,
                    Some(trace_out.as_path()),
                    &artifacts_dir,
                )?);
            }
            write_summary_report(
                &summary,
                &report_path,
                &artifacts_dir,
                profile_metadata.as_ref(),
            )?;

            if opt.minimize || opt.shrink {
                let minimized = minimize_input(
                    config,
                    target,
                    &input,
                    opt.max_input_bytes,
                    exec.status,
                    &opt.memory,
                )?;
                let _min_path = persist_crash_min_input(&corpus_dir, &minimized)?;
            }

            if opt.crash_only {
                break;
            }
        }
    }

    let finished_at = wall_time_iso_utc();
    let (duration_ms, duration_ns) = crate::duration_fields(started.elapsed());
    let mut status = if crash_count == 0 {
        ExitStatus::Pass
    } else {
        ExitStatus::Fail
    };
    let report_path = artifacts_dir.join("report.json");

    let memory_report = memory_state.map(|memory| memory.finalize());
    let last_exec_memory = last_exec
        .as_ref()
        .and_then(|(_, _, _, _, memory)| memory.clone());
    let effective_memory = fuzz_exec_memory(last_exec_memory.as_ref(), memory_report.as_ref());
    findings = crate::collapse_findings(findings);
    let mut summary = RunSummary {
        status,
        mode: RunMode::Fuzz,
        identity: RunIdentity {
            run_id: run_id.clone(),
            seed,
            trace_path: crash_trace_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
            report_path: Some(report_path.to_string_lossy().to_string()),
            artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
        },
        started_at,
        finished_at,
        duration_ms,
        duration_ns,
        tests: None,
        memory: effective_memory.as_ref().map(|memory| memory.summary.clone()),
        findings,
    };
    let (profile_input, profile_events, profile_status, profile_findings, profile_memory) = last_exec
        .clone()
        .unwrap_or_else(|| (Vec::new(), Vec::new(), ExitStatus::Pass, Vec::new(), None));
    let mut profile_summary = summary.clone();
    profile_summary.status = profile_status;
    profile_summary.findings = profile_findings;
    let mut profile_trace = TraceFile::new_fuzz(
        target_string(target),
        &profile_input,
        profile_events,
        profile_summary,
    );
    profile_trace.memory =
        profile_memory.or_else(|| effective_memory.as_ref().map(|memory| memory.to_trace()));
    let heap_findings =
        heap_budget_findings_from_trace(&profile_trace, &heap_budget_policy(config));
    if !heap_findings.is_empty() {
        summary.findings.extend(heap_findings);
        summary.findings = crate::collapse_findings(summary.findings.clone());
    }
    if let Some(memory) = effective_memory.as_ref() {
        if memory.options.fail_on_leak && memory.summary.leaked_bytes > 0 {
            status = ExitStatus::Fail;
            summary.status = status;
        }
    }

    if let Some(memory) = effective_memory.as_ref()
        && memory.options.artifacts
    {
        write_memory_artifacts(memory, &artifacts_dir)?;
    }
    let coverage_stats = FuzzCoverageStats {
        target: target_string(target),
        executed,
        crashes: crash_count,
        unique_edges: global_coverage.len(),
        discovered_edges_total,
        max_new_edges_per_input,
        corpus_entries: corpus.len(),
    };
    std::fs::write(
        artifacts_dir.join("coverage.json"),
        serde_json::to_vec(&coverage_stats)?,
    )?;

    if let Some(record_path) = &opt.record_trace_to
        && crash_trace_path.is_none()
    {
        let (input, events, exec_status, exec_findings, exec_memory) =
            last_exec.unwrap_or_else(|| (Vec::new(), Vec::new(), ExitStatus::Pass, Vec::new(), None));
        let written = crate::resolve_record_target(record_path, opt.record_collision)?;
        summary.identity.trace_path = Some(written.to_string_lossy().to_string());
        let mut trace_summary = summary.clone();
        trace_summary.status = exec_status;
        trace_summary.findings = exec_findings;
        trace_summary.identity.trace_path = Some(written.to_string_lossy().to_string());
        let mut trace = TraceFile::new_fuzz(target_string(target), &input, events, trace_summary);
        trace.memory = exec_memory.or_else(|| effective_memory.as_ref().map(|memory| memory.to_trace()));
        crate::write_trace_to_target(&trace, &written)?;
    }
    profile_trace.summary = {
        let mut summary_for_profile = summary.clone();
        summary_for_profile.status = profile_status;
        summary_for_profile
    };
    let explicit_capture = opt.record_trace_to.is_some() || crash_trace_path.is_some();
    let emit_heavy = should_emit_heavy_artifacts(status, explicit_capture)
        || matches!(opt.profile_capture, ProfileCaptureLevel::Full);
    let source_trace_path = summary.identity.trace_path.as_deref().map(std::path::Path::new);
    let mut profile_metadata = None;
    if emit_heavy {
        profile_metadata = Some(write_profile_artifacts_from_trace_with_source(
            &profile_trace,
            source_trace_path,
            &artifacts_dir,
        )?);
    }
    if !emit_heavy && should_emit_profile_artifacts(opt.profile_capture, status, explicit_capture) {
        profile_metadata = Some(write_profile_artifacts_from_trace_with_source(
            &profile_trace,
            source_trace_path,
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

pub fn replay_fuzz_trace(
    config: &Config,
    trace: &TraceFile,
    trace_path: &std::path::Path,
    opt: &crate::ReplayOptions,
) -> FozzyResult<crate::RunResult> {
    let Some(fuzz) = trace.fuzz.as_ref() else {
        return Err(FozzyError::Trace("not a fuzz trace".to_string()));
    };
    let target: FuzzTarget = fuzz.target.parse()?;
    let input = hex_decode(&fuzz.input_hex)?;
    let exec = execute_target(config, &target, &input, &fuzz_trace_memory_options(trace))?;

    let run_id = Uuid::new_v4().to_string();
    let artifacts_dir = config.runs_dir().join(&run_id);
    std::fs::create_dir_all(&artifacts_dir)?;
    let report_path = artifacts_dir.join("report.json");

    let mut findings = exec.findings.clone();
    for warning in crate::trace_schema_warnings(trace.version) {
        findings.push(Finding {
            kind: FindingKind::Checker,
            title: "stale_trace_schema".to_string(),
            message: warning,
            location: None,
        });
    }

    if trace.memory.is_some() != exec.memory.is_some() {
        findings.push(Finding {
            kind: FindingKind::Checker,
            title: "replay_memory_drift".to_string(),
            message: format!(
                "replay memory presence drift: expected_memory={} actual_memory={}",
                trace.memory.is_some(),
                exec.memory.is_some()
            ),
            location: None,
        });
    } else if let (Some(expected), Some(actual)) = (trace.memory.as_ref(), exec.memory.as_ref())
        && expected.summary != actual.summary
    {
        findings.push(Finding {
            kind: FindingKind::Checker,
            title: "replay_memory_drift".to_string(),
            message: format!(
                "replay memory drift: expected leaked_bytes={} leaked_allocs={} peak_bytes={}, got leaked_bytes={} leaked_allocs={} peak_bytes={}",
                expected.summary.leaked_bytes,
                expected.summary.leaked_allocs,
                expected.summary.peak_bytes,
                actual.summary.leaked_bytes,
                actual.summary.leaked_allocs,
                actual.summary.peak_bytes
            ),
            location: None,
        });
    }

    let replay_status = if findings
        .iter()
        .any(|finding| finding.kind == FindingKind::Checker && finding.title.starts_with("replay_"))
        && exec.status == ExitStatus::Pass
    {
        ExitStatus::Fail
    } else {
        exec.status
    };

    let mut summary = RunSummary {
        status: replay_status,
        mode: RunMode::Replay,
        identity: RunIdentity {
            run_id,
            seed: trace.summary.identity.seed,
            trace_path: Some(trace_path.to_string_lossy().to_string()),
            report_path: Some(report_path.to_string_lossy().to_string()),
            artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
        },
        started_at: exec.started_at.clone(),
        finished_at: exec.finished_at.clone(),
        duration_ms: exec.duration_ms,
        duration_ns: exec.duration_ns,
        tests: None,
        memory: exec.memory.as_ref().map(|memory| memory.summary.clone()),
        findings,
    };
    let mut profile_trace = TraceFile::new_fuzz(
        fuzz.target.clone(),
        &input,
        exec.events.clone(),
        summary.clone(),
    );
    profile_trace.memory = exec.memory.clone();
    let heap_findings =
        heap_budget_findings_from_trace(&profile_trace, &heap_budget_policy(config));
    if !heap_findings.is_empty() {
        summary.findings.extend(heap_findings);
        summary.findings = crate::collapse_findings(summary.findings.clone());
    }

    let explicit_capture = opt.dump_events;
    let emit_heavy = should_emit_heavy_artifacts(replay_status, explicit_capture)
        || matches!(opt.profile_capture, ProfileCaptureLevel::Full);
    if emit_heavy {
        std::fs::write(
            artifacts_dir.join("events.json"),
            serde_json::to_vec(&exec.events)?,
        )?;
        crate::write_timeline(&exec.events, &artifacts_dir.join("timeline.json"))?;
    }
    let mut profile_metadata = None;
    if should_emit_profile_artifacts(opt.profile_capture, replay_status, explicit_capture) {
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

pub fn shrink_fuzz_trace(
    config: &Config,
    trace_path: crate::TracePath,
    opt: &crate::ShrinkOptions,
) -> FozzyResult<crate::ShrinkResult> {
    let trace = TraceFile::read_json(trace_path.as_path())?;
    let target_status = trace.summary.status;
    let Some(fuzz) = trace.fuzz.as_ref() else {
        return Err(FozzyError::Trace("not a fuzz trace".to_string()));
    };

    let target: FuzzTarget = fuzz.target.parse()?;
    let input = hex_decode(&fuzz.input_hex)?;

    if opt.minimize != crate::ShrinkMinimize::All && opt.minimize != crate::ShrinkMinimize::Input {
        return Err(FozzyError::InvalidArgument(
            "fuzz shrink only supports --minimize input|all".to_string(),
        ));
    }

    let minimized = minimize_input(
        config,
        &target,
        &input,
        1024 * 1024,
        target_status,
        &fuzz_trace_memory_options(&trace),
    )?;
    let exec = execute_target(config, &target, &minimized, &fuzz_trace_memory_options(&trace))?;

    let out_path = opt
        .out_trace_path
        .clone()
        .unwrap_or_else(|| crate::default_min_trace_path(trace_path.as_path()));

    let summary = RunSummary {
        status: exec.status,
        mode: RunMode::Fuzz,
        identity: RunIdentity {
            run_id: Uuid::new_v4().to_string(),
            seed: trace.summary.identity.seed,
            trace_path: Some(out_path.to_string_lossy().to_string()),
            report_path: None,
            artifacts_dir: None,
        },
        started_at: exec.started_at.clone(),
        finished_at: exec.finished_at.clone(),
        duration_ms: exec.duration_ms,
        duration_ns: exec.duration_ns,
        tests: None,
        memory: exec.memory.as_ref().map(|memory| memory.summary.clone()),
        findings: exec.findings.clone(),
    };

    let mut trace_out = TraceFile::new_fuzz(
        target_string(&target),
        &minimized,
        exec.events,
        summary.clone(),
    );
    trace_out.memory = exec.memory.clone();
    trace_out.write_json(&out_path).map_err(|err| {
        FozzyError::Trace(format!(
            "failed to write shrunk fuzz trace to {}: {err}",
            out_path.display()
        ))
    })?;

    Ok(crate::ShrinkResult {
        out_trace_path: out_path.to_string_lossy().to_string(),
        result: crate::RunResult { summary },
    })
}
