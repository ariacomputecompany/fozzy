use super::super::*;
use super::*;
pub(in crate::profile) fn resolve_profile_source(
    config: &Config,
    selector: &str,
) -> FozzyResult<ResolvedProfileSource> {
    let input = PathBuf::from(crate::normalize_run_or_trace_selector(selector));
    let direct_trace_input = input.exists() && input.is_file() && crate::is_trace_path(&input);
    if direct_trace_input {
        if let Some(artifacts_dir) = trusted_explicit_profile_artifacts_dir(&input)? {
            return Ok(ResolvedProfileSource::DirectTrace {
                artifacts_dir,
                trace_path: input,
            });
        }
        let canonical = std::fs::canonicalize(&input).unwrap_or_else(|_| input.clone());
        let key = blake3::hash(canonical.to_string_lossy().as_bytes())
            .to_hex()
            .to_string();
        let dir = config.base_dir.join("profile-cache").join(key);
        return Ok(ResolvedProfileSource::DirectTrace {
            artifacts_dir: dir,
            trace_path: input,
        });
    }

    let artifacts_dir = resolve_artifacts_dir(config, selector)?;
    let report_exists = artifacts_dir.join("report.json").exists();
    let manifest_exists = artifacts_dir.join("manifest.json").exists();
    let resolved_trace = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)?;
    let validated_bundle =
        crate::load_validated_artifact_bundle_from_dir(&artifacts_dir, selector)?;
    if validated_bundle.is_some() {
        return Ok(ResolvedProfileSource::Artifacts {
            artifacts_dir,
            validated_bundle,
        });
    }
    if resolved_trace.is_some() {
        return Err(FozzyError::InvalidArgument(format!(
            "no coherent report/manifest pair found for profile trace artifacts in {}",
            artifacts_dir.display()
        )));
    }
    if profile_artifacts_exist(&artifacts_dir) || report_exists || manifest_exists {
        return Err(FozzyError::InvalidArgument(format!(
            "no coherent report/manifest pair or trace found for profile artifacts in {}",
            artifacts_dir.display()
        )));
    }

    Ok(ResolvedProfileSource::Artifacts {
        artifacts_dir,
        validated_bundle: None,
    })
}

pub(in crate::profile) fn trusted_explicit_profile_artifacts_dir(
    trace_path: &Path,
) -> FozzyResult<Option<PathBuf>> {
    if let Some(artifacts_dir) = crate::declared_artifacts_dir_for_trace(trace_path)?
        && profile_artifacts_exist(&artifacts_dir)
        && !profile_artifacts_stale(&artifacts_dir, trace_path)?
    {
        return Ok(Some(artifacts_dir));
    }
    Ok(crate::trusted_artifact_bundle_for_trace(trace_path)?.map(|bundle| bundle.artifacts_dir))
}

pub(in crate::profile) fn profile_artifacts_exist(artifacts_dir: &Path) -> bool {
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

pub(in crate::profile) fn normalize_domains(
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

pub(in crate::profile) fn enforce_cpu_contract(
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

pub(in crate::profile) fn detect_cpu_collector_capability() -> CpuCollectorCapability {
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

pub(in crate::profile) fn read_proc_int(path: &str) -> Option<i64> {
    let raw = std::fs::read_to_string(path).ok()?;
    raw.trim().parse::<i64>().ok()
}

pub(in crate::profile) fn write_json(path: &Path, value: &impl Serialize) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec(value)?)?;
    Ok(())
}

pub(in crate::profile) fn profile_artifacts_stale(
    artifacts_dir: &Path,
    trace_path: &Path,
) -> FozzyResult<bool> {
    if !profile_artifacts_exist(artifacts_dir) {
        return Ok(true);
    }
    let source_path = artifacts_dir.join("profile.source.json");
    if !source_path.exists() {
        return Ok(true);
    }
    let source: serde_json::Value = serde_json::from_slice(&std::fs::read(&source_path)?)?;
    let recorded_path = source
        .get("tracePath")
        .and_then(|v| v.as_str())
        .map(PathBuf::from);
    let recorded_size = source.get("traceSizeBytes").and_then(|v| v.as_u64());
    let recorded_modified_ns = source.get("traceModifiedNs").and_then(|v| v.as_u64());
    let recorded_digest = source.get("traceDigest").and_then(|v| v.as_str());
    let expected_path =
        std::fs::canonicalize(trace_path).unwrap_or_else(|_| trace_path.to_path_buf());
    if recorded_path.as_deref() != Some(expected_path.as_path()) {
        return Ok(true);
    }
    let fingerprint = crate::FileFingerprint::for_path(trace_path)?;
    if let (Some(size), Some(modified_ns)) = (recorded_size, recorded_modified_ns) {
        if size != fingerprint.len || u128::from(modified_ns) != fingerprint.modified_ns {
            return Ok(true);
        }
    } else {
        let expected_digest = blake3::hash(&std::fs::read(trace_path)?)
            .to_hex()
            .to_string();
        if recorded_digest != Some(expected_digest.as_str()) {
            return Ok(true);
        }
    }
    let trace_mtime = std::fs::metadata(trace_path)?.modified()?;
    for name in [
        "profile.timeline.json",
        "profile.cpu.json",
        "profile.heap.json",
        "profile.latency.json",
        "profile.metrics.json",
        "symbols.json",
        "profile.source.json",
    ] {
        let p = artifacts_dir.join(name);
        let md = std::fs::metadata(&p)?;
        if md.modified()? < trace_mtime {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(in crate::profile) fn write_text(path: &Path, value: &str) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, value)?;
    Ok(())
}
