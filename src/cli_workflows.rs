use super::*;

fn summarize_profile_top(value: &serde_json::Value) -> String {
    let warnings = value
        .get("warnings")
        .and_then(|v| v.as_array())
        .map(|items| {
            let rows = items
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>();
            if rows.is_empty() {
                "<none>".to_string()
            } else {
                rows.join("; ")
            }
        })
        .unwrap_or_else(|| "<none>".to_string());
    let empty_domains = value
        .get("emptyDomains")
        .and_then(|v| v.as_array())
        .map(|items| {
            let rows = items
                .iter()
                .filter_map(|item| {
                    let domain = item.get("domain").and_then(|v| v.as_str())?;
                    let reason = item.get("reason").and_then(|v| v.as_str())?;
                    Some(format!("{domain}:{reason}"))
                })
                .collect::<Vec<_>>();
            if rows.is_empty() {
                "<none>".to_string()
            } else {
                rows.join("; ")
            }
        })
        .unwrap_or_else(|| "<none>".to_string());
    format!("warnings={warnings} empty_domains={empty_domains}")
}

fn profile_top_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let warning_count = value
        .get("warnings")
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);
    let empty_count = value
        .get("emptyDomains")
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);
    (
        if warning_count > 0 || empty_count > 0 {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        summarize_profile_top(value),
    )
}

fn profile_diff_status(
    value: &serde_json::Value,
    require_stable: bool,
) -> (FullStepStatus, String) {
    let verdict = value
        .pointer("/summary/verdict")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let regressions = value
        .pointer("/summary/regressionCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let significant = value
        .pointer("/summary/significantRegressionCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let status = if significant > 0 || regressions > 0 || (require_stable && verdict != "stable") {
        FullStepStatus::Failed
    } else {
        FullStepStatus::Passed
    };
    (
        status,
        format!(
            "verdict={} regressions={} significant_regressions={}",
            verdict, regressions, significant
        ),
    )
}

fn profile_explain_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let cause_domain = value
        .get("likelyCauseDomain")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let shifted_path = value
        .get("topShiftedPath")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let regression_statement = value
        .get("regressionStatement")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let status = if cause_domain == "unknown"
        || shifted_path == "n/a"
        || regression_statement == "no measurable regression shift found"
    {
        FullStepStatus::Skipped
    } else {
        FullStepStatus::Passed
    };
    (
        status,
        format!("cause_domain={} shifted_path={}", cause_domain, shifted_path),
    )
}

fn flaky_report_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let run_count = value.get("runCount").and_then(|v| v.as_u64()).unwrap_or(0);
    let is_flaky = value
        .get("isFlaky")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let flake_rate = value
        .get("flakeRatePct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    (
        if is_flaky {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        format!("run_count={} is_flaky={} flake_rate_pct={}", run_count, is_flaky, flake_rate),
    )
}

fn memory_top_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let total = value.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
    let shown = value
        .get("leaks")
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);
    (
        if total > 0 {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        format!("total_leaks={} shown={}", total, shown),
    )
}

fn memory_diff_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let leaked = value
        .get("deltaLeakedBytes")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let peak = value
        .get("deltaPeakBytes")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    (
        if leaked != 0 || peak != 0 {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        format!("delta_leaked_bytes={} delta_peak_bytes={}", leaked, peak),
    )
}

fn replay_summary_status(
    expected: Option<ExitStatus>,
    summary: &RunSummary,
    strict: bool,
) -> (FullStepStatus, String) {
    let class_ok = expected
        .map(|s| (s == ExitStatus::Pass) == (summary.status == ExitStatus::Pass))
        .unwrap_or(false);
    let strict_ok = enforce_strict_summary(strict, summary).is_ok();
    (
        if class_ok && strict_ok {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "status={:?} class_ok={} strict_ok={}",
            summary.status, class_ok, strict_ok
        ),
    )
}

fn file_artifact_status(path: &Path) -> (FullStepStatus, String) {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() > 0 => (
            FullStepStatus::Passed,
            format!("path={} bytes={}", path.display(), metadata.len()),
        ),
        Ok(metadata) if metadata.is_file() => (
            FullStepStatus::Failed,
            format!("path={} bytes=0", path.display()),
        ),
        Ok(_) => (
            FullStepStatus::Failed,
            format!("path={} is not a file", path.display()),
        ),
        Err(err) => (
            FullStepStatus::Failed,
            format!("path={} missing: {err}", path.display()),
        ),
    }
}

fn clean_tree_step_status(detail: &str) -> FullStepStatus {
    if detail.contains("check skipped") {
        FullStepStatus::Skipped
    } else {
        FullStepStatus::Passed
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_gate_command(
    config: &Config,
    profile: GateProfile,
    scenario_root: &Path,
    scopes: &[String],
    seed: Option<u64>,
    doctor_runs: u32,
    strict: bool,
) -> anyhow::Result<GateReport> {
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
            Ok(detail) => push("clean_tree", clean_tree_step_status(&detail), detail),
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
    if !discovered.parse_errors.is_empty() {
        push(
            "discover",
            FullStepStatus::Failed,
            format!("parse_errors={}", discovered.parse_errors.join(" | ")),
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
            let policy_ok = !strict
                || (report.issues.is_empty()
                    && report
                        .nondeterminism_signals
                        .as_ref()
                        .map_or(true, |signals| signals.is_empty()));
            push(
                "doctor_deep",
                if report.ok && policy_ok {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                format!(
                    "ok={} policy_ok={} scenario={}",
                    report.ok,
                    policy_ok,
                    primary.display()
                ),
            );
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
            let strict_ok = enforce_strict_summary(strict, &test.summary).is_ok();
            push(
                "test_det_strict",
                if strict_ok {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                format!(
                    "status={:?} strict_ok={} run_id={}",
                    test.summary.status, strict_ok, test.summary.identity.run_id
                ),
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
            let strict_ok = enforce_strict_summary(strict, &run.summary).is_ok();
            push(
                "run_record_trace",
                if run.summary.identity.trace_path.is_some() && strict_ok {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                format!(
                    "status={:?} strict_ok={} trace={}",
                    run.summary.status,
                    strict_ok,
                    trace_path.display()
                ),
            );
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
                let (status, detail) =
                    replay_summary_status(primary_status, &replay.summary, strict);
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
            Ok(report) => push(
                "ci",
                if report.ok {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                format!("ok={} checks={}", report.ok, report.checks.len()),
            ),
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

    Ok(GateReport {
        schema_version: "fozzy.gate_report.v1".to_string(),
        profile,
        strict,
        scenario_root: scenario_root.display().to_string(),
        scopes: scope_tokens,
        matched_scenarios,
        steps,
    })
}

fn profile_string(profile: GateProfile) -> &'static str {
    match profile {
        GateProfile::Targeted => "targeted",
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_full_command(
    config: &Config,
    scenario_root: &Path,
    seed: Option<u64>,
    doctor_runs: u32,
    fuzz_time: std::time::Duration,
    explore_steps: u64,
    explore_nodes: usize,
    strict: bool,
    unsafe_mode: bool,
    allow_expected_failures: bool,
    scenario_filter: Option<&str>,
    skip_steps: &[String],
    required_steps: &[String],
    require_topology_coverage: Option<&Path>,
    topology_min_risk: u8,
    topology_profile: TopologyProfile,
    topology_shrink_policy: ShrinkCoveragePolicy,
) -> anyhow::Result<FullReport> {
    let mut temp_paths = Vec::<PathBuf>::new();
    let mut register_temp = |path: PathBuf| -> PathBuf {
        temp_paths.push(path.clone());
        path
    };
    let mut steps = Vec::<FullStepResult>::new();
    let mut push = |name: &str, status: FullStepStatus, detail: String| {
        steps.push(FullStepResult {
            name: name.to_string(),
            status,
            detail,
        });
    };
    let mut shrink_classification: Option<String> = None;

    if strict {
        match git_clean_tree_check() {
            Ok(detail) => push("clean_tree", clean_tree_step_status(&detail), detail),
            Err(err) => push("clean_tree", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        push(
            "clean_tree",
            FullStepStatus::Skipped,
            "strict disabled; git worktree check skipped".to_string(),
        );
    }

    let mut guidance = vec![
        "Use the entire command surface by default; skip only when required inputs for a command are genuinely missing."
            .to_string(),
        "Keep strict mode enabled (default) so warning-class signals fail fast; use --unsafe only for intentional relaxed passes."
            .to_string(),
        "Place executable scenarios under tests/**/*.fozzy.json; distributed scenarios should use the `distributed` schema."
            .to_string(),
    ];
    if let Some(conflict) = full_policy_conflict_details(
        skip_steps,
        required_steps,
        require_topology_coverage.is_some(),
    ) {
        push("policy_conflict", FullStepStatus::Failed, conflict);
    }

    let usage = fozzy::usage_doc();
    push(
        "usage",
        if usage.items.is_empty() {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        format!("items={}", usage.items.len()),
    );
    let version = fozzy::version_info();
    push(
        "version",
        FullStepStatus::Passed,
        format!("version={}", version.version),
    );

    let init_tmp = register_temp(
        std::env::temp_dir().join(format!("fozzy-full-init-{}", uuid::Uuid::new_v4())),
    );
    let init_status = (|| -> anyhow::Result<String> {
        std::fs::create_dir_all(&init_tmp)?;
        let prev = std::env::current_dir()?;
        std::env::set_current_dir(&init_tmp)?;
        let cfg = Config::load_optional_checked(Path::new("fozzy.toml"))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let init_res = fozzy::init_project(
            &cfg,
            Path::new("fozzy.toml"),
            &InitTemplate::Rust,
            true,
            &selected_init_test_types(&[], true),
        );
        let restore_res = std::env::set_current_dir(prev);
        if let Err(err) = restore_res {
            return Err(anyhow::anyhow!(
                "failed to restore cwd after init check: {err}"
            ));
        }
        init_res?;
        let example = init_tmp.join("tests/example.fozzy.json");
        if !example.exists() {
            return Err(anyhow::anyhow!(
                "expected init scaffold missing: {}",
                example.display()
            ));
        }
        Ok(format!("workspace={}", init_tmp.display()))
    })();
    match init_status {
        Ok(detail) => push("init", FullStepStatus::Passed, detail),
        Err(err) => push("init", FullStepStatus::Failed, err.to_string()),
    }

    let mut discovered = discover_scenarios(scenario_root);
    if let Some(filter) = scenario_filter
        && !filter.is_empty()
    {
        discovered
            .steps
            .retain(|p| p.to_string_lossy().contains(filter));
        discovered
            .distributed
            .retain(|p| p.to_string_lossy().contains(filter));
    }
    let parse_error_count = discovered.parse_errors.len();
    let parsed_summary = format!(
        "discovered step_scenarios={} distributed_scenarios={} parse_errors={}",
        discovered.steps.len(),
        discovered.distributed.len(),
        parse_error_count
    );
    push(
        "discover_scenarios",
        if parse_error_count > 0 {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        parsed_summary,
    );
    if parse_error_count > 0 {
        guidance.push(format!(
            "Fix malformed scenarios before trusting `fozzy full` coverage: {}",
            discovered.parse_errors.join(" | ")
        ));
    }

    if let Some(root) = require_topology_coverage {
        match fozzy::map_suites(&MapSuitesOptions {
            root: root.to_path_buf(),
            scenario_root: scenario_root.to_path_buf(),
            min_risk: topology_min_risk,
            profile: topology_profile,
            shrink_policy: topology_shrink_policy,
            limit: 200,
            offset: 0,
            max_matched_scenarios: 25,
        }) {
            Ok(report) => {
                let ok = report.uncovered_hotspot_count == 0;
                push(
                    "topology_coverage",
                    if ok {
                        FullStepStatus::Passed
                    } else {
                        FullStepStatus::Failed
                    },
                    format!(
                        "required_hotspots={} covered={} uncovered={} min_risk={} profile={} root={} scenario_root={}",
                        report.required_hotspot_count,
                        report.covered_hotspot_count,
                        report.uncovered_hotspot_count,
                        report.effective_min_risk,
                        format!("{:?}", report.profile).to_lowercase(),
                        root.display(),
                        scenario_root.display()
                    ),
                );
            }
            Err(err) => push("topology_coverage", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        push(
            "topology_coverage",
            FullStepStatus::Skipped,
            "not requested (use --require-topology-coverage <repo_root>)".to_string(),
        );
    }

    let pick_step = discovered
        .steps
        .iter()
        .find(|p| is_preferred_step_scenario(p))
        .cloned()
        .or_else(|| discovered.steps.first().cloned());
    let pick_distributed = discovered
        .distributed
        .iter()
        .find(|p| is_preferred_distributed_scenario(p))
        .cloned()
        .or_else(|| discovered.distributed.first().cloned());

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

    let mut primary_trace: Option<PathBuf> = None;
    let mut shrunk_trace: Option<PathBuf> = None;
    let mut primary_status: Option<ExitStatus> = None;
    let mut shrunk_status: Option<ExitStatus> = None;

    if pick_step.is_none() {
        push(
            "doctor_deep",
            FullStepStatus::Skipped,
            "no step scenario found; add tests/*.fozzy.json to run deterministic audits"
                .to_string(),
        );
        push(
            "test_det",
            FullStepStatus::Skipped,
            "no step scenario found".to_string(),
        );
        push(
            "run_record_trace",
            FullStepStatus::Skipped,
            "no step scenario found".to_string(),
        );
    } else {
        let primary = pick_step
            .clone()
            .expect("pick_step checked as Some in else branch");
        match fozzy::doctor(
            config,
            &fozzy::DoctorOptions {
                deep: true,
                scenario: Some(ScenarioPath::new(primary.clone())),
                runs: doctor_runs.max(2),
                seed,
            },
        ) {
            Ok(doctor) => {
                let doctor_failed_strict = strict
                    && (!doctor.issues.is_empty()
                        || doctor
                            .nondeterminism_signals
                            .as_ref()
                            .is_some_and(|s| !s.is_empty()));
                push(
                    "doctor_deep",
                    if doctor.ok && !doctor_failed_strict {
                        FullStepStatus::Passed
                    } else {
                        FullStepStatus::Failed
                    },
                    format!(
                        "ok={} strict_policy_ok={} runs={} scenario={}",
                        doctor.ok,
                        !doctor_failed_strict,
                        doctor_runs.max(2),
                        primary.display()
                    ),
                );
            }
            Err(err) => push("doctor_deep", FullStepStatus::Failed, err.to_string()),
        }

        let filtered_steps: Vec<PathBuf> = discovered
            .steps
            .iter()
            .filter(|p| !is_negative_fixture_scenario(p))
            .cloned()
            .collect();
        let test_targets = if filtered_steps.is_empty() {
            vec![primary.clone()]
        } else {
            filtered_steps
        };
        let test_globs: Vec<String> = test_targets
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
                let strict_ok = enforce_strict_summary(strict, &test.summary).is_ok();
                push(
                    "test_det",
                    if strict_ok {
                        FullStepStatus::Passed
                    } else {
                        FullStepStatus::Failed
                    },
                    format!(
                        "status={:?} strict_ok={} run_id={}",
                        test.summary.status, strict_ok, test.summary.identity.run_id
                    ),
                )
            }
            Err(err) => push("test_det", FullStepStatus::Failed, err.to_string()),
        }

        let trace_path = register_temp(
            std::env::temp_dir().join(format!("fozzy-full-{}.trace.fozzy", uuid::Uuid::new_v4())),
        );
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
                memory: memory.clone(),
            },
        ) {
            Ok(run) => {
                primary_status = Some(run.summary.status);
                let strict_ok = enforce_strict_summary(strict, &run.summary).is_ok();
                if run.summary.identity.trace_path.is_some() {
                    primary_trace = Some(trace_path.clone());
                }
                push(
                    "run_record_trace",
                    if run.summary.identity.trace_path.is_some() && strict_ok {
                        FullStepStatus::Passed
                    } else {
                        FullStepStatus::Failed
                    },
                    format!(
                        "status={:?} strict_ok={} trace={}",
                        run.summary.status,
                        strict_ok,
                        trace_path.display()
                    ),
                );
            }
            Err(err) => push("run_record_trace", FullStepStatus::Failed, err.to_string()),
        }
    }

    if let Some(trace) = primary_trace.as_ref() {
        match fozzy::verify_trace_file(trace) {
            Ok(verify) => {
                let strict_verify_ok = !strict
                    || (verify.checksum_present
                        && verify.checksum_valid
                        && verify.warnings.is_empty());
                push(
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
            Err(err) => push("trace_verify", FullStepStatus::Failed, err.to_string()),
        }

        match fozzy::replay_trace(
            config,
            TracePath::new(trace.clone()),
            &fozzy::ReplayOptions {
                step: false,
                until: None,
                dump_events: false,
                profile_capture: ProfileCaptureLevel::Baseline,
                reporter: Reporter::Json,
            },
        ) {
            Ok(replay) => {
                let (status, detail) =
                    replay_summary_status(primary_status, &replay.summary, strict);
                push(
                    "replay",
                    status,
                    format!("{detail} run_id={}", replay.summary.identity.run_id),
                );
            }
            Err(err) => push("replay", FullStepStatus::Failed, err.to_string()),
        }

        match fozzy::ci_command(
            config,
            &CiOptions {
                trace: trace.clone(),
                flake_runs: Vec::new(),
                flake_budget_pct: None,
                perf_baseline: None,
                max_p99_delta_pct: None,
                strict,
            },
        ) {
            Ok(ci) => push(
                "ci",
                if ci.ok {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                format!("ok={} checks={}", ci.ok, ci.checks.len()),
            ),
            Err(err) => push("ci", FullStepStatus::Failed, err.to_string()),
        }

        let shrink_out = register_temp(
            std::env::temp_dir().join(format!("fozzy-full-{}.min.fozzy", uuid::Uuid::new_v4())),
        );
        match fozzy::shrink_trace(
            config,
            TracePath::new(trace.clone()),
            &fozzy::ShrinkOptions {
                out_trace_path: Some(shrink_out.clone()),
                budget: None,
                aggressive: false,
                minimize: ShrinkMinimize::All,
            },
        ) {
            Ok(shrink) => {
                shrunk_trace = Some(PathBuf::from(shrink.out_trace_path.clone()));
                shrunk_status = Some(shrink.result.summary.status);
                let status = if allow_expected_failures {
                    if let Some(primary) = primary_status {
                        if shrink_status_matches(primary, shrink.result.summary.status) {
                            shrink_classification =
                                Some("expected_fail_class_preserved".to_string());
                            FullStepStatus::Passed
                        } else {
                            shrink_classification =
                                Some("expected_fail_class_mismatch".to_string());
                            FullStepStatus::Failed
                        }
                    } else {
                        shrink_classification = Some("primary_status_missing".to_string());
                        FullStepStatus::Passed
                    }
                } else if shrink.result.summary.status == ExitStatus::Pass {
                    shrink_classification = Some("pass_required_policy".to_string());
                    FullStepStatus::Passed
                } else {
                    shrink_classification = Some("policy_rejected_non_pass".to_string());
                    FullStepStatus::Failed
                };
                push(
                    "shrink",
                    status,
                    format!("out_trace={}", shrink.out_trace_path),
                );
            }
            Err(err) => {
                shrink_classification = Some("tooling_failure".to_string());
                push("shrink", FullStepStatus::Failed, err.to_string())
            }
        }

        if let Some(min_trace) = shrunk_trace.as_ref() {
            match fozzy::replay_trace(
                config,
                TracePath::new(min_trace.clone()),
                &fozzy::ReplayOptions {
                    step: false,
                    until: None,
                    dump_events: false,
                    profile_capture: ProfileCaptureLevel::Baseline,
                    reporter: Reporter::Json,
                },
            ) {
                Ok(replay) => {
                    let (status, detail) =
                        replay_summary_status(shrunk_status, &replay.summary, strict);
                    push("replay_shrunk", status, detail);
                }
                Err(err) => push("replay_shrunk", FullStepStatus::Failed, err.to_string()),
            }
        } else {
            push(
                "replay_shrunk",
                FullStepStatus::Skipped,
                "shrink output not available".to_string(),
            );
        }

        let _ = fozzy::artifacts_command(
            config,
            &ArtifactCommand::Ls {
                run: trace.display().to_string(),
            },
        )
        .map(|_| {
            push(
                "artifacts_ls",
                FullStepStatus::Passed,
                trace.display().to_string(),
            )
        })
        .map_err(|err| push("artifacts_ls", FullStepStatus::Failed, err.to_string()));

        let artifacts_export = register_temp(
            std::env::temp_dir().join(format!("fozzy-full-artifacts-{}.zip", uuid::Uuid::new_v4())),
        );
        match fozzy::artifacts_command(
            config,
            &ArtifactCommand::Export {
                run: trace.display().to_string(),
                out: artifacts_export.clone(),
            },
        ) {
            Ok(_) => {
                let (status, detail) = file_artifact_status(&artifacts_export);
                push("artifacts_export", status, detail);
            }
            Err(err) => push("artifacts_export", FullStepStatus::Failed, err.to_string()),
        }

        let artifacts_pack = register_temp(
            std::env::temp_dir().join(format!("fozzy-full-pack-{}.zip", uuid::Uuid::new_v4())),
        );
        match fozzy::artifacts_command(
            config,
            &ArtifactCommand::Pack {
                run: trace.display().to_string(),
                out: artifacts_pack.clone(),
            },
        ) {
            Ok(_) => {
                let (status, detail) = file_artifact_status(&artifacts_pack);
                push("artifacts_pack", status, detail);
            }
            Err(err) => push("artifacts_pack", FullStepStatus::Failed, err.to_string()),
        }

        if let Some(min_trace) = shrunk_trace.as_ref() {
            match fozzy::artifacts_command(
                config,
                &ArtifactCommand::Diff {
                    left: trace.display().to_string(),
                    right: min_trace.display().to_string(),
                },
            ) {
                Ok(_) => push(
                    "artifacts_diff",
                    FullStepStatus::Passed,
                    format!("left={} right={}", trace.display(), min_trace.display()),
                ),
                Err(err) => push("artifacts_diff", FullStepStatus::Failed, err.to_string()),
            }
        } else {
            push(
                "artifacts_diff",
                FullStepStatus::Skipped,
                "requires shrink output".to_string(),
            );
        }

        match fozzy::report_command(
            config,
            &ReportCommand::Show {
                run: trace.display().to_string(),
                format: Reporter::Pretty,
            },
        ) {
            Ok(value) => {
                let format = value
                    .get("format")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pretty");
                let bytes = value
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.len())
                    .unwrap_or(0);
                push(
                    "report_show",
                    FullStepStatus::Passed,
                    format!("format={format} content_bytes={bytes}"),
                )
            }
            Err(err) => push("report_show", FullStepStatus::Failed, err.to_string()),
        }

        match fozzy::report_command(
            config,
            &ReportCommand::Query {
                run: trace.display().to_string(),
                jq: Some(".status".to_string()),
                list_paths: false,
            },
        ) {
            Ok(value) => {
                let status_value = value
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| value.to_string());
                push(
                    "report_query",
                    FullStepStatus::Passed,
                    format!(".status={status_value}"),
                )
            }
            Err(err) => push("report_query", FullStepStatus::Failed, err.to_string()),
        }

        match fozzy::report_command(
            config,
            &ReportCommand::Query {
                run: trace.display().to_string(),
                jq: None,
                list_paths: true,
            },
        ) {
            Ok(value) => push(
                "report_query_paths",
                FullStepStatus::Passed,
                format!(
                    "paths={}",
                    value.get("paths")
                        .and_then(|v| v.as_array())
                        .map(|v| v.len())
                        .unwrap_or(0)
                ),
            ),
            Err(err) => push(
                "report_query_paths",
                FullStepStatus::Failed,
                err.to_string(),
            ),
        }

        if let Some(min_trace) = shrunk_trace.as_ref() {
            match fozzy::report_command(
                config,
                &ReportCommand::Flaky {
                    runs: vec![trace.display().to_string(), min_trace.display().to_string()],
                    flake_budget: None,
                },
            ) {
                Ok(value) => {
                    let (status, detail) = flaky_report_status(&value);
                    push("report_flaky", status, detail)
                }
                Err(err) => push("report_flaky", FullStepStatus::Failed, err.to_string()),
            }
        } else {
            push(
                "report_flaky",
                FullStepStatus::Skipped,
                "requires second trace input".to_string(),
            );
        }

        match fozzy::memory_command(
            config,
            &MemoryCommand::Top {
                run: trace.display().to_string(),
                limit: 10,
            },
        ) {
            Ok(value) => {
                let (status, detail) = memory_top_status(&value);
                push("memory_top", status, detail)
            }
            Err(err) => push("memory_top", FullStepStatus::Failed, err.to_string()),
        }

        match fozzy::memory_command(
            config,
            &MemoryCommand::Graph {
                run: trace.display().to_string(),
                out: None,
            },
        ) {
            Ok(value) => push(
                "memory_graph",
                FullStepStatus::Passed,
                format!(
                    "nodes={} edges={}",
                    value.pointer("/graph/nodes")
                        .and_then(|v| v.as_array())
                        .map(|v| v.len())
                        .unwrap_or(0),
                    value.pointer("/graph/edges")
                        .and_then(|v| v.as_array())
                        .map(|v| v.len())
                        .unwrap_or(0)
                ),
            ),
            Err(err) => push("memory_graph", FullStepStatus::Failed, err.to_string()),
        }

        if let Some(min_trace) = shrunk_trace.as_ref() {
            match fozzy::memory_command(
                config,
                &MemoryCommand::Diff {
                    left: trace.display().to_string(),
                    right: min_trace.display().to_string(),
                },
            ) {
                Ok(value) => {
                    let (status, detail) = memory_diff_status(&value);
                    push("memory_diff", status, detail)
                }
                Err(err) => push("memory_diff", FullStepStatus::Failed, err.to_string()),
            }
        } else {
            push(
                "memory_diff",
                FullStepStatus::Skipped,
                "requires second trace input".to_string(),
            );
        }

        match fozzy::profile_command(
            config,
            &ProfileCommand::Top {
                run: trace.display().to_string(),
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

        if let Some(min_trace) = shrunk_trace.as_ref() {
            match fozzy::profile_command(
                config,
                &ProfileCommand::Diff {
                    left: trace.display().to_string(),
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
                    push("profile_diff", status, detail)
                }
                Err(err) => push("profile_diff", FullStepStatus::Failed, err.to_string()),
            }
        } else {
            push(
                "profile_diff",
                FullStepStatus::Skipped,
                "requires second trace input".to_string(),
            );
        }

        match fozzy::profile_command(
            config,
            &ProfileCommand::Explain {
                run: trace.display().to_string(),
                diff_with: shrunk_trace.as_ref().map(|p| p.display().to_string()),
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
        ] {
            push(
                name,
                FullStepStatus::Skipped,
                "no recorded trace available".to_string(),
            );
        }
    }

    if let Some(primary) = pick_step.as_ref() {
        let full_fuzz_target = FuzzTarget::Scenario {
            path: primary.clone(),
        };
        let fuzz_trace = register_temp(std::env::temp_dir().join(format!(
            "fozzy-full-fuzz-{}.trace.fozzy",
            uuid::Uuid::new_v4()
        )));
        match fozzy::fuzz(
            config,
            &full_fuzz_target,
            &FuzzOptions {
                det: false,
                mode: FuzzMode::Coverage,
                seed,
                time: Some(fuzz_time),
                runs: None,
                max_input_bytes: 4096,
                corpus_dir: None,
                mutator: None,
                shrink: true,
                record_trace_to: Some(fuzz_trace.clone()),
                reporter: Reporter::Json,
                crash_only: false,
                minimize: true,
                record_collision: RecordCollisionPolicy::Overwrite,
                profile_capture: ProfileCaptureLevel::Baseline,
                memory: memory.clone(),
            },
        ) {
            Ok(fuzz_run) => push(
                "fuzz",
                FullStepStatus::Passed,
                format!(
                    "status={:?} run_id={} scenario={}",
                    fuzz_run.summary.status,
                    fuzz_run.summary.identity.run_id,
                    primary.display()
                ),
            ),
            Err(err) => push("fuzz", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        push(
            "fuzz",
            FullStepStatus::Skipped,
            "no step scenario found for scenario-backed fuzz".to_string(),
        );
    }

    if let Some(distributed) = pick_distributed.as_ref() {
        match fozzy::explore(
            config,
            ScenarioPath::new(distributed.clone()),
            &ExploreOptions {
                seed,
                time: None,
                steps: Some(explore_steps),
                nodes: Some(explore_nodes),
                faults: None,
                schedule: ScheduleStrategy::CoverageGuided,
                checker: None,
                record_trace_to: None,
                shrink: true,
                minimize: true,
                reporter: Reporter::Json,
                record_collision: RecordCollisionPolicy::Error,
                profile_capture: ProfileCaptureLevel::Baseline,
                memory: memory.clone(),
            },
        ) {
            Ok(explore) => push(
                "explore",
                FullStepStatus::Passed,
                format!(
                    "status={:?} scenario={}",
                    explore.summary.status,
                    distributed.display()
                ),
            ),
            Err(err) => push("explore", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        push(
            "explore",
            FullStepStatus::Skipped,
            "no distributed scenario found; add tests/*.fozzy.json with `distributed` schema"
                .to_string(),
        );
    }

    let corpus_dir = register_temp(
        std::env::temp_dir().join(format!("fozzy-full-corpus-{}", uuid::Uuid::new_v4())),
    );
    let seed_file = corpus_dir.join("seed.bin");
    let corpus_zip = register_temp(
        std::env::temp_dir().join(format!("fozzy-full-corpus-{}.zip", uuid::Uuid::new_v4())),
    );
    let corpus_import_dir = register_temp(
        std::env::temp_dir().join(format!("fozzy-full-corpus-import-{}", uuid::Uuid::new_v4())),
    );
    let corpus_setup = (|| -> anyhow::Result<()> {
        std::fs::create_dir_all(&corpus_dir)?;
        std::fs::write(&seed_file, b"fozzy-corpus-seed")?;
        Ok(())
    })();
    if let Err(err) = corpus_setup {
        for name in [
            "corpus_add",
            "corpus_list",
            "corpus_minimize",
            "corpus_export",
            "corpus_import",
        ] {
            push(name, FullStepStatus::Failed, err.to_string());
        }
    } else {
        match fozzy::corpus_command(
            config,
            &CorpusCommand::Add {
                dir: corpus_dir.clone(),
                file: seed_file.clone(),
            },
        ) {
            Ok(value) => push(
                "corpus_add",
                FullStepStatus::Passed,
                value.get("added")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| corpus_dir.to_string_lossy().to_string()),
            ),
            Err(err) => push("corpus_add", FullStepStatus::Failed, err.to_string()),
        }
        match fozzy::corpus_command(
            config,
            &CorpusCommand::List {
                dir: corpus_dir.clone(),
            },
        ) {
            Ok(value) => push(
                "corpus_list",
                FullStepStatus::Passed,
                format!(
                    "files={}",
                    value.as_array().map(|v| v.len()).unwrap_or_default()
                ),
            ),
            Err(err) => push("corpus_list", FullStepStatus::Failed, err.to_string()),
        }
        match fozzy::corpus_command(
            config,
            &CorpusCommand::Minimize {
                dir: corpus_dir.clone(),
                budget: None,
            },
        ) {
            Ok(value) => {
                let before = value.get("filesBefore").and_then(|v| v.as_u64()).unwrap_or(0);
                let after = value.get("filesAfter").and_then(|v| v.as_u64()).unwrap_or(0);
                let removed = value
                    .get("duplicatesRemoved")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                push(
                    "corpus_minimize",
                    FullStepStatus::Passed,
                    format!(
                        "files_before={before} files_after={after} duplicates_removed={removed}"
                    ),
                )
            }
            Err(err) => push("corpus_minimize", FullStepStatus::Failed, err.to_string()),
        }
        match fozzy::corpus_command(
            config,
            &CorpusCommand::Export {
                dir: corpus_dir.clone(),
                out: corpus_zip.clone(),
            },
        ) {
            Ok(_) => push(
                "corpus_export",
                FullStepStatus::Passed,
                corpus_zip.display().to_string(),
            ),
            Err(err) => push("corpus_export", FullStepStatus::Failed, err.to_string()),
        }
        match fozzy::corpus_command(
            config,
            &CorpusCommand::Import {
                zip: corpus_zip,
                out: corpus_import_dir,
            },
        ) {
            Ok(value) => push(
                "corpus_import",
                FullStepStatus::Passed,
                value.get("dir")
                    .and_then(|v| v.as_str())
                    .unwrap_or("imported corpus")
                    .to_string(),
            ),
            Err(err) => push("corpus_import", FullStepStatus::Failed, err.to_string()),
        }
    }

    if let Some(primary) = pick_step.as_ref() {
        match fozzy::run_scenario(
            config,
            ScenarioPath::new(primary.clone()),
            &RunOptions {
                det: false,
                seed,
                timeout: None,
                reporter: Reporter::Json,
                record_trace_to: None,
                filter: None,
                jobs: None,
                fail_fast: false,
                record_collision: RecordCollisionPolicy::Error,
                profile_capture: ProfileCaptureLevel::Baseline,
                proc_backend: fozzy::ProcBackend::Host,
                fs_backend: fozzy::FsBackend::Host,
                http_backend: fozzy::HttpBackend::Host,
                memory,
            },
        ) {
            Ok(host_run) => push(
                "host_backends_run",
                FullStepStatus::Passed,
                format!("status={:?}", host_run.summary.status),
            ),
            Err(err) => push("host_backends_run", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        push(
            "host_backends_run",
            FullStepStatus::Skipped,
            "no step scenario found".to_string(),
        );
    }

    let env = fozzy::env_info(config);
    push(
        "env",
        FullStepStatus::Passed,
        format!(
            "proc={} fs={} http={}",
            env.capabilities
                .get("proc")
                .map(|c| c.backend.as_str())
                .unwrap_or("unknown"),
            env.capabilities
                .get("fs")
                .map(|c| c.backend.as_str())
                .unwrap_or("unknown"),
            env.capabilities
                .get("http")
                .map(|c| c.backend.as_str())
                .unwrap_or("unknown")
        ),
    );

    apply_full_policy_filters(&mut steps, skip_steps, required_steps);

    let report = FullReport {
        schema_version: "fozzy.full_report.v1".to_string(),
        strict,
        unsafe_mode,
        scenario_root: scenario_root.display().to_string(),
        guidance,
        shrink_classification,
        steps,
    };
    for p in temp_paths {
        let _ = if p.is_dir() {
            std::fs::remove_dir_all(&p)
        } else {
            std::fs::remove_file(&p)
        };
    }
    Ok(report)
}

fn discover_scenarios(root: &Path) -> FullScenarioDiscovery {
    let mut out = FullScenarioDiscovery {
        steps: Vec::new(),
        distributed: Vec::new(),
        parse_errors: Vec::new(),
    };
    if !root.exists() {
        return out;
    }
    for entry in WalkDir::new(root).into_iter().flatten() {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".fozzy.json") {
            continue;
        }
        let bytes = match std::fs::read(path) {
            Ok(v) => v,
            Err(err) => {
                out.parse_errors
                    .push(format!("{}: {}", path.display(), err));
                continue;
            }
        };
        match serde_json::from_slice::<fozzy::ScenarioFile>(&bytes) {
            Ok(fozzy::ScenarioFile::Steps(_)) => out.steps.push(path.to_path_buf()),
            Ok(fozzy::ScenarioFile::Distributed(_)) => out.distributed.push(path.to_path_buf()),
            Ok(fozzy::ScenarioFile::Suites(_)) => out.parse_errors.push(format!(
                "{}: suites format is not executable",
                path.display()
            )),
            Err(err) => out.parse_errors.push(format!("{}: {err}", path.display())),
        }
    }
    out.steps.sort();
    out.distributed.sort();
    out
}

fn git_clean_tree_check() -> anyhow::Result<String> {
    let out = ProcessCommand::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map_err(|err| anyhow::anyhow!("failed to execute git status --porcelain: {err}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stderr_lower = stderr.to_ascii_lowercase();
        if stderr_lower.contains("not a git repository") {
            return Ok("git worktree check skipped: not a git repository".to_string());
        }
        return Err(anyhow::anyhow!(
            "git status --porcelain failed; verify this is a git worktree{}{}",
            if stderr.is_empty() { "" } else { ": " },
            stderr
        ));
    }
    let body = String::from_utf8_lossy(&out.stdout);
    let dirty: Vec<&str> = body.lines().collect();
    if dirty.is_empty() {
        return Ok("git worktree clean".to_string());
    }
    let preview = dirty
        .iter()
        .take(3)
        .copied()
        .collect::<Vec<_>>()
        .join(" | ");
    Err(anyhow::anyhow!(
        "git worktree is not clean ({} change(s)); example: {}",
        dirty.len(),
        preview
    ))
}

pub(super) fn selected_init_test_types(
    with: &[InitTestType],
    all_tests: bool,
) -> Vec<InitTestType> {
    if all_tests || with.is_empty() {
        return vec![InitTestType::All];
    }
    let mut out = with.to_vec();
    if out.contains(&InitTestType::All) {
        return vec![InitTestType::All];
    }
    out.sort_by_key(|v| match v {
        InitTestType::Run => 0,
        InitTestType::Fuzz => 1,
        InitTestType::Explore => 2,
        InitTestType::Memory => 3,
        InitTestType::Host => 4,
        InitTestType::All => 5,
    });
    out.dedup();
    out
}

fn apply_full_policy_filters(
    steps: &mut [FullStepResult],
    skip_steps: &[String],
    required_steps: &[String],
) {
    use std::collections::BTreeSet;
    let skip: BTreeSet<String> = skip_steps
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let required: BTreeSet<String> = required_steps
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();

    for step in steps {
        let key = step.name.to_ascii_lowercase();
        if key == "policy_conflict" {
            continue;
        }
        if !required.is_empty() && !required.contains(&key) {
            step.status = FullStepStatus::Skipped;
            step.detail = format!("skipped by required-steps policy; {}", step.detail);
            continue;
        }
        if skip.contains(&key) {
            step.status = FullStepStatus::Skipped;
            step.detail = format!("skipped by skip-steps policy; {}", step.detail);
        }
    }
}

fn full_policy_conflict_details(
    skip_steps: &[String],
    required_steps: &[String],
    topology_required: bool,
) -> Option<String> {
    use std::collections::BTreeSet;
    if !topology_required {
        return None;
    }
    let req: BTreeSet<String> = required_steps
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    if !req.is_empty() && !req.contains("topology_coverage") {
        return Some(
            "--require-topology-coverage was set, but --required-steps excludes topology_coverage; refusing implicit policy neutralization"
                .to_string(),
        );
    }
    let skip: BTreeSet<String> = skip_steps
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    if skip.contains("topology_coverage") {
        return Some(
            "--require-topology-coverage conflicts with --skip-steps topology_coverage; remove one policy flag"
                .to_string(),
        );
    }
    None
}

fn shrink_status_matches(target: ExitStatus, candidate: ExitStatus) -> bool {
    if target == ExitStatus::Pass {
        candidate == ExitStatus::Pass
    } else {
        candidate != ExitStatus::Pass
    }
}

fn is_negative_fixture_scenario(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["fail", "leak", "panic", "timeout", "checkers", "assertions"]
        .iter()
        .any(|tok| name.contains(tok))
}

fn is_preferred_step_scenario(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.contains("pass") || name.contains("example")
}

fn is_preferred_distributed_scenario(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    !name.contains("checkers")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fozzy::{RunIdentity, RunMode};

    #[test]
    fn profile_diff_status_rejects_regressions() {
        let value = serde_json::json!({
            "summary": {
                "verdict": "minor_regression",
                "regressionCount": 1,
                "significantRegressionCount": 0
            }
        });
        let (status, detail) = profile_diff_status(&value, false);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("verdict=minor_regression"));
    }

    #[test]
    fn profile_diff_status_requires_stable_when_requested() {
        let value = serde_json::json!({
            "summary": {
                "verdict": "improvement",
                "regressionCount": 0,
                "significantRegressionCount": 0
            }
        });
        let (status, _) = profile_diff_status(&value, true);
        assert!(matches!(status, FullStepStatus::Failed));

        let (status_no_stable, _) = profile_diff_status(&value, false);
        assert!(matches!(status_no_stable, FullStepStatus::Passed));
    }

    #[test]
    fn profile_top_status_rejects_empty_domains() {
        let value = serde_json::json!({
            "warnings": [],
            "emptyDomains": [{"domain": "heap", "reason": "no heap samples in trace"}]
        });
        let (status, detail) = profile_top_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("heap:no heap samples in trace"));
    }

    #[test]
    fn profile_top_status_rejects_warnings() {
        let value = serde_json::json!({
            "warnings": ["cpu domain uses host-time sampling"],
            "emptyDomains": []
        });
        let (status, detail) = profile_top_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("warnings=cpu domain uses host-time sampling"));
    }

    #[test]
    fn profile_explain_status_skips_non_diagnostic_results() {
        let value = serde_json::json!({
            "regressionStatement": "no measurable regression shift found",
            "likelyCauseDomain": "unknown",
            "topShiftedPath": "n/a"
        });
        let (status, detail) = profile_explain_status(&value);
        assert!(matches!(status, FullStepStatus::Skipped));
        assert!(detail.contains("cause_domain=unknown"));
        assert!(detail.contains("shifted_path=n/a"));
    }

    #[test]
    fn profile_explain_status_accepts_real_diagnosis() {
        let value = serde_json::json!({
            "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
            "likelyCauseDomain": "latency",
            "topShiftedPath": "metric::p99_latency_ms"
        });
        let (status, detail) = profile_explain_status(&value);
        assert!(matches!(status, FullStepStatus::Passed));
        assert!(detail.contains("cause_domain=latency"));
        assert!(detail.contains("shifted_path=metric::p99_latency_ms"));
    }

    fn sample_run_summary(status: ExitStatus) -> RunSummary {
        RunSummary {
            status,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "test-run".to_string(),
                seed: 7,
                trace_path: None,
                report_path: None,
                artifacts_dir: None,
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 1,
            duration_ns: 1_000_000,
            tests: None,
            memory: None,
            findings: Vec::new(),
        }
    }

    #[test]
    fn replay_summary_status_rejects_class_mismatch() {
        let summary = sample_run_summary(ExitStatus::Fail);
        let (status, detail) = replay_summary_status(Some(ExitStatus::Pass), &summary, true);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("class_ok=false"));
    }

    #[test]
    fn file_artifact_status_rejects_missing_output() {
        let path = std::env::temp_dir().join(format!(
            "fozzy-missing-artifact-{}.zip",
            uuid::Uuid::new_v4()
        ));
        let (status, detail) = file_artifact_status(&path);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("missing"));
    }

    #[test]
    fn flaky_report_status_rejects_flaky_results() {
        let value = serde_json::json!({
            "runCount": 2,
            "isFlaky": true,
            "flakeRatePct": 50.0
        });
        let (status, detail) = flaky_report_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("is_flaky=true"));
    }

    #[test]
    fn memory_top_status_rejects_leaks() {
        let value = serde_json::json!({
            "total": 1,
            "leaks": [{"allocId": 1}]
        });
        let (status, detail) = memory_top_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("total_leaks=1"));
    }

    #[test]
    fn memory_diff_status_rejects_contract_drift() {
        let value = serde_json::json!({
            "deltaLeakedBytes": 64,
            "deltaPeakBytes": 0
        });
        let (status, detail) = memory_diff_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("delta_leaked_bytes=64"));
    }
}
