use std::path::{Path, PathBuf};

use crate::{Config, FozzyResult, RunSummary, TraceFile};

#[derive(Debug, Clone)]
pub(crate) struct ValidatedArtifactBundle {
    pub artifacts_dir: PathBuf,
    pub summary: RunSummary,
    pub trace_path: Option<PathBuf>,
}

pub(crate) fn load_validated_artifact_bundle(
    config: &Config,
    selector: &str,
) -> FozzyResult<Option<ValidatedArtifactBundle>> {
    let artifacts_dir = crate::resolve_artifacts_dir(config, selector)?;
    load_validated_artifact_bundle_from_dir(&artifacts_dir, selector)
}

pub(crate) fn load_validated_artifact_bundle_from_dir(
    artifacts_dir: &Path,
    selector: &str,
) -> FozzyResult<Option<ValidatedArtifactBundle>> {
    let summary = if let Some(summary) =
        crate::load_checked_report_summary_from_artifacts_dir(artifacts_dir, selector)?
    {
        summary
    } else if let Some(summary) =
        crate::load_checked_manifest_trace_summary_from_artifacts_dir(artifacts_dir, selector)?
    {
        summary
    } else {
        return Ok(None);
    };
    let trace_path = crate::resolve_trace_path_from_artifacts_dir(artifacts_dir)?;
    Ok(Some(ValidatedArtifactBundle {
        artifacts_dir: artifacts_dir.to_path_buf(),
        summary,
        trace_path,
    }))
}

pub(crate) fn trusted_declared_artifact_bundle_for_trace(
    trace_path: &Path,
) -> FozzyResult<Option<ValidatedArtifactBundle>> {
    let Some(artifacts_dir) = declared_artifacts_dir_for_trace(trace_path)? else {
        return Ok(None);
    };
    trusted_bundle_for_trace_in_dir(trace_path, &artifacts_dir)
}

pub(crate) fn trusted_artifact_bundle_for_trace(
    trace_path: &Path,
) -> FozzyResult<Option<ValidatedArtifactBundle>> {
    if let Some(bundle) = trusted_declared_artifact_bundle_for_trace(trace_path)? {
        return Ok(Some(bundle));
    }

    let Some(parent) = trace_path.parent() else {
        return Ok(None);
    };
    if parent == trace_path {
        return Ok(None);
    }
    trusted_bundle_for_trace_in_dir(trace_path, parent)
}

fn trusted_bundle_for_trace_in_dir(
    trace_path: &Path,
    artifacts_dir: &Path,
) -> FozzyResult<Option<ValidatedArtifactBundle>> {
    let trace = TraceFile::read_json(trace_path)?;
    let selector = trace_path.display().to_string();
    let Some(bundle) = load_validated_artifact_bundle_from_dir(artifacts_dir, &selector)? else {
        return Ok(None);
    };
    let Some(resolved_trace) = bundle.trace_path.as_ref() else {
        return Ok(None);
    };
    let expected_trace =
        std::fs::canonicalize(trace_path).unwrap_or_else(|_| trace_path.to_path_buf());
    let actual_trace =
        std::fs::canonicalize(resolved_trace).unwrap_or_else(|_| resolved_trace.clone());
    if actual_trace != expected_trace {
        return Ok(None);
    }
    if bundle.summary.identity.run_id != trace.summary.identity.run_id
        || bundle.summary.identity.seed != trace.summary.identity.seed
    {
        return Ok(None);
    }
    Ok(Some(bundle))
}

pub(crate) fn declared_artifacts_dir_for_trace(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    let trace = TraceFile::read_json(trace_path)?;
    Ok(trace
        .summary
        .identity
        .artifacts_dir
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| path.exists() && path.is_dir()))
}

pub(crate) fn trusted_sidecar_path_for_trace(
    trace_path: &Path,
    file_name: &str,
) -> FozzyResult<Option<PathBuf>> {
    let Some(bundle) = trusted_artifact_bundle_for_trace(trace_path)? else {
        return Ok(None);
    };
    let sidecar_path = bundle.artifacts_dir.join(file_name);
    if sidecar_path.exists() {
        Ok(Some(sidecar_path))
    } else {
        Ok(None)
    }
}
