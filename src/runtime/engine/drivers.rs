use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{
    Config, Decision, ExitStatus, Finding, FindingKind, FozzyError, FozzyResult, MemoryOptions,
    RunMode, Scenario, ScenarioPath, ScenarioV1Steps, TraceEvent,
};

use super::exec::ExecCtx;
use super::helpers::{ReplayCursor, should_emit_heavy_artifacts};
use super::types::{FsBackend, HttpBackend, ProcBackend, ProfileCaptureLevel, ScenarioRun};

pub(crate) fn shrink_status_matches(target: ExitStatus, candidate: ExitStatus) -> bool {
    if target == ExitStatus::Pass {
        candidate == ExitStatus::Pass
    } else {
        candidate != ExitStatus::Pass
    }
}

pub fn should_emit_profile_artifacts(
    capture: ProfileCaptureLevel,
    status: ExitStatus,
    explicit_request: bool,
) -> bool {
    match capture {
        ProfileCaptureLevel::Baseline => should_emit_heavy_artifacts(status, explicit_request),
        ProfileCaptureLevel::Full => true,
    }
}

pub(crate) fn run_scenario_inner(
    _config: &Config,
    _mode: RunMode,
    scenario_path: ScenarioPath,
    seed: u64,
    det: bool,
    timeout: Option<Duration>,
    proc_backend: ProcBackend,
    fs_backend: FsBackend,
    http_backend: HttpBackend,
    memory: MemoryOptions,
) -> FozzyResult<ScenarioRun> {
    let loaded = Scenario::load(&scenario_path)?;
    loaded.validate()?;

    let embedded = ScenarioV1Steps {
        version: 1,
        name: loaded.name.clone(),
        steps: loaded.steps.clone(),
    };

    run_embedded_scenario_inner(
        embedded,
        scenario_path.as_path().to_path_buf(),
        seed,
        det,
        timeout,
        proc_backend,
        fs_backend,
        http_backend,
        memory,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_embedded_scenario_inner(
    scenario: ScenarioV1Steps,
    scenario_path: PathBuf,
    seed: u64,
    det: bool,
    timeout: Option<Duration>,
    proc_backend: ProcBackend,
    fs_backend: FsBackend,
    http_backend: HttpBackend,
    memory: MemoryOptions,
) -> FozzyResult<ScenarioRun> {
    let started_at = crate::wall_time_iso_utc();
    let started = Instant::now();
    let deadline = timeout.map(|t| started + t);
    let mut ctx = ExecCtx::new(
        seed,
        det,
        deadline,
        proc_backend,
        fs_backend,
        http_backend,
        memory,
    );
    let start_virtual_ms = ctx.clock.now_ms();
    let mut scheduler = crate::DeterministicScheduler::new(crate::SchedulerMode::Fifo, seed);
    for (idx, step) in scenario.steps.iter().enumerate() {
        scheduler.enqueue(step.kind_name().to_string(), idx);
    }

    while let Some(item) = scheduler.pop_next() {
        let idx = item.payload;
        let step = &scenario.steps[idx];
        if timeout_reached(&ctx, det, timeout, deadline, start_virtual_ms) {
            ctx.findings.push(Finding {
                kind: FindingKind::Hang,
                title: "timeout".to_string(),
                message: "scenario timed out".to_string(),
                location: None,
            });
            return Ok(ctx.finish(
                ExitStatus::Timeout,
                scenario_path,
                scenario,
                started_at.clone(),
                started.elapsed(),
            ));
        }

        ctx.decisions.push(Decision::SchedulerPick {
            task_id: item.id,
            label: item.label,
        });
        let step_kind = step.kind_name().to_string();
        let span_id = format!("step-{idx}");
        let step_start_ms = ctx.clock.now_ms();
        ctx.events.push(TraceEvent {
            time_ms: step_start_ms,
            name: "sched_pick".to_string(),
            fields: serde_json::Map::from_iter([
                ("task_id".to_string(), serde_json::json!(item.id)),
                ("step_index".to_string(), serde_json::json!(idx as u64)),
                (
                    "step_kind".to_string(),
                    serde_json::json!(step_kind.clone()),
                ),
            ]),
        });
        ctx.events.push(TraceEvent {
            time_ms: step_start_ms,
            name: "span_start".to_string(),
            fields: serde_json::Map::from_iter([
                ("span".to_string(), serde_json::json!(span_id.clone())),
                ("task".to_string(), serde_json::json!("step")),
                ("step_index".to_string(), serde_json::json!(idx as u64)),
                (
                    "step_kind".to_string(),
                    serde_json::json!(step_kind.clone()),
                ),
            ]),
        });
        ctx.set_active_step(&scenario_path, idx);
        if let Err(finding) = ctx.exec_step(step) {
            let end_ms = ctx.clock.now_ms();
            ctx.events.push(TraceEvent {
                time_ms: end_ms,
                name: "span_end".to_string(),
                fields: serde_json::Map::from_iter([
                    ("span".to_string(), serde_json::json!(span_id)),
                    ("status".to_string(), serde_json::json!("error")),
                    (
                        "duration_ms".to_string(),
                        serde_json::json!(end_ms.saturating_sub(step_start_ms)),
                    ),
                ]),
            });
            let status = if finding_is_timeout(&finding) {
                ExitStatus::Timeout
            } else {
                ExitStatus::Fail
            };
            ctx.findings.push(finding);
            return Ok(ctx.finish(
                status,
                scenario_path,
                scenario,
                started_at.clone(),
                started.elapsed(),
            ));
        }
        let end_ms = ctx.clock.now_ms();
        ctx.events.push(TraceEvent {
            time_ms: end_ms,
            name: "span_end".to_string(),
            fields: serde_json::Map::from_iter([
                ("span".to_string(), serde_json::json!(span_id)),
                ("status".to_string(), serde_json::json!("ok")),
                (
                    "duration_ms".to_string(),
                    serde_json::json!(end_ms.saturating_sub(step_start_ms)),
                ),
            ]),
        });

        if timeout_reached(&ctx, det, timeout, deadline, start_virtual_ms) {
            ctx.findings.push(Finding {
                kind: FindingKind::Hang,
                title: "timeout".to_string(),
                message: "scenario timed out".to_string(),
                location: None,
            });
            return Ok(ctx.finish(
                ExitStatus::Timeout,
                scenario_path,
                scenario,
                started_at.clone(),
                started.elapsed(),
            ));
        }
    }

    Ok(ctx.finish(
        ExitStatus::Pass,
        scenario_path,
        scenario,
        started_at,
        started.elapsed(),
    ))
}

pub(crate) fn run_embedded_steps_for_fuzz(
    scenario: &ScenarioV1Steps,
    scenario_path: &Path,
    seed: u64,
    memory: MemoryOptions,
) -> FozzyResult<ScenarioRun> {
    run_embedded_scenario_inner(
        scenario.clone(),
        scenario_path.to_path_buf(),
        seed,
        true,
        None,
        ProcBackend::Scripted,
        FsBackend::Virtual,
        HttpBackend::Scripted,
        memory,
    )
}

fn timeout_reached(
    ctx: &ExecCtx<'_>,
    det: bool,
    timeout: Option<Duration>,
    deadline: Option<Instant>,
    start_virtual_ms: u64,
) -> bool {
    let Some(limit) = timeout else {
        return false;
    };
    if det {
        let elapsed_ms = ctx.clock.now_ms().saturating_sub(start_virtual_ms);
        elapsed_ms >= limit.as_millis().min(u128::from(u64::MAX)) as u64
    } else {
        deadline.is_some_and(|dl| Instant::now() > dl)
    }
}

fn finding_is_timeout(finding: &Finding) -> bool {
    finding.kind == FindingKind::Hang && finding.title == "timeout"
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_scenario_replay_inner<'a>(
    _config: &Config,
    _mode: RunMode,
    scenario: &ScenarioV1Steps,
    scenario_path: &str,
    seed: u64,
    decisions: Option<&'a [Decision]>,
    until: Option<Duration>,
    step: bool,
    proc_backend: ProcBackend,
    fs_backend: FsBackend,
    http_backend: HttpBackend,
    memory: MemoryOptions,
) -> FozzyResult<ScenarioRun> {
    if scenario.version != 1 {
        return Err(FozzyError::Scenario(format!(
            "unsupported scenario version {} (expected 1)",
            scenario.version
        )));
    }

    let has_scheduler_pick = decisions
        .map(|d| {
            d.iter()
                .any(|x| matches!(x, Decision::SchedulerPick { .. }))
        })
        .unwrap_or(false);

    let started_at = crate::wall_time_iso_utc();
    let started = Instant::now();
    let deadline = until.map(|t| started + t);
    let mut ctx = ExecCtx::new(
        seed,
        true,
        deadline,
        proc_backend,
        fs_backend,
        http_backend,
        memory,
    );
    if let Some(d) = decisions {
        ctx.replay = Some(ReplayCursor::new(d));
    }

    if has_scheduler_pick {
        let mut scheduler = crate::DeterministicScheduler::new(crate::SchedulerMode::Fifo, seed);
        for (idx, step) in scenario.steps.iter().enumerate() {
            scheduler.enqueue(step.kind_name().to_string(), idx);
        }
        while let Some(item) = scheduler.pop_next() {
            let idx = item.payload;
            let step_def = &scenario.steps[idx];
            if let Some(dl) = deadline
                && Instant::now() > dl
            {
                ctx.findings.push(Finding {
                    kind: FindingKind::Hang,
                    title: "until".to_string(),
                    message: "replay stopped at --until budget".to_string(),
                    location: None,
                });
                return Ok(ctx.finish(
                    ExitStatus::Timeout,
                    PathBuf::from(scenario_path),
                    scenario.clone(),
                    started_at.clone(),
                    started.elapsed(),
                ));
            }

            if step {
                std::thread::sleep(Duration::from_millis(10));
            }

            ctx.expect_scheduler_pick(item.id, &item.label)?;
            let step_kind = step_def.kind_name().to_string();
            let span_id = format!("step-{idx}");
            let step_start_ms = ctx.clock.now_ms();
            ctx.events.push(TraceEvent {
                time_ms: step_start_ms,
                name: "sched_pick".to_string(),
                fields: serde_json::Map::from_iter([
                    ("task_id".to_string(), serde_json::json!(item.id)),
                    ("step_index".to_string(), serde_json::json!(idx as u64)),
                    (
                        "step_kind".to_string(),
                        serde_json::json!(step_kind.clone()),
                    ),
                ]),
            });
            ctx.events.push(TraceEvent {
                time_ms: step_start_ms,
                name: "span_start".to_string(),
                fields: serde_json::Map::from_iter([
                    ("span".to_string(), serde_json::json!(span_id.clone())),
                    ("task".to_string(), serde_json::json!("step")),
                    ("step_index".to_string(), serde_json::json!(idx as u64)),
                    (
                        "step_kind".to_string(),
                        serde_json::json!(step_kind.clone()),
                    ),
                ]),
            });
            ctx.set_active_step(Path::new(scenario_path), idx);
            if let Err(finding) = ctx.exec_step(step_def) {
                let end_ms = ctx.clock.now_ms();
                ctx.events.push(TraceEvent {
                    time_ms: end_ms,
                    name: "span_end".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("span".to_string(), serde_json::json!(span_id)),
                        ("status".to_string(), serde_json::json!("error")),
                        (
                            "duration_ms".to_string(),
                            serde_json::json!(end_ms.saturating_sub(step_start_ms)),
                        ),
                    ]),
                });
                let status = if finding_is_timeout(&finding) {
                    ExitStatus::Timeout
                } else {
                    ExitStatus::Fail
                };
                ctx.findings.push(finding);
                return Ok(ctx.finish(
                    status,
                    PathBuf::from(scenario_path),
                    scenario.clone(),
                    started_at.clone(),
                    started.elapsed(),
                ));
            }
            let end_ms = ctx.clock.now_ms();
            ctx.events.push(TraceEvent {
                time_ms: end_ms,
                name: "span_end".to_string(),
                fields: serde_json::Map::from_iter([
                    ("span".to_string(), serde_json::json!(span_id)),
                    ("status".to_string(), serde_json::json!("ok")),
                    (
                        "duration_ms".to_string(),
                        serde_json::json!(end_ms.saturating_sub(step_start_ms)),
                    ),
                ]),
            });
        }
    } else {
        for (idx, step_def) in scenario.steps.iter().enumerate() {
            if let Some(dl) = deadline
                && Instant::now() > dl
            {
                ctx.findings.push(Finding {
                    kind: FindingKind::Hang,
                    title: "until".to_string(),
                    message: "replay stopped at --until budget".to_string(),
                    location: None,
                });
                return Ok(ctx.finish(
                    ExitStatus::Timeout,
                    PathBuf::from(scenario_path),
                    scenario.clone(),
                    started_at.clone(),
                    started.elapsed(),
                ));
            }

            if step {
                std::thread::sleep(Duration::from_millis(10));
            }

            ctx.expect_step(idx)?;
            let step_kind = step_def.kind_name().to_string();
            let span_id = format!("step-{idx}");
            let step_start_ms = ctx.clock.now_ms();
            ctx.events.push(TraceEvent {
                time_ms: step_start_ms,
                name: "sched_pick".to_string(),
                fields: serde_json::Map::from_iter([
                    ("task_id".to_string(), serde_json::json!(idx as u64 + 1)),
                    ("step_index".to_string(), serde_json::json!(idx as u64)),
                    (
                        "step_kind".to_string(),
                        serde_json::json!(step_kind.clone()),
                    ),
                ]),
            });
            ctx.events.push(TraceEvent {
                time_ms: step_start_ms,
                name: "span_start".to_string(),
                fields: serde_json::Map::from_iter([
                    ("span".to_string(), serde_json::json!(span_id.clone())),
                    ("task".to_string(), serde_json::json!("step")),
                    ("step_index".to_string(), serde_json::json!(idx as u64)),
                    (
                        "step_kind".to_string(),
                        serde_json::json!(step_kind.clone()),
                    ),
                ]),
            });
            ctx.set_active_step(Path::new(scenario_path), idx);
            if let Err(finding) = ctx.exec_step(step_def) {
                let end_ms = ctx.clock.now_ms();
                ctx.events.push(TraceEvent {
                    time_ms: end_ms,
                    name: "span_end".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("span".to_string(), serde_json::json!(span_id)),
                        ("status".to_string(), serde_json::json!("error")),
                        (
                            "duration_ms".to_string(),
                            serde_json::json!(end_ms.saturating_sub(step_start_ms)),
                        ),
                    ]),
                });
                let status = if finding_is_timeout(&finding) {
                    ExitStatus::Timeout
                } else {
                    ExitStatus::Fail
                };
                ctx.findings.push(finding);
                return Ok(ctx.finish(
                    status,
                    PathBuf::from(scenario_path),
                    scenario.clone(),
                    started_at.clone(),
                    started.elapsed(),
                ));
            }
            let end_ms = ctx.clock.now_ms();
            ctx.events.push(TraceEvent {
                time_ms: end_ms,
                name: "span_end".to_string(),
                fields: serde_json::Map::from_iter([
                    ("span".to_string(), serde_json::json!(span_id)),
                    ("status".to_string(), serde_json::json!("ok")),
                    (
                        "duration_ms".to_string(),
                        serde_json::json!(end_ms.saturating_sub(step_start_ms)),
                    ),
                ]),
            });
        }
    }

    if let Some(cursor) = ctx.replay.as_ref()
        && cursor.remaining() > 0
    {
        ctx.findings.push(Finding {
            kind: FindingKind::Checker,
            title: "replay_unused_decisions".to_string(),
            message: format!(
                "replay finished with {} unused decisions",
                cursor.remaining()
            ),
            location: None,
        });
        return Ok(ctx.finish(
            ExitStatus::Fail,
            PathBuf::from(scenario_path),
            scenario.clone(),
            started_at,
            started.elapsed(),
        ));
    }

    Ok(ctx.finish(
        ExitStatus::Pass,
        PathBuf::from(scenario_path),
        scenario.clone(),
        started_at,
        started.elapsed(),
    ))
}
