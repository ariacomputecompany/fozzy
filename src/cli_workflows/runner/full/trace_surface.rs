use super::state::{FullRunState, PrimaryRunState, ScenarioSelection};
use super::*;

pub(super) fn run_deterministic_surface(
    state: &mut FullRunState,
    config: &Config,
    selection: &ScenarioSelection,
    seed: Option<u64>,
    doctor_runs: u32,
    strict: bool,
) -> PrimaryRunState {
    let mut run_state = PrimaryRunState::default();
    let Some(primary) = selection.step.clone() else {
        state.push_skipped(
            "doctor_deep",
            "no step scenario found; add tests/*.fozzy.json to run deterministic audits",
        );
        state.push_skipped("test_det", "no step scenario found");
        state.push_skipped("run_record_trace", "no step scenario found");
        return run_state;
    };

    state.start_step("doctor_deep", format!("scenario={}", primary.display()));
    match run_with_timeout("doctor_deep", EXECUTION_TIMEOUT, {
        let config = config.clone();
        let primary = primary.clone();
        move || {
            Ok(fozzy::doctor(
                &config,
                &fozzy::DoctorOptions {
                    deep: true,
                    scenario: Some(ScenarioPath::new(primary.clone())),
                    runs: doctor_runs.max(2),
                    seed,
                },
            )?)
        }
    }) {
        Ok(doctor) => {
            let (status, detail) = doctor_report_status(
                &doctor,
                strict,
                primary.as_path(),
                doctor_runs.max(2),
                resolved_workflow_seed(seed),
            );
            state.push("doctor_deep", status, detail);
        }
        Err(err) => {
            state.abort_due_to_timeout(
                "doctor_deep",
                err.to_string(),
                "The deterministic doctor audit did not finish in time; `fozzy full` stopped here with a structured failure."
                    .to_string(),
            );
            return run_state;
        }
    }

    let filtered_steps: Vec<PathBuf> = selection
        .discovered
        .steps
        .iter()
        .filter(|path| !is_negative_fixture_scenario(path))
        .cloned()
        .collect();
    let test_targets = if filtered_steps.is_empty() {
        vec![primary.clone()]
    } else {
        filtered_steps
    };
    let test_globs: Vec<String> = test_targets
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    state.start_step("test_det", format!("scenarios={}", test_globs.len()));
    match run_with_timeout("test_det", EXECUTION_TIMEOUT, {
        let config = config.clone();
        let test_globs = test_globs.clone();
        let memory = selection.memory.clone();
        move || {
            Ok(fozzy::run_tests(
                &config,
                &test_globs,
                &RunOptions {
                    det: true,
                    seed,
                    timeout: None,
                    reporter: Reporter::Json,
                    record_trace_to: None,
                    filter: None,
                    jobs: None,
                    fail_fast: false,
                    record_collision: RecordCollisionPolicy::Error,
                    profile_capture: ProfileCaptureLevel::Baseline,
                    proc_backend: config.proc_backend,
                    fs_backend: config.fs_backend,
                    http_backend: config.http_backend,
                    memory,
                },
            )?)
        }
    }) {
        Ok(test) => {
            let (status, detail) = run_summary_pass_status(
                &test.summary,
                strict,
                resolved_workflow_seed(seed),
                RunMode::Test,
            );
            state.push(
                "test_det",
                status,
                format!("{detail} run_id={}", test.summary.identity.run_id),
            );
        }
        Err(err) => {
            state.abort_due_to_timeout(
                "test_det",
                err.to_string(),
                "Deterministic suite execution did not finish in time; `fozzy full` emitted a structured failure instead of hanging."
                    .to_string(),
            );
            return run_state;
        }
    }

    let trace_path = state.register_temp(
        std::env::temp_dir().join(format!("fozzy-full-{}.trace.fozzy", uuid::Uuid::new_v4())),
    );
    state.start_step(
        "run_record_trace",
        format!("scenario={}", primary.display()),
    );
    match run_with_timeout("run_record_trace", EXECUTION_TIMEOUT, {
        let config = config.clone();
        let primary = primary.clone();
        let trace_path = trace_path.clone();
        let memory = selection.memory.clone();
        move || {
            Ok(fozzy::run_scenario(
                &config,
                ScenarioPath::new(primary),
                &RunOptions {
                    det: true,
                    seed,
                    timeout: None,
                    reporter: Reporter::Json,
                    record_trace_to: Some(trace_path),
                    filter: None,
                    jobs: None,
                    fail_fast: false,
                    record_collision: RecordCollisionPolicy::Overwrite,
                    profile_capture: ProfileCaptureLevel::Baseline,
                    proc_backend: config.proc_backend,
                    fs_backend: config.fs_backend,
                    http_backend: config.http_backend,
                    memory,
                },
            )?)
        }
    }) {
        Ok(run) => {
            run_state.primary_status = Some(run.summary.status);
            let (status, detail) = recorded_trace_status(
                &run.summary,
                strict,
                resolved_workflow_seed(seed),
                RunMode::Run,
                &trace_path,
            );
            let trace_recorded = run.summary.identity.trace_path.is_some()
                && matches!(file_artifact_status(&trace_path).0, FullStepStatus::Passed);
            if trace_recorded {
                run_state.primary_trace = Some(trace_path);
            }
            state.push("run_record_trace", status, detail);
        }
        Err(err) => state.abort_due_to_timeout(
            "run_record_trace",
            err.to_string(),
            "Primary deterministic trace recording did not finish in time; `fozzy full` stopped with a machine-readable failure."
                .to_string(),
        ),
    }

    run_state
}

pub(super) fn run_trace_surface(
    state: &mut FullRunState,
    config: &Config,
    run_state: &mut PrimaryRunState,
    strict: bool,
    allow_expected_failures: bool,
    seed: Option<u64>,
) {
    let Some(trace) = run_state.primary_trace.as_ref() else {
        state.push_skipped_many(
            &[
                "trace_verify",
                "replay",
                "ci",
                "shrink",
                "replay_shrunk",
                "artifacts_ls",
                "artifacts_export",
                "artifacts_pack",
                "artifacts_diff",
                "report_show",
                "report_query",
                "report_query_paths",
                "report_flaky",
                "memory_top",
                "memory_graph",
                "memory_diff",
                "profile_top",
                "profile_diff",
                "profile_explain",
            ],
            "no recorded trace available",
        );
        return;
    };

    let seed_value = resolved_workflow_seed(seed);

    state.start_step("trace_verify", format!("trace={}", trace.display()));
    match fozzy::verify_trace_file(trace) {
        Ok(verify) => {
            let strict_verify_ok = !strict
                || (verify.checksum_present && verify.checksum_valid && verify.warnings.is_empty());
            state.push(
                "trace_verify",
                if verify.ok && strict_verify_ok {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                format!(
                    "ok={} checksum_present={} checksum_valid={} warnings={}",
                    verify.ok,
                    verify.checksum_present,
                    verify.checksum_valid,
                    if verify.warnings.is_empty() {
                        "<none>".to_string()
                    } else {
                        verify.warnings.join("; ")
                    }
                ),
            );
        }
        Err(err) => state.push("trace_verify", FullStepStatus::Failed, err.to_string()),
    }

    run_replay_step(
        state,
        config,
        trace,
        run_state.primary_status,
        strict,
        seed_value,
        "replay",
    );

    if state.should_abort() {
        return;
    }

    state.start_step("ci", format!("trace={}", trace.display()));
    match run_with_timeout("ci", EXECUTION_TIMEOUT, {
        let config = config.clone();
        let trace = trace.to_path_buf();
        move || {
            Ok(fozzy::ci_evaluate(
                &config,
                &CiOptions {
                    trace,
                    flake_runs: Vec::new(),
                    flake_budget_pct: None,
                    perf_baseline: None,
                    max_p99_delta_pct: None,
                    strict,
                },
            )?)
        }
    }) {
        Ok(ci) => {
            let (status, detail) = ci_report_status(&ci);
            state.push("ci", status, detail);
        }
        Err(err) => {
            state.abort_due_to_timeout(
                "ci",
                err.to_string(),
                "CI evaluation did not finish in time; `fozzy full` terminated with a structured failure."
                    .to_string(),
            );
            return;
        }
    }

    let shrink_out = state.register_temp(
        std::env::temp_dir().join(format!("fozzy-full-{}.min.fozzy", uuid::Uuid::new_v4())),
    );
    state.start_step("shrink", format!("trace={}", trace.display()));
    match run_with_timeout("shrink", EXECUTION_TIMEOUT, {
        let config = config.clone();
        let trace = trace.to_path_buf();
        let shrink_out = shrink_out.clone();
        move || {
            Ok(fozzy::shrink_trace(
                &config,
                TracePath::new(trace),
                &fozzy::ShrinkOptions {
                    out_trace_path: Some(shrink_out),
                    budget: None,
                    aggressive: false,
                    minimize: ShrinkMinimize::All,
                },
            )?)
        }
    }) {
        Ok(shrink) => {
            run_state.shrunk_trace = Some(PathBuf::from(shrink.out_trace_path.clone()));
            run_state.shrunk_status = Some(shrink.result.summary.status);
            let (status, detail, classification) = shrink_step_status(
                run_state.primary_status,
                &shrink.result.summary,
                strict,
                seed_value,
                RunMode::Replay,
                allow_expected_failures,
                Path::new(&shrink.out_trace_path),
            );
            state.shrink_classification = Some(classification);
            state.push("shrink", status, detail);
        }
        Err(err) => {
            state.shrink_classification = Some("tooling_failure".to_string());
            state.abort_due_to_timeout(
                "shrink",
                err.to_string(),
                "Trace shrinking did not finish in time; `fozzy full` stopped with a machine-readable failure."
                    .to_string(),
            );
            return;
        }
    }

    if let Some(min_trace) = run_state.shrunk_trace.as_ref() {
        run_replay_step(
            state,
            config,
            min_trace,
            run_state.shrunk_status,
            strict,
            seed_value,
            "replay_shrunk",
        );
    } else {
        state.push_skipped("replay_shrunk", "shrink output not available");
    }
    if state.should_abort() {
        return;
    }

    run_artifact_surface(state, config, trace, run_state.shrunk_trace.as_ref());
    run_report_surface(state, config, trace, run_state.shrunk_trace.as_ref());
    run_memory_surface(state, config, trace, run_state.shrunk_trace.as_ref());
    run_profile_surface(
        state,
        config,
        trace,
        run_state.shrunk_trace.as_ref(),
        strict,
    );
}

fn run_replay_step(
    state: &mut FullRunState,
    config: &Config,
    trace: &Path,
    expected_status: Option<ExitStatus>,
    strict: bool,
    seed: u64,
    step_name: &str,
) {
    state.start_step(step_name, format!("trace={}", trace.display()));
    match run_with_timeout(step_name, EXECUTION_TIMEOUT, {
        let config = config.clone();
        let trace = trace.to_path_buf();
        move || {
            Ok(fozzy::replay_trace(
                &config,
                TracePath::new(trace),
                &fozzy::ReplayOptions {
                    step: false,
                    until: None,
                    dump_events: false,
                    profile_capture: ProfileCaptureLevel::Baseline,
                    reporter: Reporter::Json,
                },
            )?)
        }
    }) {
        Ok(replay) => {
            let (status, detail) = replay_summary_status(
                expected_status,
                &replay.summary,
                strict,
                seed,
                RunMode::Replay,
            );
            let detail = if step_name == "replay" {
                format!("{detail} run_id={}", replay.summary.identity.run_id)
            } else {
                detail
            };
            state.push(step_name, status, detail);
        }
        Err(err) => state.abort_due_to_timeout(
            step_name,
            err.to_string(),
            format!(
                "Trace replay phase `{step_name}` did not finish in time; `fozzy full` returned a structured failure."
            ),
        ),
    }
}

fn run_artifact_surface(
    state: &mut FullRunState,
    config: &Config,
    trace: &Path,
    shrunk_trace: Option<&PathBuf>,
) {
    let trace_label = trace.display().to_string();
    let _ = fozzy::artifacts_command(
        config,
        &ArtifactCommand::Ls {
            run: trace_label.clone(),
        },
    )
    .map(|output| {
        let (status, detail) = artifacts_list_status(&output, trace);
        state.push("artifacts_ls", status, detail);
    })
    .map_err(|err| state.push("artifacts_ls", FullStepStatus::Failed, err.to_string()));

    let artifacts_export = state.register_temp(
        std::env::temp_dir().join(format!("fozzy-full-artifacts-{}.zip", uuid::Uuid::new_v4())),
    );
    match fozzy::artifacts_command(
        config,
        &ArtifactCommand::Export {
            run: trace_label.clone(),
            out: artifacts_export.clone(),
        },
    ) {
        Ok(_) => {
            let (status, detail) = zip_artifact_status(&artifacts_export);
            state.push("artifacts_export", status, detail);
        }
        Err(err) => state.push("artifacts_export", FullStepStatus::Failed, err.to_string()),
    }

    let artifacts_pack = state.register_temp(
        std::env::temp_dir().join(format!("fozzy-full-pack-{}.zip", uuid::Uuid::new_v4())),
    );
    match fozzy::artifacts_command(
        config,
        &ArtifactCommand::Pack {
            run: trace_label.clone(),
            out: artifacts_pack.clone(),
        },
    ) {
        Ok(_) => {
            let (status, detail) = zip_artifact_status(&artifacts_pack);
            state.push("artifacts_pack", status, detail);
        }
        Err(err) => state.push("artifacts_pack", FullStepStatus::Failed, err.to_string()),
    }

    if let Some(min_trace) = shrunk_trace {
        match fozzy::artifacts_command(
            config,
            &ArtifactCommand::Diff {
                left: trace_label,
                right: min_trace.display().to_string(),
            },
        ) {
            Ok(output) => {
                let (status, detail) = artifacts_diff_status(&output);
                state.push("artifacts_diff", status, detail);
            }
            Err(err) => state.push("artifacts_diff", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        state.push_skipped("artifacts_diff", "requires shrink output");
    }
}

fn run_report_surface(
    state: &mut FullRunState,
    config: &Config,
    trace: &Path,
    shrunk_trace: Option<&PathBuf>,
) {
    let trace_label = trace.display().to_string();
    match fozzy::report_command(
        config,
        &ReportCommand::Show {
            run: trace_label.clone(),
            format: Reporter::Pretty,
        },
    ) {
        Ok(value) => {
            let (status, detail) = report_show_status(&value);
            state.push("report_show", status, detail);
        }
        Err(err) => state.push("report_show", FullStepStatus::Failed, err.to_string()),
    }

    match fozzy::report_command(
        config,
        &ReportCommand::Query {
            run: trace_label.clone(),
            path_expr: Some(".status".to_string()),
            list_paths: false,
        },
    ) {
        Ok(value) => {
            let (status, detail) = report_query_status(&value);
            state.push("report_query", status, detail);
        }
        Err(err) => state.push("report_query", FullStepStatus::Failed, err.to_string()),
    }

    match fozzy::report_command(
        config,
        &ReportCommand::Query {
            run: trace_label.clone(),
            path_expr: None,
            list_paths: true,
        },
    ) {
        Ok(value) => {
            let (status, detail) = report_query_paths_status(&value);
            state.push("report_query_paths", status, detail);
        }
        Err(err) => state.push(
            "report_query_paths",
            FullStepStatus::Failed,
            err.to_string(),
        ),
    }

    if let Some(min_trace) = shrunk_trace {
        match fozzy::report_command(
            config,
            &ReportCommand::Flaky {
                runs: vec![trace_label, min_trace.display().to_string()],
                flake_budget: None,
            },
        ) {
            Ok(value) => {
                let (status, detail) = flaky_report_status(&value);
                state.push("report_flaky", status, detail);
            }
            Err(err) => state.push("report_flaky", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        state.push_skipped("report_flaky", "requires second trace input");
    }
}

fn run_memory_surface(
    state: &mut FullRunState,
    config: &Config,
    trace: &Path,
    shrunk_trace: Option<&PathBuf>,
) {
    let trace_label = trace.display().to_string();
    match fozzy::memory_command(
        config,
        &MemoryCommand::Top {
            run: trace_label.clone(),
            limit: 10,
        },
    ) {
        Ok(value) => {
            let (status, detail) = memory_top_status(&value);
            state.push("memory_top", status, detail);
        }
        Err(err) => state.push("memory_top", FullStepStatus::Failed, err.to_string()),
    }

    match fozzy::memory_command(
        config,
        &MemoryCommand::Graph {
            run: trace_label.clone(),
            out: None,
        },
    ) {
        Ok(value) => {
            let (status, detail) = memory_graph_status(&value);
            state.push("memory_graph", status, detail);
        }
        Err(err) => state.push("memory_graph", FullStepStatus::Failed, err.to_string()),
    }

    if let Some(min_trace) = shrunk_trace {
        match fozzy::memory_command(
            config,
            &MemoryCommand::Diff {
                left: trace_label,
                right: min_trace.display().to_string(),
            },
        ) {
            Ok(value) => {
                let (status, detail) = memory_diff_status(&value);
                state.push("memory_diff", status, detail);
            }
            Err(err) => state.push("memory_diff", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        state.push_skipped("memory_diff", "requires second trace input");
    }
}

fn run_profile_surface(
    state: &mut FullRunState,
    config: &Config,
    trace: &Path,
    shrunk_trace: Option<&PathBuf>,
    strict: bool,
) {
    let trace_label = trace.display().to_string();
    match fozzy::profile_command(
        config,
        &ProfileCommand::Top {
            run: trace_label.clone(),
            cpu: false,
            heap: true,
            latency: true,
            io: true,
            sched: true,
            limit: 10,
        },
        strict,
    ) {
        Ok(value) => {
            let (status, detail) = profile_top_status(&value);
            state.push("profile_top", status, detail);
        }
        Err(err) => state.push("profile_top", FullStepStatus::Failed, err.to_string()),
    }

    if let Some(min_trace) = shrunk_trace {
        match fozzy::profile_command(
            config,
            &ProfileCommand::Diff {
                left: trace_label.clone(),
                right: min_trace.display().to_string(),
                cpu: false,
                heap: true,
                latency: true,
                io: true,
                sched: true,
            },
            strict,
        ) {
            Ok(value) => {
                let (status, detail) = profile_diff_status(&value, false);
                state.push("profile_diff", status, detail);
            }
            Err(err) => state.push("profile_diff", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        state.push_skipped("profile_diff", "requires second trace input");
    }

    match fozzy::profile_command(
        config,
        &ProfileCommand::Explain {
            run: trace_label,
            diff_with: shrunk_trace.map(|path| path.display().to_string()),
        },
        strict,
    ) {
        Ok(value) => {
            let (status, detail) = profile_explain_status(&value);
            state.push("profile_explain", status, detail);
        }
        Err(err) => state.push("profile_explain", FullStepStatus::Failed, err.to_string()),
    }
}
