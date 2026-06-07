use super::super::*;
use super::*;

#[derive(Debug, Clone)]
pub(in crate::profile) enum ResolvedProfileSource {
    DirectTrace {
        artifacts_dir: PathBuf,
        trace_path: PathBuf,
    },
    Artifacts {
        artifacts_dir: PathBuf,
        validated_bundle: Option<crate::ValidatedArtifactBundle>,
    },
}

pub(in crate::profile) fn load_profile_bundle(
    config: &Config,
    selector: &str,
    spec: ProfileLoadSpec,
) -> FozzyResult<ProfileBundle> {
    let source = resolve_profile_source(config, selector)?;
    let (artifacts_dir, trace_path, expected_identity) = match &source {
        ResolvedProfileSource::DirectTrace {
            artifacts_dir,
            trace_path,
        } => {
            let trace = crate::read_cached_trace_file(trace_path)?;
            (
                artifacts_dir.clone(),
                Some(trace_path.clone()),
                Some((trace.summary.identity.run_id, trace.summary.identity.seed)),
            )
        }
        ResolvedProfileSource::Artifacts {
            artifacts_dir,
            validated_bundle,
        } => {
            let trace_path = validated_bundle
                .as_ref()
                .and_then(|bundle| bundle.trace_path.clone());
            let expected_identity = if let Some(trace_path) = trace_path.as_ref() {
                let trace = crate::read_cached_trace_file(trace_path)?;
                Some((trace.summary.identity.run_id, trace.summary.identity.seed))
            } else {
                validated_bundle.as_ref().map(|bundle| {
                    (
                        bundle.summary.identity.run_id.clone(),
                        bundle.summary.identity.seed,
                    )
                })
            };
            (artifacts_dir.clone(), trace_path, expected_identity)
        }
    };
    if let Some(ref trace_path) = trace_path {
        if profile_artifacts_stale(&artifacts_dir, &trace_path)? {
            let trace = crate::read_cached_trace_file(trace_path)?;
            let bundle = build_profile_bundle_from_trace(&trace, &artifacts_dir, spec);
            if let Some((expected_run_id, expected_seed)) = expected_identity {
                validate_profile_bundle_identity(&bundle, &expected_run_id, expected_seed)?;
            }
            return Ok(bundle);
        }
    } else if !profile_artifacts_exist(&artifacts_dir) {
        return Err(FozzyError::InvalidArgument(format!(
            "no trace.fozzy found for {selector:?}; profiler requires trace artifacts"
        )));
    }

    let bundle = match read_profile_bundle_from_dir(&artifacts_dir, spec) {
        Ok(bundle) => bundle,
        Err(err) => {
            if let Some(trace_path) = trace_path.clone() {
                let trace = crate::read_cached_trace_file(&trace_path)?;
                let bundle = build_profile_bundle_from_trace(&trace, &artifacts_dir, spec);
                if let Some((expected_run_id, expected_seed)) = expected_identity.clone() {
                    validate_profile_bundle_identity(&bundle, &expected_run_id, expected_seed)?;
                }
                bundle
            } else {
                return Err(err);
            }
        }
    };
    if let Some((expected_run_id, expected_seed)) = expected_identity {
        if let Err(err) = validate_profile_bundle_identity(&bundle, &expected_run_id, expected_seed)
        {
            if let Some(trace_path) = trace_path.clone() {
                let trace = crate::read_cached_trace_file(&trace_path)?;
                let rebuilt = build_profile_bundle_from_trace(&trace, &artifacts_dir, spec);
                validate_profile_bundle_identity(&rebuilt, &expected_run_id, expected_seed)?;
                return Ok(rebuilt);
            }
            return Err(err);
        }
    }
    Ok(bundle)
}

fn build_profile_bundle_from_trace(
    trace: &TraceFile,
    artifacts_dir: &Path,
    spec: ProfileLoadSpec,
) -> ProfileBundle {
    let timeline = build_profile_timeline(trace);
    let cpu_profile = build_cpu_profile(trace, &timeline);
    let heap_profile = build_heap_profile(trace, &timeline);
    let latency_profile = build_latency_profile(trace, &timeline);
    let symbols = build_symbols_map(trace, &timeline, &cpu_profile);
    let metrics = build_profile_metrics(
        trace,
        &timeline,
        &cpu_profile,
        &heap_profile,
        &latency_profile,
    );

    ProfileBundle {
        artifacts_dir: artifacts_dir.to_path_buf(),
        timeline: spec.timeline.then_some(timeline),
        cpu: spec.cpu.then_some(cpu_profile),
        heap: spec.heap.then_some(heap_profile),
        latency: spec.latency.then_some(latency_profile),
        metrics,
        symbols: spec.symbols.then_some(symbols),
    }
}

pub(in crate::profile) fn read_profile_bundle_from_dir(
    artifacts_dir: &Path,
    spec: ProfileLoadSpec,
) -> FozzyResult<ProfileBundle> {
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
        artifacts_dir: artifacts_dir.to_path_buf(),
        timeline,
        cpu,
        heap,
        latency,
        metrics,
        symbols,
    })
}

pub(in crate::profile) fn parse_selector_group(value: &str) -> Vec<String> {
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

pub(in crate::profile) fn load_profile_bundle_group(
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

pub(in crate::profile) fn aggregate_metric_bundle(
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

pub(in crate::profile) fn metric_stats(values: &[f64]) -> MetricStats {
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

pub(in crate::profile) fn validate_profile_bundle_identity(
    bundle: &ProfileBundle,
    expected_run_id: &str,
    expected_seed: u64,
) -> FozzyResult<()> {
    if bundle.metrics.run_id != expected_run_id {
        return Err(FozzyError::InvalidArgument(format!(
            "profile metrics in {} belong to runId={}, expected {}",
            bundle.artifacts_dir.join("profile.metrics.json").display(),
            bundle.metrics.run_id,
            expected_run_id
        )));
    }
    if let Some(timeline) = bundle.timeline.as_ref() {
        for event in timeline {
            if event.run_id != expected_run_id || event.seed != expected_seed {
                return Err(FozzyError::InvalidArgument(format!(
                    "profile timeline in {} contains event identity runId={} seed={}, expected runId={} seed={}",
                    bundle.artifacts_dir.join("profile.timeline.json").display(),
                    event.run_id,
                    event.seed,
                    expected_run_id,
                    expected_seed
                )));
            }
        }
    }
    if let Some(cpu) = bundle.cpu.as_ref()
        && cpu.run_id != expected_run_id
    {
        return Err(FozzyError::InvalidArgument(format!(
            "profile cpu artifact in {} belongs to runId={}, expected {}",
            bundle.artifacts_dir.join("profile.cpu.json").display(),
            cpu.run_id,
            expected_run_id
        )));
    }
    if let Some(heap) = bundle.heap.as_ref()
        && heap.run_id != expected_run_id
    {
        return Err(FozzyError::InvalidArgument(format!(
            "profile heap artifact in {} belongs to runId={}, expected {}",
            bundle.artifacts_dir.join("profile.heap.json").display(),
            heap.run_id,
            expected_run_id
        )));
    }
    if let Some(latency) = bundle.latency.as_ref()
        && latency.run_id != expected_run_id
    {
        return Err(FozzyError::InvalidArgument(format!(
            "profile latency artifact in {} belongs to runId={}, expected {}",
            bundle.artifacts_dir.join("profile.latency.json").display(),
            latency.run_id,
            expected_run_id
        )));
    }
    if let Some(symbols) = bundle.symbols.as_ref()
        && symbols.run_id != expected_run_id
    {
        return Err(FozzyError::InvalidArgument(format!(
            "profile symbols artifact in {} belongs to runId={}, expected {}",
            bundle.artifacts_dir.join("symbols.json").display(),
            symbols.run_id,
            expected_run_id
        )));
    }
    Ok(())
}

pub(in crate::profile) fn top_by_tag(
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

pub(in crate::profile) fn empty_domain(domain: &str, reason: &str) -> serde_json::Value {
    serde_json::json!({
        "domain": domain,
        "empty": true,
        "reason": reason,
    })
}

pub(in crate::profile) fn profile_env_report(config: &Config, strict: bool) -> serde_json::Value {
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
