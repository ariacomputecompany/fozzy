use std::path::{Path, PathBuf};

use crate::{Config, ExitStatus, FozzyError, FozzyResult, RunManifest, RunSummary, TraceFile};

pub(crate) fn resolve_artifacts_dir(config: &Config, run: &str) -> FozzyResult<PathBuf> {
    let path = PathBuf::from(crate::normalize_run_or_trace_selector(run));
    if path.exists() {
        if path.is_dir() {
            return Ok(path);
        }

        if path.is_file()
            && crate::is_trace_path(&path)
            && let Some(artifacts_dir) = trusted_trace_declared_artifacts_dir(&path)?
        {
            return Ok(artifacts_dir);
        }

        if let Some(parent) = path.parent() {
            return Ok(parent.to_path_buf());
        }
    }

    if let Some(alias_path) = resolve_run_alias(config, run)? {
        return Ok(alias_path);
    }

    Ok(config.runs_dir().join(run))
}

pub(crate) fn trusted_explicit_trace_artifacts_dir(
    trace_path: &Path,
) -> FozzyResult<Option<PathBuf>> {
    Ok(crate::trusted_artifact_bundle_for_trace(trace_path)?.map(|bundle| bundle.artifacts_dir))
}

pub(crate) fn has_untrusted_sibling_artifacts(trace_path: &Path) -> FozzyResult<bool> {
    let Some(parent) = trace_path.parent() else {
        return Ok(false);
    };
    let explicit_trace =
        std::fs::canonicalize(trace_path).unwrap_or_else(|_| trace_path.to_path_buf());
    let has_artifactish_siblings = std::fs::read_dir(parent)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path == *trace_path {
                return None;
            }
            if let Ok(canonical) = std::fs::canonicalize(&path)
                && canonical == explicit_trace
            {
                return None;
            }
            let name = path.file_name()?.to_string_lossy().to_string();
            let is_artifact = crate::artifact_file_specs()
                .iter()
                .any(|(candidate, _)| *candidate == name);
            is_artifact.then_some(path)
        })
        .next()
        .is_some();
    if !has_artifactish_siblings {
        return Ok(false);
    }

    Ok(
        load_checked_run_summary_from_artifacts_dir(parent, &trace_path.display().to_string())?
            .is_none(),
    )
}

pub(crate) fn resolve_trace_path_from_artifacts_dir(
    artifacts_dir: &Path,
) -> FozzyResult<Option<PathBuf>> {
    let local_trace = artifacts_dir.join("trace.fozzy");
    let local_trace = local_trace.exists().then_some(local_trace);
    let report_path = artifacts_dir.join("report.json");
    let report_trace = if report_path.exists()
        && let Ok(summary) = serde_json::from_slice::<RunSummary>(&std::fs::read(&report_path)?)
        && let Some(path) = summary.identity.trace_path
    {
        let trace_path = PathBuf::from(path);
        trace_path.exists().then_some(trace_path)
    } else {
        None
    };

    let manifest_path = artifacts_dir.join("manifest.json");
    let manifest_trace = if manifest_path.exists()
        && let Ok(manifest) = serde_json::from_slice::<RunManifest>(&std::fs::read(&manifest_path)?)
        && let Some(path) = manifest.trace_path
    {
        let trace_path = PathBuf::from(path);
        trace_path.exists().then_some(trace_path)
    } else {
        None
    };

    if let (Some(report_trace), Some(manifest_trace)) = (&report_trace, &manifest_trace)
        && trace_identity_key(report_trace)? != trace_identity_key(manifest_trace)?
    {
        return Err(FozzyError::InvalidArgument(format!(
            "conflicting declared trace identities in {}: report.json={}, manifest.json={}",
            artifacts_dir.display(),
            report_trace.display(),
            manifest_trace.display()
        )));
    }

    let declared_trace = report_trace.or(manifest_trace);
    if let (Some(local_trace), Some(declared_trace)) = (&local_trace, &declared_trace)
        && trace_identity_key(local_trace)? != trace_identity_key(declared_trace)?
    {
        return Err(FozzyError::InvalidArgument(format!(
            "conflicting local and declared trace identities in {}: local={}, declared={}",
            artifacts_dir.display(),
            local_trace.display(),
            declared_trace.display()
        )));
    }

    Ok(declared_trace.or(local_trace))
}

pub(crate) fn load_checked_report_summary_from_artifacts_dir(
    artifacts_dir: &Path,
    run: &str,
) -> FozzyResult<Option<RunSummary>> {
    let report_path = artifacts_dir.join("report.json");
    if !report_path.exists() {
        return Ok(None);
    }

    let mut files = vec![report_path.clone()];
    let manifest_path = artifacts_dir.join("manifest.json");
    if manifest_path.exists() {
        files.push(manifest_path);
    }
    if let Some(trace_path) = resolve_trace_path_from_artifacts_dir(artifacts_dir)? {
        files.push(trace_path);
    }
    crate::validate_manifest_integrity(&files, run)?;

    Ok(Some(serde_json::from_slice(&std::fs::read(report_path)?)?))
}

pub(crate) fn load_checked_manifest_trace_summary_from_artifacts_dir(
    artifacts_dir: &Path,
    run: &str,
) -> FozzyResult<Option<RunSummary>> {
    let manifest_path = artifacts_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Ok(None);
    }
    let Some(trace_path) = resolve_trace_path_from_artifacts_dir(artifacts_dir)? else {
        return Err(FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: missing declared trace artifact"
        )));
    };
    let manifest: RunManifest =
        serde_json::from_slice(&std::fs::read(&manifest_path)?).map_err(|e| {
            FozzyError::InvalidArgument(format!(
                "invalid manifest for {run:?}: {} ({e})",
                manifest_path.display()
            ))
        })?;
    if manifest.schema_version != "fozzy.run_manifest.v1" {
        return Err(FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: unsupported schemaVersion {}",
            manifest.schema_version
        )));
    }
    let trace: TraceFile = serde_json::from_slice(&std::fs::read(&trace_path)?).map_err(|e| {
        FozzyError::InvalidArgument(format!(
            "invalid trace for {run:?}: {} ({e})",
            trace_path.display()
        ))
    })?;
    let expected_trace = trace_path.to_string_lossy().to_string();
    let expected_artifacts_dir = artifacts_dir.to_string_lossy().to_string();
    if manifest.trace_path.as_deref() != Some(expected_trace.as_str())
        || trace.summary.identity.trace_path.as_deref() != Some(expected_trace.as_str())
    {
        return Err(FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: declared trace artifact mismatch"
        )));
    }
    if manifest.mode == crate::RunMode::Replay {
        if trace.summary.status != manifest.status || trace.summary.identity.seed != manifest.seed {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid manifest for {run:?}: replay source trace mismatch"
            )));
        }
        return Ok(Some(trace.summary));
    }
    if trace.summary.identity.run_id != manifest.run_id
        || trace.summary.mode != manifest.mode
        || trace.summary.status != manifest.status
        || trace.summary.identity.seed != manifest.seed
        || trace.summary.identity.report_path != manifest.report_path
        || trace.summary.identity.artifacts_dir.as_deref() != Some(expected_artifacts_dir.as_str())
        || manifest.artifacts_dir.as_deref() != Some(expected_artifacts_dir.as_str())
    {
        return Err(FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: manifest/trace identity mismatch"
        )));
    }
    Ok(Some(trace.summary))
}

pub(crate) fn load_checked_run_summary_from_artifacts_dir(
    artifacts_dir: &Path,
    run: &str,
) -> FozzyResult<Option<RunSummary>> {
    if let Some(summary) = load_checked_report_summary_from_artifacts_dir(artifacts_dir, run)? {
        return Ok(Some(summary));
    }
    load_checked_manifest_trace_summary_from_artifacts_dir(artifacts_dir, run)
}

fn trusted_trace_declared_artifacts_dir(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    Ok(
        crate::trusted_declared_artifact_bundle_for_trace(trace_path)?
            .map(|bundle| bundle.artifacts_dir),
    )
}

fn trace_identity_key(path: &Path) -> FozzyResult<PathBuf> {
    std::fs::canonicalize(path).map_err(Into::into)
}

fn resolve_run_alias(config: &Config, run: &str) -> FozzyResult<Option<PathBuf>> {
    let key = run.trim().to_ascii_lowercase();
    if key != "latest" && key != "last-pass" && key != "last-fail" {
        return Ok(None);
    }

    let runs_dir = config.runs_dir();
    if !runs_dir.exists() {
        return Err(FozzyError::InvalidArgument(format!(
            "run alias {run:?} cannot be resolved: runs directory not found ({})",
            runs_dir.display()
        )));
    }

    let mut run_dirs = std::fs::read_dir(&runs_dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if !entry.file_type().ok()?.is_dir() {
                return None;
            }
            let md = entry.metadata().ok()?;
            let modified = md.modified().ok()?;
            Some((entry.path(), modified))
        })
        .collect::<Vec<_>>();
    run_dirs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if run_dirs.is_empty() {
        return Err(FozzyError::InvalidArgument(format!(
            "run alias {run:?} cannot be resolved: no coherent completed runs found"
        )));
    }

    for (dir, _) in run_dirs {
        let summary =
            match load_checked_run_summary_from_artifacts_dir(&dir, &dir.display().to_string()) {
                Ok(Some(summary)) => summary,
                Ok(None) | Err(_) => continue,
            };
        if key == "latest" {
            return Ok(Some(dir));
        }
        if (key == "last-pass" && summary.status == ExitStatus::Pass)
            || (key == "last-fail" && summary.status != ExitStatus::Pass)
        {
            return Ok(Some(dir));
        }
    }
    let reason = if key == "latest" {
        "no coherent completed runs found"
    } else {
        "no matching coherent run found"
    };
    Err(FozzyError::InvalidArgument(format!(
        "run alias {run:?} cannot be resolved: {reason}"
    )))
}
