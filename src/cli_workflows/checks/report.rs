use super::*;

pub(crate) fn replay_summary_status(
    expected: Option<ExitStatus>,
    summary: &RunSummary,
    strict: bool,
    expected_seed: u64,
    expected_mode: RunMode,
) -> (FullStepStatus, String) {
    let class_ok = expected
        .map(|s| (s == ExitStatus::Pass) == (summary.status == ExitStatus::Pass))
        .unwrap_or(false);
    let strict_ok = enforce_strict_summary(strict, summary).is_ok();
    let run_id_present = !summary.identity.run_id.trim().is_empty();
    let seed_matches = summary.identity.seed == expected_seed;
    let mode_matches = summary.mode == expected_mode;
    let (artifact_identity_ok, artifact_identity_detail) =
        summary_artifact_identity_status(summary);
    (
        if class_ok
            && strict_ok
            && run_id_present
            && seed_matches
            && mode_matches
            && artifact_identity_ok
        {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "status={:?} class_ok={} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} {}",
            summary.status,
            class_ok,
            strict_ok,
            run_id_present,
            seed_matches,
            expected_seed,
            mode_matches,
            expected_mode,
            artifact_identity_detail
        ),
    )
}

pub(crate) fn file_artifact_status(path: &Path) -> (FullStepStatus, String) {
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

pub(crate) fn zip_artifact_status(path: &Path) -> (FullStepStatus, String) {
    let (file_status, file_detail) = file_artifact_status(path);
    if !matches!(file_status, FullStepStatus::Passed) {
        return (file_status, file_detail);
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(err) => {
            return (
                FullStepStatus::Failed,
                format!("{file_detail} zip_open_error={err}"),
            );
        }
    };
    let archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(err) => {
            return (
                FullStepStatus::Failed,
                format!("{file_detail} zip_parse_error={err}"),
            );
        }
    };
    let entries = archive.len();
    (
        if entries > 0 {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!("{file_detail} zip_entries={entries}"),
    )
}

pub(crate) fn report_show_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let format = value
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("pretty");
    let known_format = matches!(format, "pretty" | "junit" | "html");
    let content = value.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let bytes = content.len();
    let non_blank = !content.trim().is_empty();
    (
        if bytes > 0 && known_format && non_blank {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "format={format} known_format={} non_blank={} content_bytes={bytes}",
            known_format, non_blank
        ),
    )
}

pub(crate) fn report_query_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let status_value = value
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| value.to_string());
    (
        if status_value == "pass" {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(".status={status_value}"),
    )
}

pub(crate) fn report_query_paths_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let paths = value
        .get("paths")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let count = paths.len();
    let mut seen = std::collections::BTreeSet::new();
    let invalid = paths
        .iter()
        .filter(|path| path.as_str().is_none_or(|s| s.trim().is_empty()))
        .count();
    let duplicate = paths
        .iter()
        .filter_map(|path| path.as_str().map(str::trim))
        .filter(|s| !s.is_empty())
        .filter(|path| !seen.insert((*path).to_string()))
        .count();
    (
        if count > 0 && invalid == 0 && duplicate == 0 {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!("paths={count} invalid={invalid} duplicate={duplicate}"),
    )
}

pub(crate) fn summary_artifact_identity_status(summary: &RunSummary) -> (bool, String) {
    let report_path = summary.identity.report_path.as_deref().map(str::trim);
    let artifacts_dir = summary.identity.artifacts_dir.as_deref().map(str::trim);
    let report_present = report_path.is_some_and(|path| !path.is_empty());
    let artifacts_present = artifacts_dir.is_some_and(|path| !path.is_empty());
    let report_path = report_path
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let artifacts_dir = artifacts_dir
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let report_exists = report_path.as_ref().is_some_and(|path| {
        std::fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.len() > 0)
            .unwrap_or(false)
    });
    let artifacts_exists = artifacts_dir.as_ref().is_some_and(|path| {
        std::fs::metadata(path)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
    });
    let report_matches_dir = report_path
        .as_ref()
        .zip(artifacts_dir.as_ref())
        .is_some_and(|(report, dir)| {
            report.parent().is_some_and(|parent| parent == dir)
                && report.file_name().is_some_and(|name| name == "report.json")
                && dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == summary.identity.run_id)
        });
    let manifest_path = artifacts_dir.as_ref().map(|dir| dir.join("manifest.json"));
    let manifest_exists = manifest_path.as_ref().is_some_and(|path| {
        std::fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.len() > 0)
            .unwrap_or(false)
    });
    let report_content_matches = report_path.as_ref().is_some_and(|path| {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<RunSummary>(&bytes).ok())
            .is_some_and(|report| {
                report.identity.run_id == summary.identity.run_id
                    && report.identity.seed == summary.identity.seed
                    && report.mode == summary.mode
                    && report.status == summary.status
                    && report.identity.report_path == summary.identity.report_path
                    && report.identity.artifacts_dir == summary.identity.artifacts_dir
            })
    });
    let manifest_content_matches = manifest_path.as_ref().is_some_and(|path| {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<fozzy::RunManifest>(&bytes).ok())
            .is_some_and(|manifest| {
                manifest.run_id == summary.identity.run_id
                    && manifest.seed == summary.identity.seed
                    && manifest.mode == summary.mode
                    && manifest.status == summary.status
                    && manifest.report_path == summary.identity.report_path
                    && manifest.artifacts_dir == summary.identity.artifacts_dir
                    && manifest.trace_path == summary.identity.trace_path
                    && manifest.findings_count == summary.findings.len()
                    && manifest.duration_ms == summary.duration_ms
                    && manifest.duration_ns == summary.duration_ns
            })
    });
    (
        report_present
            && artifacts_present
            && report_exists
            && artifacts_exists
            && report_matches_dir
            && report_content_matches
            && manifest_exists
            && manifest_content_matches,
        format!(
            "report_present={} artifacts_present={} report_exists={} artifacts_exists={} report_matches_dir={} report_content_matches={} manifest_exists={} manifest_content_matches={}",
            report_present,
            artifacts_present,
            report_exists,
            artifacts_exists,
            report_matches_dir,
            report_content_matches,
            manifest_exists,
            manifest_content_matches
        ),
    )
}

pub(crate) fn trace_summary_identity_status(
    trace_path: &Path,
    summary: &RunSummary,
) -> (bool, String) {
    let trace_exists = std::fs::metadata(trace_path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false);
    let trace_content_matches = std::fs::read(trace_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<fozzy::TraceFile>(&bytes).ok())
        .is_some_and(|trace| {
            trace.summary.identity.run_id == summary.identity.run_id
                && trace.summary.identity.seed == summary.identity.seed
                && trace.summary.mode == summary.mode
                && trace.summary.status == summary.status
                && trace.summary.identity.trace_path == summary.identity.trace_path
                && trace.summary.identity.report_path == summary.identity.report_path
                && trace.summary.identity.artifacts_dir == summary.identity.artifacts_dir
        });
    (
        trace_exists && trace_content_matches,
        format!(
            "trace_exists={} trace_content_matches={}",
            trace_exists, trace_content_matches
        ),
    )
}

pub(crate) fn run_summary_pass_status(
    summary: &RunSummary,
    strict: bool,
    expected_seed: u64,
    expected_mode: RunMode,
) -> (FullStepStatus, String) {
    let strict_ok = enforce_strict_summary(strict, summary).is_ok();
    let run_id_present = !summary.identity.run_id.trim().is_empty();
    let seed_matches = summary.identity.seed == expected_seed;
    let mode_matches = summary.mode == expected_mode;
    let (artifact_identity_ok, artifact_identity_detail) =
        summary_artifact_identity_status(summary);
    (
        if summary.status == ExitStatus::Pass
            && strict_ok
            && run_id_present
            && seed_matches
            && mode_matches
            && artifact_identity_ok
        {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "status={:?} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} {}",
            summary.status,
            strict_ok,
            run_id_present,
            seed_matches,
            expected_seed,
            mode_matches,
            expected_mode,
            artifact_identity_detail
        ),
    )
}

pub(crate) fn recorded_trace_status(
    summary: &RunSummary,
    strict: bool,
    expected_seed: u64,
    expected_mode: RunMode,
    trace_path: &Path,
) -> (FullStepStatus, String) {
    let (summary_status, summary_detail) =
        run_summary_pass_status(summary, strict, expected_seed, expected_mode);
    let (file_status, file_detail) = file_artifact_status(trace_path);
    let (trace_identity_ok, trace_identity_detail) =
        trace_summary_identity_status(trace_path, summary);
    let reported_trace = summary.identity.trace_path.as_deref();
    let reported_matches = reported_trace.is_some_and(|reported| Path::new(reported) == trace_path);
    let status = if reported_matches
        && matches!(summary_status, FullStepStatus::Passed)
        && matches!(file_status, FullStepStatus::Passed)
        && trace_identity_ok
    {
        FullStepStatus::Passed
    } else {
        FullStepStatus::Failed
    };
    (
        status,
        format!(
            "{} trace_reported={} trace_matches={} {} {}",
            summary_detail,
            reported_trace.is_some(),
            reported_matches,
            file_detail,
            trace_identity_detail
        ),
    )
}

pub(crate) fn shrink_step_status(
    primary_status: Option<ExitStatus>,
    summary: &RunSummary,
    strict: bool,
    expected_seed: u64,
    expected_mode: RunMode,
    allow_expected_failures: bool,
    out_trace: &Path,
) -> (FullStepStatus, String, String) {
    let strict_ok = enforce_strict_summary(strict, summary).is_ok();
    let run_id_present = !summary.identity.run_id.trim().is_empty();
    let seed_matches = summary.identity.seed == expected_seed;
    let mode_matches = summary.mode == expected_mode;
    let (file_status, file_detail) = file_artifact_status(out_trace);
    let artifact_ok = matches!(file_status, FullStepStatus::Passed);
    let (trace_identity_ok, trace_identity_detail) =
        trace_summary_identity_status(out_trace, summary);
    let reported_trace = summary.identity.trace_path.as_deref();
    let reported_matches = reported_trace.is_some_and(|reported| Path::new(reported) == out_trace);
    if allow_expected_failures {
        match primary_status {
            Some(primary) => {
                let class_ok = shrink_status_matches(primary, summary.status);
                let classification = if class_ok
                    && strict_ok
                    && run_id_present
                    && seed_matches
                    && mode_matches
                    && artifact_ok
                    && reported_matches
                    && trace_identity_ok
                {
                    "expected_fail_class_preserved"
                } else if !class_ok {
                    "expected_fail_class_mismatch"
                } else if !run_id_present {
                    "run_identity_missing"
                } else if !seed_matches {
                    "seed_mismatch"
                } else if !mode_matches {
                    "mode_mismatch"
                } else if !artifact_ok {
                    "out_trace_missing"
                } else if !reported_matches {
                    "out_trace_identity_mismatch"
                } else if !trace_identity_ok {
                    "out_trace_content_mismatch"
                } else {
                    "strict_policy_rejected"
                };
                (
                    if class_ok
                        && strict_ok
                        && run_id_present
                        && seed_matches
                        && mode_matches
                        && artifact_ok
                        && reported_matches
                        && trace_identity_ok
                    {
                        FullStepStatus::Passed
                    } else {
                        FullStepStatus::Failed
                    },
                    format!(
                        "status={:?} class_ok={} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} trace_reported={} trace_matches={} {} {}",
                        summary.status,
                        class_ok,
                        strict_ok,
                        run_id_present,
                        seed_matches,
                        expected_seed,
                        mode_matches,
                        expected_mode,
                        reported_trace.is_some(),
                        reported_matches,
                        file_detail,
                        trace_identity_detail
                    ),
                    classification.to_string(),
                )
            }
            None => (
                FullStepStatus::Failed,
                format!(
                    "status={:?} class_ok=false strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} trace_reported={} trace_matches={} {} {}",
                    summary.status,
                    strict_ok,
                    run_id_present,
                    seed_matches,
                    expected_seed,
                    mode_matches,
                    expected_mode,
                    reported_trace.is_some(),
                    reported_matches,
                    file_detail,
                    trace_identity_detail
                ),
                "primary_status_missing".to_string(),
            ),
        }
    } else if summary.status == ExitStatus::Pass
        && strict_ok
        && run_id_present
        && seed_matches
        && mode_matches
        && artifact_ok
        && reported_matches
        && trace_identity_ok
    {
        (
            FullStepStatus::Passed,
            format!(
                "status={:?} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} trace_reported={} trace_matches={} {} {}",
                summary.status,
                strict_ok,
                run_id_present,
                seed_matches,
                expected_seed,
                mode_matches,
                expected_mode,
                reported_trace.is_some(),
                reported_matches,
                file_detail,
                trace_identity_detail
            ),
            "pass_required_policy".to_string(),
        )
    } else {
        let classification = if summary.status != ExitStatus::Pass {
            "policy_rejected_non_pass"
        } else if !run_id_present {
            "run_identity_missing"
        } else if !seed_matches {
            "seed_mismatch"
        } else if !mode_matches {
            "mode_mismatch"
        } else if !artifact_ok {
            "out_trace_missing"
        } else if !reported_matches {
            "out_trace_identity_mismatch"
        } else if !trace_identity_ok {
            "out_trace_content_mismatch"
        } else {
            "strict_policy_rejected"
        };
        (
            FullStepStatus::Failed,
            format!(
                "status={:?} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} trace_reported={} trace_matches={} {} {}",
                summary.status,
                strict_ok,
                run_id_present,
                seed_matches,
                expected_seed,
                mode_matches,
                expected_mode,
                reported_trace.is_some(),
                reported_matches,
                file_detail,
                trace_identity_detail
            ),
            classification.to_string(),
        )
    }
}
