use super::super::*;
use super::*;
pub(in crate::profile) fn profile_doctor(
    config: &Config,
    strict: bool,
    run: &str,
    deep: bool,
) -> FozzyResult<serde_json::Value> {
    let run_label = crate::normalize_run_or_trace_selector(run);
    let mut checks = Vec::<serde_json::Value>::new();
    let mut issues = Vec::<String>::new();
    let check = |name: &str, status: &str, detail: serde_json::Value| {
        serde_json::json!({
            "name": name,
            "ok": status == "pass",
            "status": status,
            "detail": detail,
        })
    };
    let env_report = profile_env_report(config, strict);
    let unavailable_domains = env_report
        .get("domains")
        .and_then(|v| v.as_object())
        .map(|domains| {
            domains
                .iter()
                .filter_map(|(name, domain)| {
                    let available = domain
                        .get("available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if available { None } else { Some(name.clone()) }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let env_status = if unavailable_domains.is_empty() {
        "pass"
    } else {
        issues.push(format!(
            "env: unsupported domains={}",
            unavailable_domains.join(",")
        ));
        "warn"
    };
    checks.push(check("env", env_status, env_report));

    let bundle = match load_profile_bundle(
        config,
        run,
        ProfileLoadSpec {
            timeline: true,
            cpu: true,
            heap: true,
            latency: true,
            symbols: false,
        },
    ) {
        Ok(bundle) => {
            checks.push(check(
                "load_bundle",
                "pass",
                serde_json::json!("resolved run/trace and loaded profile artifacts"),
            ));
            bundle
        }
        Err(err) => {
            let detail = err.to_string();
            issues.push(detail.clone());
            checks.push(check("load_bundle", "fail", serde_json::json!(detail)));
            return Ok(serde_json::json!({
                "schemaVersion": "fozzy.profile_doctor.v1",
                "run": run_label,
                "ok": false,
                "checks": checks,
                "issues": issues,
            }));
        }
    };

    let top_domains = normalize_domains(false, false, false, false, false);
    let top_cpu_contract = enforce_cpu_contract(
        true,
        top_domains.iter().any(|d| d == "cpu"),
        &[bundle.cpu.as_ref().map(|cpu| cpu.sample_count).unwrap_or(0)],
    );
    let top_has_any = !top_by_tag(
        bundle.timeline.as_ref().expect("timeline loaded"),
        ProfileEventKind::Io,
        10,
    )
    .is_empty()
        || !top_by_tag(
            bundle.timeline.as_ref().expect("timeline loaded"),
            ProfileEventKind::Sched,
            10,
        )
        .is_empty()
        || !bundle
            .heap
            .as_ref()
            .expect("heap loaded")
            .hotspots
            .is_empty()
        || !bundle
            .latency
            .as_ref()
            .expect("latency loaded")
            .critical_path
            .is_empty();
    let top_status = if top_cpu_contract.is_err() || !top_has_any {
        "warn"
    } else {
        "pass"
    };
    let top_detail = match top_cpu_contract {
        Err(err) => err.to_string(),
        Ok(()) if top_has_any => format!("default domains={top_domains:?}"),
        Ok(()) => "default profile top returned no rows".to_string(),
    };
    checks.push(check("top", top_status, serde_json::json!(top_detail)));

    let heap_folded = heap_folded(bundle.heap.as_ref().expect("heap loaded"));
    checks.push(check(
        "flame_heap",
        if heap_folded.is_empty() {
            "warn"
        } else {
            "pass"
        },
        serde_json::json!(if heap_folded.is_empty() {
            "no heap samples in trace"
        } else {
            "heap flame data present"
        }),
    ));
    checks.push(check(
        "flame_cpu",
        if bundle
            .cpu
            .as_ref()
            .expect("cpu loaded")
            .folded_stacks
            .is_empty()
        {
            "warn"
        } else {
            "pass"
        },
        serde_json::json!(if bundle
            .cpu
            .as_ref()
            .expect("cpu loaded")
            .folded_stacks
            .is_empty()
        {
            "no cpu samples in trace"
        } else {
            "cpu flame data present"
        }),
    ));

    checks.push(check(
        "timeline",
        "pass",
        serde_json::json!(format!(
            "events={}",
            bundle.timeline.as_ref().expect("timeline loaded").len()
        )),
    ));
    let diff_domains = ["cpu", "heap", "latency", "io", "sched"]
        .iter()
        .filter(|domain| !unavailable_domains.iter().any(|name| name == **domain))
        .map(|domain| (*domain).to_string())
        .collect::<Vec<_>>();
    let diff = compute_diff(
        run,
        run,
        &diff_domains,
        &bundle.metrics,
        &bundle.metrics,
        bundle.heap.as_ref(),
        bundle.heap.as_ref(),
        &HashMap::new(),
        &HashMap::new(),
        1,
        1,
    );
    let diff_ok = diff.summary.verdict == "stable"
        && diff.summary.regression_count == 0
        && diff.summary.significant_regression_count == 0;
    checks.push(check(
        "diff",
        if diff_ok { "pass" } else { "warn" },
        serde_json::json!(format!(
            "verdict={} regressions={} significant_regressions={} domains={}",
            diff.summary.verdict,
            diff.summary.regression_count,
            diff.summary.significant_regression_count,
            diff_domains.join(",")
        )),
    ));
    let explain = explain_single(
        run,
        &bundle.artifacts_dir,
        &bundle.metrics,
        bundle.latency.as_ref().expect("latency loaded"),
    );
    let explain_ok = is_diagnostic_profile_explain(&explain);
    checks.push(check(
        "explain",
        if explain_ok { "pass" } else { "warn" },
        serde_json::json!(if explain_ok {
            explain.likely_cause_domain
        } else {
            "single-run explain is observational, not diagnostic".to_string()
        }),
    ));
    let speedscope: serde_json::Value =
        folded_to_speedscope(run, &bundle.cpu.as_ref().expect("cpu loaded").folded_stacks);
    let speedscope_frames = speedscope
        .get("shared")
        .and_then(|v| v.get("frames"))
        .and_then(|v| v.as_array())
        .map(|v| v.len())
        .unwrap_or(0);
    checks.push(check(
        "export",
        if speedscope_frames == 0 {
            "warn"
        } else {
            "pass"
        },
        serde_json::json!(format!("speedscope_frames={speedscope_frames}")),
    ));

    if deep {
        let shrink_check = match resolve_profile_trace(config, run) {
            Ok((_, trace_path)) => {
                let out = std::env::temp_dir().join(format!(
                    "fozzy-profile-doctor-{}.trace.fozzy",
                    uuid::Uuid::new_v4()
                ));
                match shrink_trace(
                    config,
                    TracePath::new(trace_path.clone()),
                    &ShrinkOptions {
                        out_trace_path: Some(out.clone()),
                        budget: Some(std::time::Duration::from_secs(2)),
                        aggressive: false,
                        minimize: ShrinkMinimize::All,
                    },
                ) {
                    Ok(s) => {
                        let shrunk_trace = TraceFile::read_json(Path::new(&s.out_trace_path))?;
                        let baseline = metric_value(
                            ProfileMetric::CpuTime,
                            &TraceFile::read_json(&trace_path)?,
                        )?;
                        let after = metric_value(ProfileMetric::CpuTime, &shrunk_trace)?;
                        let preserved = after >= baseline;
                        check(
                            "shrink_cpu_increase",
                            if preserved { "pass" } else { "warn" },
                            serde_json::json!(if preserved {
                                format!(
                                    "preserved contract baseline={} after={}",
                                    format_metric_value(baseline),
                                    format_metric_value(after)
                                )
                            } else {
                                format!(
                                    "no feasible shrink found that preserves increase contract baseline={} after={}",
                                    format_metric_value(baseline),
                                    format_metric_value(after)
                                )
                            }),
                        )
                    }
                    Err(err) => check(
                        "shrink_cpu_increase",
                        "fail",
                        serde_json::json!(err.to_string()),
                    ),
                }
            }
            Err(err) => check(
                "shrink_cpu_increase",
                "fail",
                serde_json::json!(err.to_string()),
            ),
        };
        if shrink_check
            .get("ok")
            .and_then(|v| v.as_bool())
            .is_some_and(|v| !v)
        {
            if let Some(detail) = shrink_check.get("detail").and_then(|v| v.as_str()) {
                issues.push(detail.to_string());
            }
        }
        checks.push(shrink_check);
    } else {
        checks.push(check(
            "shrink_cpu_increase",
            "skipped",
            serde_json::json!("skipped (use --deep for shrink+contract checks)"),
        ));
    }

    for check in &checks {
        let status = check
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("fail");
        if status == "warn"
            && let (Some(name), Some(detail)) = (
                check.get("name").and_then(|v| v.as_str()),
                check.get("detail").and_then(|v| v.as_str()),
            )
        {
            issues.push(format!("{name}: {detail}"));
        }
    }
    let ok = checks.iter().all(|c| {
        matches!(
            c.get("status").and_then(|v| v.as_str()),
            Some("pass" | "skipped")
        )
    });
    Ok(serde_json::json!({
        "schemaVersion": "fozzy.profile_doctor.v1",
        "run": run_label,
        "ok": ok,
        "checks": checks,
        "issues": issues,
    }))
}

pub(in crate::profile) fn is_diagnostic_profile_explain(explain: &ProfileExplain) -> bool {
    !explain.regression_statement.is_empty()
        && explain.regression_statement != "no measurable regression shift found"
        && !explain.regression_statement.starts_with("run ")
        && !explain.top_shifted_path.is_empty()
        && explain.top_shifted_path != "n/a"
        && !explain.likely_cause_domain.is_empty()
        && explain.likely_cause_domain != "unknown"
}

pub(in crate::profile) fn resolve_profile_trace(
    config: &Config,
    selector: &str,
) -> FozzyResult<(PathBuf, PathBuf)> {
    let (artifacts_dir, trace_path) = resolve_profile_artifacts(config, selector)?;
    if let Some(trace_path) = trace_path {
        Ok((artifacts_dir, trace_path))
    } else {
        Err(FozzyError::InvalidArgument(format!(
            "no trace.fozzy found for {selector:?}; profiler requires trace artifacts"
        )))
    }
}

pub(in crate::profile) fn resolve_profile_artifacts(
    config: &Config,
    selector: &str,
) -> FozzyResult<(PathBuf, Option<PathBuf>)> {
    match resolve_profile_source(config, selector)? {
        ResolvedProfileSource::DirectTrace {
            artifacts_dir,
            trace_path,
        } => Ok((artifacts_dir, Some(trace_path))),
        ResolvedProfileSource::Artifacts {
            artifacts_dir,
            validated_bundle,
        } => Ok((
            artifacts_dir,
            validated_bundle.and_then(|bundle| bundle.trace_path.clone()),
        )),
    }
}
