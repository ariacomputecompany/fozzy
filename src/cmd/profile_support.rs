use super::*;

pub(super) fn load_profile_bundle(
    config: &Config,
    selector: &str,
    spec: ProfileLoadSpec,
) -> FozzyResult<ProfileBundle> {
    let (artifacts_dir, trace_path) = resolve_profile_artifacts(config, selector)?;
    if let Some(trace_path) = trace_path {
        if profile_artifacts_stale(&artifacts_dir, &trace_path)? {
            let trace = TraceFile::read_json(&trace_path)?;
            write_profile_artifacts_from_trace(&trace, &artifacts_dir)?;
            refresh_manifest_for_profile_artifacts(&artifacts_dir)?;
        }
    } else if !profile_artifacts_exist(&artifacts_dir) {
        return Err(FozzyError::InvalidArgument(format!(
            "no trace.fozzy found for {selector:?}; profiler requires trace artifacts"
        )));
    }

    let metrics: ProfileMetrics =
        serde_json::from_slice(&std::fs::read(artifacts_dir.join("profile.metrics.json"))?)?;
    let timeline = if spec.timeline {
        Some(
            serde_json::from_slice::<ProfileTimelineArtifact>(&std::fs::read(
                artifacts_dir.join("profile.timeline.json"),
            )?)?
            .events,
        )
    } else {
        None
    };
    let cpu = if spec.cpu {
        Some(serde_json::from_slice(&std::fs::read(
            artifacts_dir.join("profile.cpu.json"),
        )?)?)
    } else {
        None
    };
    let heap = if spec.heap {
        Some(serde_json::from_slice(&std::fs::read(
            artifacts_dir.join("profile.heap.json"),
        )?)?)
    } else {
        None
    };
    let latency = if spec.latency {
        Some(serde_json::from_slice(&std::fs::read(
            artifacts_dir.join("profile.latency.json"),
        )?)?)
    } else {
        None
    };
    let symbols = if spec.symbols {
        Some(serde_json::from_slice(&std::fs::read(
            artifacts_dir.join("symbols.json"),
        )?)?)
    } else {
        None
    };

    Ok(ProfileBundle {
        artifacts_dir,
        timeline,
        cpu,
        heap,
        latency,
        metrics,
        symbols,
    })
}

pub(super) fn parse_selector_group(value: &str) -> Vec<String> {
    let selectors = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if selectors.is_empty() {
        vec![value.to_string()]
    } else {
        selectors
    }
}

pub(super) fn load_profile_bundle_group(
    config: &Config,
    selectors: &[String],
    spec: ProfileLoadSpec,
) -> FozzyResult<Vec<ProfileBundle>> {
    let mut bundles = Vec::<ProfileBundle>::new();
    for selector in selectors {
        bundles.push(load_profile_bundle(config, selector, spec)?);
    }
    Ok(bundles)
}

pub(super) fn aggregate_metric_bundle(
    bundles: &[ProfileBundle],
) -> FozzyResult<(ProfileMetrics, HashMap<String, MetricStats>)> {
    let first = bundles.first().ok_or_else(|| {
        FozzyError::InvalidArgument("diff requires at least one sample".to_string())
    })?;
    let values_for = |field: fn(&ProfileMetrics) -> f64| {
        bundles
            .iter()
            .map(|b| field(&b.metrics))
            .collect::<Vec<f64>>()
    };
    let mut stats = HashMap::<String, MetricStats>::new();
    for (name, values) in [
        ("virtual_time_ms", values_for(|m| m.virtual_time_ms as f64)),
        ("cpu_time_ms", values_for(|m| m.cpu_time_ms as f64)),
        ("host_time_ms", values_for(|m| m.host_time_ms as f64)),
        ("p50_latency_ms", values_for(|m| m.p50_latency_ms as f64)),
        ("p95_latency_ms", values_for(|m| m.p95_latency_ms as f64)),
        ("p99_latency_ms", values_for(|m| m.p99_latency_ms as f64)),
        ("max_latency_ms", values_for(|m| m.max_latency_ms as f64)),
        ("alloc_bytes", values_for(|m| m.alloc_bytes as f64)),
        ("in_use_bytes", values_for(|m| m.in_use_bytes as f64)),
        ("io_ops", values_for(|m| m.io_ops as f64)),
        ("sched_ops", values_for(|m| m.sched_ops as f64)),
    ] {
        stats.insert(name.to_string(), metric_stats(&values));
    }

    let mean_u64 = |name: &str, fallback: u64| {
        stats
            .get(name)
            .map(|s| s.mean.max(0.0).round() as u64)
            .unwrap_or(fallback)
    };
    let mut out = first.metrics.clone();
    out.virtual_time_ms = mean_u64("virtual_time_ms", out.virtual_time_ms);
    out.host_time_ms = mean_u64("host_time_ms", out.host_time_ms);
    out.cpu_time_ms = mean_u64("cpu_time_ms", out.cpu_time_ms);
    out.alloc_bytes = mean_u64("alloc_bytes", out.alloc_bytes);
    out.in_use_bytes = mean_u64("in_use_bytes", out.in_use_bytes);
    out.p50_latency_ms = mean_u64("p50_latency_ms", out.p50_latency_ms);
    out.p95_latency_ms = mean_u64("p95_latency_ms", out.p95_latency_ms);
    out.p99_latency_ms = mean_u64("p99_latency_ms", out.p99_latency_ms);
    out.max_latency_ms = mean_u64("max_latency_ms", out.max_latency_ms);
    out.io_ops = mean_u64("io_ops", out.io_ops);
    out.sched_ops = mean_u64("sched_ops", out.sched_ops);
    out.confidence = Some(if bundles.len() <= 1 { 0.8 } else { 0.9 });
    Ok((out, stats))
}

fn metric_stats(values: &[f64]) -> MetricStats {
    if values.is_empty() {
        return MetricStats::default();
    }
    let n = values.len();
    let mean = values.iter().copied().sum::<f64>() / n as f64;
    let variance = values
        .iter()
        .map(|v| {
            let d = *v - mean;
            d * d
        })
        .sum::<f64>()
        / n as f64;
    MetricStats {
        n,
        mean,
        std_dev: variance.sqrt(),
    }
}

pub(super) fn top_by_tag(
    timeline: &[ProfileEvent],
    kind: ProfileEventKind,
    limit: usize,
) -> Vec<serde_json::Value> {
    let mut counts = BTreeMap::<String, u64>::new();
    for event in timeline {
        if event.kind != kind {
            continue;
        }
        let name = event
            .tags
            .get("name")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        *counts.entry(name).or_insert(0) += 1;
    }
    let mut rows = counts.into_iter().collect::<Vec<_>>();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.into_iter()
        .take(limit)
        .map(|(name, count)| serde_json::json!({"name": name, "count": count}))
        .collect()
}

pub(super) fn empty_domain(domain: &str, reason: &str) -> serde_json::Value {
    serde_json::json!({
        "domain": domain,
        "empty": true,
        "reason": reason,
    })
}

pub(super) fn profile_env_report(config: &Config, strict: bool) -> serde_json::Value {
    let collector = detect_cpu_collector_capability();
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    serde_json::json!({
        "schemaVersion": "fozzy.profile_env.v4",
        "strict": strict,
        "determinismContract": {
            "replayBoundTo": "deterministic_decisions_and_virtual_events",
            "nonDeterministicMeasurements": ["cpu_time_ms", "host_time_ms"],
        },
        "host": {
            "os": os,
            "arch": arch,
        },
        "backends": {
            "proc": format!("{:?}", config.proc_backend).to_lowercase(),
            "fs": format!("{:?}", config.fs_backend).to_lowercase(),
            "http": format!("{:?}", config.http_backend).to_lowercase(),
        },
        "domains": {
            "cpu": {
                "available": false,
                "quality": "unsupported",
                "primaryCollector": collector.primary_collector,
                "activeCollector": collector.active_collector,
                "linuxPerfEventOpen": collector.linux_perf_event_open,
                "samplePeriodMs": collector.sample_period_ms,
                "diagnostics": collector.diagnostics,
                "notes": "CPU profiling is disabled for production traces until the runtime emits real sample events; span-derived pseudo-CPU profiles are rejected."
            },
            "heap": {
                "available": true,
                "quality": "high",
                "notes": "derived from memory_alloc/memory_free events in trace"
            },
            "latency": {
                "available": true,
                "quality": "high",
                "notes": "derived from deterministic trace timeline deltas"
            },
            "io": {
                "available": true,
                "quality": "high",
                "notes": "derived from io/net event counts in trace"
            },
            "sched": {
                "available": true,
                "quality": "high",
                "notes": "derived from distributed scheduler events in trace"
            }
        }
    })
}

pub(super) fn profile_doctor(
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

fn is_diagnostic_profile_explain(explain: &ProfileExplain) -> bool {
    !explain.regression_statement.is_empty()
        && explain.regression_statement != "no measurable regression shift found"
        && !explain.regression_statement.starts_with("run ")
        && !explain.top_shifted_path.is_empty()
        && explain.top_shifted_path != "n/a"
        && !explain.likely_cause_domain.is_empty()
        && explain.likely_cause_domain != "unknown"
}

pub(super) fn resolve_profile_trace(
    config: &Config,
    selector: &str,
) -> FozzyResult<(PathBuf, PathBuf)> {
    let (artifacts_dir, trace_path) = resolve_profile_artifacts(config, selector)?;
    if let Some(trace_path) = trace_path {
        return Ok((artifacts_dir, trace_path));
    }
    Err(FozzyError::InvalidArgument(format!(
        "no trace.fozzy found for {selector:?}; profiler requires trace artifacts"
    )))
}

pub(super) fn resolve_profile_artifacts(
    config: &Config,
    selector: &str,
) -> FozzyResult<(PathBuf, Option<PathBuf>)> {
    let input = PathBuf::from(crate::normalize_run_or_trace_selector(selector));
    if input.exists() && input.is_file() && crate::is_trace_path(&input) {
        let trace = TraceFile::read_json(&input)?;
        if let Some(artifacts_dir) = trace
            .summary
            .identity
            .artifacts_dir
            .as_deref()
            .map(PathBuf::from)
            .filter(|dir| dir.exists() && dir.is_dir())
        {
            return Ok((artifacts_dir, Some(input)));
        }
        let canonical = std::fs::canonicalize(&input).unwrap_or_else(|_| input.clone());
        let key = blake3::hash(canonical.to_string_lossy().as_bytes())
            .to_hex()
            .to_string();
        let dir = config.base_dir.join("profile-cache").join(key);
        return Ok((dir, Some(input)));
    }

    let artifacts_dir = resolve_artifacts_dir(config, selector)?;
    let mut checked_summary = None;
    if artifacts_dir.join("report.json").exists() {
        checked_summary =
            crate::load_checked_report_summary_from_artifacts_dir(&artifacts_dir, selector)?;
    }
    if let Some(trace_path) = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)? {
        return Ok((artifacts_dir, Some(trace_path)));
    }
    if checked_summary.is_none() && profile_artifacts_exist(&artifacts_dir) {
        return Err(FozzyError::InvalidArgument(format!(
            "no coherent report/manifest pair or trace found for profile artifacts in {}",
            artifacts_dir.display()
        )));
    }

    Ok((artifacts_dir, None))
}

fn refresh_manifest_for_profile_artifacts(artifacts_dir: &Path) -> FozzyResult<()> {
    let Some(summary) = crate::load_checked_report_summary_from_artifacts_dir(
        artifacts_dir,
        &artifacts_dir.display().to_string(),
    )?
    else {
        return Ok(());
    };
    crate::write_run_manifest(&summary, artifacts_dir)?;
    Ok(())
}

fn profile_artifacts_exist(artifacts_dir: &Path) -> bool {
    for name in [
        "profile.timeline.json",
        "profile.cpu.json",
        "profile.heap.json",
        "profile.latency.json",
        "profile.metrics.json",
        "symbols.json",
    ] {
        if !artifacts_dir.join(name).exists() {
            return false;
        }
    }
    true
}

pub(super) fn normalize_domains(
    cpu: bool,
    heap: bool,
    latency: bool,
    io: bool,
    sched: bool,
) -> Vec<String> {
    if !cpu && !heap && !latency && !io && !sched {
        return vec![
            "cpu".to_string(),
            "io".to_string(),
            "sched".to_string(),
            "heap".to_string(),
            "latency".to_string(),
        ];
    }
    let mut out = Vec::new();
    if cpu {
        out.push("cpu".to_string());
    }
    if heap {
        out.push("heap".to_string());
    }
    if latency {
        out.push("latency".to_string());
    }
    if io {
        out.push("io".to_string());
    }
    if sched {
        out.push("sched".to_string());
    }
    out
}

pub(super) fn enforce_cpu_contract(
    strict: bool,
    cpu_requested: bool,
    sample_counts: &[usize],
) -> FozzyResult<()> {
    let _ = strict;
    if !cpu_requested {
        return Ok(());
    }
    let sample_count = sample_counts.iter().copied().min().unwrap_or(0);
    if sample_count == 0 {
        return Err(FozzyError::InvalidArgument(
            "cpu profiling requires real sample events in the trace; current trace has none. rerun once production CPU sample capture is implemented, or use heap/latency/io/sched domains instead.".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn detect_cpu_collector_capability() -> CpuCollectorCapability {
    let fallback = "in_process_sampler".to_string();
    if cfg!(target_os = "linux") {
        let mut diagnostics = Vec::<String>::new();
        let perf_device_present = Path::new("/sys/bus/event_source/devices/cpu/type").exists();
        diagnostics.push(format!("perf_event_device_present={perf_device_present}"));

        let paranoid = read_proc_int("/proc/sys/kernel/perf_event_paranoid");
        if let Some(v) = paranoid {
            diagnostics.push(format!("perf_event_paranoid={v}"));
        } else {
            diagnostics.push("perf_event_paranoid=unknown".to_string());
        }

        let kptr = read_proc_int("/proc/sys/kernel/kptr_restrict");
        if let Some(v) = kptr {
            diagnostics.push(format!("kptr_restrict={v}"));
        }

        let perf_allowed = perf_device_present && paranoid.is_some_and(|v| v <= 2);
        let active = if perf_allowed {
            "perf_event_open".to_string()
        } else {
            fallback.clone()
        };
        if !perf_allowed {
            diagnostics.push(
                "falling back to in_process_sampler (perf_event_open unavailable for current permissions)"
                    .to_string(),
            );
        }
        CpuCollectorCapability {
            primary_collector: "perf_event_open".to_string(),
            fallback_collector: fallback,
            active_collector: active,
            linux_perf_event_open: perf_allowed,
            diagnostics,
            sample_period_ms: 10,
        }
    } else if cfg!(target_os = "macos") {
        CpuCollectorCapability {
            primary_collector: "mach_thread_sampler".to_string(),
            fallback_collector: fallback.clone(),
            active_collector: fallback,
            linux_perf_event_open: false,
            diagnostics: vec![
                "mach thread cpu sampling is not wired into trace emission".to_string(),
                "cpu domain remains unavailable until runtime sample events are recorded"
                    .to_string(),
            ],
            sample_period_ms: 10,
        }
    } else {
        CpuCollectorCapability {
            primary_collector: "perf_event_open".to_string(),
            fallback_collector: fallback.clone(),
            active_collector: fallback,
            linux_perf_event_open: false,
            diagnostics: vec![
                "cpu sample capture is not available on this platform in production traces"
                    .to_string(),
            ],
            sample_period_ms: 10,
        }
    }
}

fn read_proc_int(path: &str) -> Option<i64> {
    let raw = std::fs::read_to_string(path).ok()?;
    raw.trim().parse::<i64>().ok()
}

pub(super) fn write_json(path: &Path, value: &impl Serialize) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec(value)?)?;
    Ok(())
}

fn profile_artifacts_stale(artifacts_dir: &Path, trace_path: &Path) -> FozzyResult<bool> {
    if !profile_artifacts_exist(artifacts_dir) {
        return Ok(true);
    }
    let trace_mtime = std::fs::metadata(trace_path)?.modified()?;
    for name in [
        "profile.timeline.json",
        "profile.cpu.json",
        "profile.heap.json",
        "profile.latency.json",
        "profile.metrics.json",
        "symbols.json",
    ] {
        let p = artifacts_dir.join(name);
        let md = std::fs::metadata(&p)?;
        if md.modified()? < trace_mtime {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn write_text(path: &Path, value: &str) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, value)?;
    Ok(())
}
