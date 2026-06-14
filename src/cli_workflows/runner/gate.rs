use super::*;

pub(crate) fn clean_tree_step_status(check: &GitWorktreeCheck) -> (FullStepStatus, String) {
    match check {
        GitWorktreeCheck::Clean => (FullStepStatus::Passed, "git worktree clean".to_string()),
        GitWorktreeCheck::Dirty {
            change_count,
            preview,
        } => (
            FullStepStatus::Advisory,
            format!(
                "git worktree is dirty ({} change(s)); advisory only; example: {}",
                change_count, preview
            ),
        ),
        GitWorktreeCheck::NotGitRepo => (
            FullStepStatus::Skipped,
            "git worktree check skipped: not a git repository".to_string(),
        ),
    }
}

pub(crate) fn run_gate_command(
    config: &Config,
    profile: GateProfile,
    scenario_root: &Path,
    scopes: &[String],
    seed: Option<u64>,
    doctor_runs: u32,
    strict: bool,
) -> anyhow::Result<GateReport> {
    let seed = Some(resolved_workflow_seed(seed));
    let mut steps = Vec::<FullStepResult>::new();
    let mut push = |name: &str, status: FullStepStatus, detail: String| {
        steps.push(FullStepResult {
            name: name.to_string(),
            status,
            detail,
        });
    };

    if strict {
        match git_clean_tree_check() {
            Ok(check) => {
                let (status, detail) = clean_tree_step_status(&check);
                push("clean_tree", status, detail);
            }
            Err(err) => push("clean_tree", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        push(
            "clean_tree",
            FullStepStatus::Skipped,
            "strict disabled; git worktree check skipped".to_string(),
        );
    }

    let discovered = discover_scenarios(scenario_root);
    if !discovered.parse_errors.is_empty() || discovered.steps.is_empty() {
        push(
            "discover",
            FullStepStatus::Failed,
            if !discovered.parse_errors.is_empty() {
                format!("parse_errors={}", discovered.parse_errors.join(" | "))
            } else {
                format!(
                    "step_scenarios={} distributed_scenarios={}",
                    discovered.steps.len(),
                    discovered.distributed.len()
                )
            },
        );
    } else {
        push(
            "discover",
            FullStepStatus::Passed,
            format!(
                "step_scenarios={} distributed_scenarios={}",
                discovered.steps.len(),
                discovered.distributed.len()
            ),
        );
    }

    let scope_tokens: Vec<String> = scopes
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    let mut targets: Vec<PathBuf> = discovered
        .steps
        .iter()
        .filter(|p| {
            if scope_tokens.is_empty() {
                return true;
            }
            let key = p.to_string_lossy().to_ascii_lowercase();
            scope_tokens.iter().any(|token| key.contains(token))
        })
        .cloned()
        .collect();
    targets.sort();
    let matched_scenarios: Vec<String> = targets
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    if targets.is_empty() {
        push(
            "scope_match",
            FullStepStatus::Failed,
            "no step scenarios matched requested scope".to_string(),
        );
        return Ok(GateReport {
            schema_version: "fozzy.gate_report.v1".to_string(),
            profile,
            strict,
            scenario_root: scenario_root.display().to_string(),
            scopes: scope_tokens,
            matched_scenarios,
            steps,
        });
    }
    push(
        "scope_match",
        FullStepStatus::Passed,
        format!("matched={}", targets.len()),
    );

    let primary = targets
        .iter()
        .find(|p| is_preferred_step_scenario(p))
        .cloned()
        .unwrap_or_else(|| targets[0].clone());

    let memory = MemoryOptions {
        track: true,
        limit_mb: config.mem_limit_mb,
        fail_after_allocs: config.mem_fail_after,
        fail_on_leak: config.fail_on_leak,
        leak_budget_bytes: config.leak_budget,
        artifacts: true,
        fragmentation_seed: config.mem_fragmentation_seed,
        pressure_wave: config.mem_pressure_wave.clone(),
    };

    match fozzy::doctor(
        config,
        &fozzy::DoctorOptions {
            deep: true,
            scenario: Some(ScenarioPath::new(primary.clone())),
            runs: doctor_runs.max(2),
            seed,
        },
    ) {
        Ok(report) => {
            let (status, detail) = doctor_report_status(
                &report,
                strict,
                primary.as_path(),
                doctor_runs.max(2),
                resolved_workflow_seed(seed),
            );
            push("doctor_deep", status, detail);
        }
        Err(err) => push("doctor_deep", FullStepStatus::Failed, err.to_string()),
    }

    let test_globs: Vec<String> = targets
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    match fozzy::run_tests(
        config,
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
            memory: memory.clone(),
        },
    ) {
        Ok(test) => {
            let (status, detail) = run_summary_pass_status(
                &test.summary,
                strict,
                resolved_workflow_seed(seed),
                RunMode::Test,
            );
            push(
                "test_det_strict",
                status,
                format!("{detail} run_id={}", test.summary.identity.run_id),
            );
        }
        Err(err) => push("test_det_strict", FullStepStatus::Failed, err.to_string()),
    }

    let trace_path = std::env::temp_dir().join(format!(
        "fozzy-gate-{}-{}.trace.fozzy",
        profile_string(profile),
        uuid::Uuid::new_v4()
    ));
    let mut primary_status: Option<ExitStatus> = None;
    let mut replay_run_id: Option<String> = None;
    match fozzy::run_scenario(
        config,
        ScenarioPath::new(primary.clone()),
        &RunOptions {
            det: true,
            seed,
            timeout: None,
            reporter: Reporter::Json,
            record_trace_to: Some(trace_path.clone()),
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
    ) {
        Ok(run) => {
            primary_status = Some(run.summary.status);
            let (status, detail) = recorded_trace_status(
                &run.summary,
                strict,
                resolved_workflow_seed(seed),
                RunMode::Run,
                &trace_path,
            );
            push("run_record_trace", status, detail);
        }
        Err(err) => push("run_record_trace", FullStepStatus::Failed, err.to_string()),
    }

    if trace_path.exists() {
        match fozzy::verify_trace_file(&trace_path) {
            Ok(verify) => {
                let strict_ok = !strict
                    || (verify.checksum_present
                        && verify.checksum_valid
                        && verify.warnings.is_empty());
                push(
                    "trace_verify",
                    if verify.ok && strict_ok {
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
            Err(err) => push("trace_verify", FullStepStatus::Failed, err.to_string()),
        }

        match fozzy::replay_trace(
            config,
            TracePath::new(trace_path.clone()),
            &fozzy::ReplayOptions {
                step: false,
                until: None,
                dump_events: false,
                profile_capture: ProfileCaptureLevel::Baseline,
                reporter: Reporter::Json,
            },
        ) {
            Ok(replay) => {
                replay_run_id = Some(replay.summary.identity.run_id.clone());
                let (status, detail) = replay_summary_status(
                    primary_status,
                    &replay.summary,
                    strict,
                    resolved_workflow_seed(seed),
                    RunMode::Replay,
                );
                push("replay", status, detail);
            }
            Err(err) => push("replay", FullStepStatus::Failed, err.to_string()),
        }

        let ci = fozzy::ci_evaluate(
            config,
            &CiOptions {
                trace: trace_path.clone(),
                flake_runs: Vec::new(),
                flake_budget_pct: None,
                perf_baseline: None,
                max_p99_delta_pct: None,
                strict,
            },
        );
        match ci {
            Ok(report) => {
                let (status, detail) = ci_report_status(&report);
                push("ci", status, detail);
            }
            Err(err) => push("ci", FullStepStatus::Failed, err.to_string()),
        }

        match fozzy::profile_command(
            config,
            &ProfileCommand::Top {
                run: trace_path.display().to_string(),
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
                push("profile_top", status, detail)
            }
            Err(err) => push("profile_top", FullStepStatus::Failed, err.to_string()),
        }
        if let Some(replay_run_id) = replay_run_id.as_ref() {
            match fozzy::profile_command(
                config,
                &ProfileCommand::Diff {
                    left: trace_path.display().to_string(),
                    right: replay_run_id.clone(),
                    cpu: false,
                    heap: true,
                    latency: true,
                    io: true,
                    sched: true,
                },
                strict,
            ) {
                Ok(value) => {
                    let (status, detail) = profile_diff_status(&value, true);
                    push("profile_diff", status, detail)
                }
                Err(err) => push("profile_diff", FullStepStatus::Failed, err.to_string()),
            }
        } else {
            push(
                "profile_diff",
                FullStepStatus::Skipped,
                "replay run id unavailable".to_string(),
            );
        }
        match fozzy::profile_command(
            config,
            &ProfileCommand::Explain {
                run: trace_path.display().to_string(),
                diff_with: replay_run_id.clone(),
            },
            strict,
        ) {
            Ok(value) => {
                let (status, detail) = profile_explain_status(&value);
                push("profile_explain", status, detail)
            }
            Err(err) => push("profile_explain", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        for name in [
            "trace_verify",
            "replay",
            "ci",
            "profile_top",
            "profile_diff",
            "profile_explain",
        ] {
            push(
                name,
                FullStepStatus::Skipped,
                "trace was not recorded".to_string(),
            );
        }
    }

    let report = GateReport {
        schema_version: "fozzy.gate_report.v1".to_string(),
        profile,
        strict,
        scenario_root: scenario_root.display().to_string(),
        scopes: scope_tokens,
        matched_scenarios,
        steps,
    };
    let _ = std::fs::remove_file(&trace_path);
    Ok(report)
}

fn profile_string(profile: GateProfile) -> &'static str {
    match profile {
        GateProfile::Targeted => "targeted",
    }
}
