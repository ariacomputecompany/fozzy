use std::process::ExitCode;

use super::*;

pub(super) fn run_command(
    cli: &Cli,
    config: &Config,
    logger: &CliLogger,
) -> anyhow::Result<ExitCode> {
    let proc_backend = cli.proc_backend.unwrap_or(config.proc_backend);
    let fs_backend = cli.fs_backend.unwrap_or(config.fs_backend);
    let http_backend = cli.http_backend.unwrap_or(config.http_backend);
    match &cli.command {
        Command::Init {
            force,
            template,
            with,
            all_tests,
        } => {
            let init_types = selected_init_test_types(with, *all_tests);
            fozzy::init_project(
                config,
                &cli.config,
                &InitTemplate::from_option(template.as_ref()),
                *force,
                &init_types,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Test {
            globs,
            det,
            seed,
            jobs,
            timeout,
            filter,
            reporter,
            record,
            fail_fast,
            record_collision,
            mem_track,
            mem_limit_mb,
            mem_fail_after,
            mem_fragmentation_seed,
            mem_pressure_wave,
            fail_on_leak,
            leak_budget,
        } => {
            let memory = resolve_memory_options(
                config,
                *mem_track,
                false,
                *mem_limit_mb,
                *mem_fail_after,
                *mem_fragmentation_seed,
                mem_pressure_wave.clone(),
                *fail_on_leak,
                *leak_budget,
            );
            let run = fozzy::run_tests(
                config,
                globs,
                &RunOptions {
                    det: *det,
                    seed: *seed,
                    timeout: timeout.map(|d| d.0),
                    reporter: (*reporter).into(),
                    record_trace_to: record.clone(),
                    filter: filter.clone(),
                    jobs: *jobs,
                    fail_fast: *fail_fast,
                    record_collision: *record_collision,
                    profile_capture: fozzy::ProfileCaptureLevel::Baseline,
                    proc_backend,
                    fs_backend,
                    http_backend,
                    memory,
                },
            )?;
            logger.print_run_summary(&run.summary)?;
            enforce_strict_run(cli, &run.summary)?;
            Ok(exit_code_for_status(run.summary.status))
        }
        Command::Run {
            scenario,
            det,
            seed,
            timeout,
            reporter,
            record,
            record_collision,
            mem_track,
            mem_limit_mb,
            mem_fail_after,
            mem_fragmentation_seed,
            mem_pressure_wave,
            fail_on_leak,
            leak_budget,
            mem_artifacts,
            profile_capture,
        } => {
            let memory = resolve_memory_options(
                config,
                *mem_track,
                *mem_artifacts,
                *mem_limit_mb,
                *mem_fail_after,
                *mem_fragmentation_seed,
                mem_pressure_wave.clone(),
                *fail_on_leak,
                *leak_budget,
            );
            let run = fozzy::run_scenario(
                config,
                ScenarioPath::new(scenario.clone()),
                &RunOptions {
                    det: *det,
                    seed: *seed,
                    timeout: timeout.map(|d| d.0),
                    reporter: (*reporter).into(),
                    record_trace_to: record.clone(),
                    filter: None,
                    jobs: None,
                    fail_fast: false,
                    record_collision: *record_collision,
                    profile_capture: *profile_capture,
                    proc_backend,
                    fs_backend,
                    http_backend,
                    memory,
                },
            )?;
            logger.print_run_summary(&run.summary)?;
            enforce_strict_run(cli, &run.summary)?;
            Ok(exit_code_for_status(run.summary.status))
        }
        Command::Fuzz {
            target,
            det,
            mode,
            seed,
            time,
            runs,
            max_input,
            corpus,
            mutator,
            shrink,
            record,
            reporter,
            crash_only,
            minimize,
            record_collision,
            mem_track,
            mem_limit_mb,
            mem_fail_after,
            mem_fragmentation_seed,
            mem_pressure_wave,
            fail_on_leak,
            leak_budget,
            mem_artifacts,
            profile_capture,
        } => {
            let memory = resolve_memory_options(
                config,
                *mem_track,
                *mem_artifacts,
                *mem_limit_mb,
                *mem_fail_after,
                *mem_fragmentation_seed,
                mem_pressure_wave.clone(),
                *fail_on_leak,
                *leak_budget,
            );
            let target: FuzzTarget = target.parse()?;
            let run = fozzy::fuzz(
                config,
                &target,
                &FuzzOptions {
                    det: *det,
                    mode: *mode,
                    seed: *seed,
                    time: time.map(|d| d.0),
                    runs: *runs,
                    max_input_bytes: *max_input,
                    corpus_dir: corpus.clone(),
                    mutator: mutator.clone(),
                    shrink: *shrink,
                    record_trace_to: record.clone(),
                    reporter: (*reporter).into(),
                    crash_only: *crash_only,
                    minimize: *minimize,
                    record_collision: *record_collision,
                    profile_capture: *profile_capture,
                    memory,
                },
            )?;
            logger.print_run_summary(&run.summary)?;
            enforce_strict_run(cli, &run.summary)?;
            Ok(exit_code_for_status(run.summary.status))
        }
        Command::Explore {
            scenario,
            seed,
            time,
            steps,
            nodes,
            faults,
            schedule,
            checker,
            record,
            shrink,
            minimize,
            reporter,
            record_collision,
            mem_track,
            mem_limit_mb,
            mem_fail_after,
            mem_fragmentation_seed,
            mem_pressure_wave,
            fail_on_leak,
            leak_budget,
            mem_artifacts,
            profile_capture,
        } => {
            let memory = resolve_memory_options(
                config,
                *mem_track,
                *mem_artifacts,
                *mem_limit_mb,
                *mem_fail_after,
                *mem_fragmentation_seed,
                mem_pressure_wave.clone(),
                *fail_on_leak,
                *leak_budget,
            );
            let run = fozzy::explore(
                config,
                ScenarioPath::new(scenario.clone()),
                &ExploreOptions {
                    seed: *seed,
                    time: time.map(|d| d.0),
                    steps: *steps,
                    nodes: *nodes,
                    faults: faults.clone(),
                    schedule: *schedule,
                    checker: checker.clone(),
                    record_trace_to: record.clone(),
                    shrink: *shrink,
                    minimize: *minimize,
                    reporter: (*reporter).into(),
                    record_collision: *record_collision,
                    profile_capture: *profile_capture,
                    memory,
                },
            )?;
            logger.print_run_summary(&run.summary)?;
            enforce_strict_run(cli, &run.summary)?;
            Ok(exit_code_for_status(run.summary.status))
        }
        Command::Replay {
            trace,
            step,
            until,
            dump_events,
            profile_capture,
            profile_regen,
            profile_export_format,
            profile_export_out,
            reporter,
        } => {
            let run = fozzy::replay_trace(
                config,
                TracePath::new(trace.clone()),
                &fozzy::ReplayOptions {
                    step: *step,
                    until: until.map(|d| d.0),
                    dump_events: *dump_events,
                    profile_capture: if *profile_regen {
                        ProfileCaptureLevel::Full
                    } else {
                        *profile_capture
                    },
                    reporter: (*reporter).into(),
                },
            )?;
            if let (Some(format), Some(out)) = (profile_export_format, profile_export_out.as_ref())
            {
                let export = fozzy::profile_command(
                    config,
                    &ProfileCommand::Export {
                        run: run.summary.identity.run_id.clone(),
                        format: *format,
                        out: out.clone(),
                    },
                    strict_enabled(cli),
                )?;
                logger.print_serialized(&serde_json::json!({
                    "run": run.summary,
                    "profileExport": export
                }))?;
            } else {
                logger.print_run_summary(&run.summary)?;
            }
            enforce_strict_run(cli, &run.summary)?;
            Ok(exit_code_for_status(run.summary.status))
        }
        Command::Trace { command } => {
            match command {
                TraceCommand::Verify { path } => {
                    let out = fozzy::verify_trace_file(path)?;
                    if strict_enabled(cli)
                        && (!out.checksum_present
                            || !out.checksum_valid
                            || !out.warnings.is_empty())
                    {
                        let mut reasons = Vec::new();
                        if !out.checksum_present {
                            reasons.push("checksum missing".to_string());
                        }
                        if !out.checksum_valid {
                            reasons.push("checksum invalid".to_string());
                        }
                        if !out.warnings.is_empty() {
                            reasons.push(format!("warnings: {}", out.warnings.join("; ")));
                        }
                        return Err(anyhow::anyhow!(
                            "strict mode: trace verify failed integrity policy ({})",
                            reasons.join(", ")
                        ));
                    }
                    logger.print_serialized(&out)?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Shrink {
            trace,
            out,
            budget,
            aggressive,
            minimize,
            reporter: _,
        } => {
            let result = fozzy::shrink_trace(
                config,
                TracePath::new(trace.clone()),
                &fozzy::ShrinkOptions {
                    out_trace_path: out.clone(),
                    budget: budget.map(|d| d.0),
                    aggressive: *aggressive,
                    minimize: *minimize,
                },
            )?;
            logger.print_run_summary(&result.result.summary)?;
            enforce_strict_run(cli, &result.result.summary)?;
            Ok(exit_code_for_status(result.result.summary.status))
        }
        Command::Corpus { command } => {
            let out = fozzy::corpus_command(config, command)?;
            logger.print_serialized(&out)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Artifacts { command } => {
            let out = fozzy::artifacts_command(config, command)?;
            logger.print_serialized(&out)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Report { command } => {
            let out = fozzy::report_command(config, command)?;
            logger.print_serialized(&out)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Memory { command } => {
            let out = fozzy::memory_command(config, command)?;
            logger.print_serialized(&out)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Profile { command } => {
            let out = fozzy::profile_command(config, command, strict_enabled(cli))?;
            logger.print_serialized(&out)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Map { command } => {
            let out = fozzy::map_command(config, command)?;
            logger.print_serialized(&out)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Doctor {
            deep,
            scenario,
            runs,
            seed,
        } => {
            let report = fozzy::doctor(
                config,
                &fozzy::DoctorOptions {
                    deep: *deep,
                    scenario: scenario.clone().map(ScenarioPath::new),
                    runs: *runs,
                    seed: *seed,
                },
            )?;
            logger.print_serialized(&report)?;
            if strict_enabled(cli) {
                let mut reasons = Vec::new();
                if !report.issues.is_empty() {
                    reasons.push(format!("{} issue(s)", report.issues.len()));
                }
                if let Some(signals) = &report.nondeterminism_signals
                    && !signals.is_empty()
                {
                    reasons.push(format!("{} nondeterminism signal(s)", signals.len()));
                }
                if !reasons.is_empty() {
                    return Err(anyhow::anyhow!(
                        "strict mode: doctor reported {}",
                        reasons.join(" and ")
                    ));
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Env => {
            let info = fozzy::env_info(config);
            logger.print_serialized(&info)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Ci {
            trace,
            flake_runs,
            flake_budget,
            perf_baseline,
            max_p99_delta_pct,
        } => {
            let out = fozzy::ci_evaluate(
                config,
                &CiOptions {
                    trace: trace.clone(),
                    flake_runs: flake_runs.clone(),
                    flake_budget_pct: *flake_budget,
                    perf_baseline: perf_baseline.clone(),
                    max_p99_delta_pct: *max_p99_delta_pct,
                    strict: strict_enabled(cli),
                },
            )?;
            logger.print_serialized(&out)?;
            Ok(if out.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Command::Gate {
            profile,
            scenario_root,
            scope,
            seed,
            doctor_runs,
        } => {
            let report = cli_workflows::run_gate_command(
                config,
                *profile,
                scenario_root,
                scope,
                *seed,
                *doctor_runs,
                strict_enabled(cli),
            )?;
            let has_failed = report
                .steps
                .iter()
                .any(|s| matches!(s.status, FullStepStatus::Failed));
            logger.print_serialized(&report)?;
            Ok(if has_failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }
        Command::Version => {
            let info = fozzy::version_info();
            logger.print_serialized(&info)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Usage => {
            let doc = fozzy::usage_doc();
            logger.print_usage(&doc)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Schema => {
            let doc = fozzy::schema_doc();
            logger.print_serialized(&doc)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Validate { scenario } => {
            let scenario_path = ScenarioPath::new(scenario.clone());
            let out = match fozzy::Scenario::load_file(&scenario_path) {
                Ok(fozzy::ScenarioFile::Steps(steps)) => {
                    let loaded = fozzy::Scenario {
                        name: steps.name.clone(),
                        steps: steps.steps.clone(),
                    };
                    match loaded.validate() {
                        Ok(()) => serde_json::json!({
                            "ok": true,
                            "scenario": scenario.display().to_string(),
                            "variant": "steps",
                            "name": loaded.name,
                            "steps": loaded.steps.len()
                        }),
                        Err(err) => serde_json::json!({
                            "ok": false,
                            "scenario": scenario.display().to_string(),
                            "variant": "steps",
                            "error": err.to_string()
                        }),
                    }
                }
                Ok(fozzy::ScenarioFile::Distributed(dist)) => match dist.validate() {
                    Ok(()) => serde_json::json!({
                        "ok": true,
                        "scenario": scenario.display().to_string(),
                        "variant": "distributed",
                        "name": dist.name,
                        "steps": dist.distributed.steps.len(),
                        "invariants": dist.distributed.invariants.len()
                    }),
                    Err(err) => serde_json::json!({
                        "ok": false,
                        "scenario": scenario.display().to_string(),
                        "variant": "distributed",
                        "error": err.to_string()
                    }),
                },
                Ok(fozzy::ScenarioFile::Suites(suites)) => serde_json::json!({
                    "ok": false,
                    "scenario": scenario.display().to_string(),
                    "variant": "suites",
                    "error": format!(
                        "scenario file {} uses `suites` without an executable step DSL (v0.1 only supports `steps` or `distributed` for execution)",
                        scenario.display()
                    ),
                    "name": suites.name
                }),
                Err(err) => serde_json::json!({
                    "ok": false,
                    "scenario": scenario.display().to_string(),
                    "error": err.to_string()
                }),
            };
            logger.print_serialized(&out)?;
            Ok(if out.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(2)
            })
        }
        Command::Full {
            scenario_root,
            seed,
            doctor_runs,
            fuzz_time,
            explore_steps,
            explore_nodes,
            allow_expected_failures,
            scenario_filter,
            skip_steps,
            required_steps,
            require_topology_coverage,
            topology_min_risk,
            topology_profile,
            topology_shrink_policy,
        } => {
            let report = cli_workflows::run_full_command(
                config,
                scenario_root,
                *seed,
                *doctor_runs,
                fuzz_time.0,
                *explore_steps,
                *explore_nodes,
                strict_enabled(cli),
                cli.unsafe_mode,
                *allow_expected_failures,
                scenario_filter.as_deref(),
                skip_steps,
                required_steps,
                require_topology_coverage.as_deref(),
                *topology_min_risk,
                *topology_profile,
                *topology_shrink_policy,
            )?;
            let has_failed = report
                .steps
                .iter()
                .any(|s| matches!(s.status, FullStepStatus::Failed));
            logger.print_serialized(&report)?;
            Ok(if has_failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }
    }
}
