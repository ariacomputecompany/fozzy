use std::path::{Path, PathBuf};

use crate::{Config, FozzyError, FozzyResult};
use crate::{validate_direct_trace_bundle, validate_run_bundle_integrity};

use super::{ArtifactEntry, ArtifactKind};

pub(super) fn artifacts_list(config: &Config, run: &str) -> FozzyResult<Vec<ArtifactEntry>> {
    let run_path = PathBuf::from(crate::normalize_run_or_trace_selector(run));
    if run_path.exists() && run_path.is_file() && crate::is_trace_path(&run_path) {
        let mut out = Vec::new();
        let mut files = vec![run_path.clone()];
        push_if_exists(&mut out, ArtifactKind::Trace, run_path.clone())?;
        let trusted_artifacts_dir = crate::trusted_explicit_trace_artifacts_dir(&run_path)?;
        let allow_sidecars_without_metadata = trusted_artifacts_dir.is_some();
        if let Some(artifacts_dir) = trusted_artifacts_dir {
            for (kind, path) in crate::artifact_file_entries(&artifacts_dir) {
                if path.exists() && path.is_file() {
                    files.push(path.clone());
                }
                push_if_exists(&mut out, kind, path)?;
            }
        } else if crate::has_untrusted_sibling_artifacts(&run_path)? {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "untrusted sibling artifacts for direct trace {run:?}; report.json and manifest.json are required to trust sibling files"
            )));
        }
        validate_direct_trace_bundle(&files, run, allow_sidecars_without_metadata)?;
        return Ok(out);
    }

    if run_path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("fozzy"))
        && !run_path.exists()
    {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "trace path not found: {}",
            run_path.display()
        )));
    }

    let artifacts_dir = crate::resolve_artifacts_dir(config, run)?;
    if !artifacts_dir.exists() {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "run artifacts not found: {}",
            artifacts_dir.display()
        )));
    }
    validate_run_artifacts_for_listing(&artifacts_dir, run)?;
    let mut out = Vec::new();

    if let Some(trace_path) = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)? {
        push_if_exists(&mut out, ArtifactKind::Trace, trace_path)?;
    }
    for (kind, path) in crate::artifact_file_entries(&artifacts_dir) {
        push_if_exists(&mut out, kind, path)?;
    }

    Ok(out)
}

pub(super) fn validate_run_artifacts_for_listing(
    artifacts_dir: &Path,
    run: &str,
) -> FozzyResult<()> {
    let report = artifacts_dir.join("report.json");
    let manifest = artifacts_dir.join("manifest.json");
    let trace = crate::resolve_trace_path_from_artifacts_dir(artifacts_dir)?;
    if !report.exists() && !manifest.exists() {
        if trace.is_some() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "incomplete artifacts for {run:?}; missing required files: report.json, manifest.json"
            )));
        }
        return Ok(());
    }

    let mut files = Vec::new();
    for path in crate::artifact_file_paths(artifacts_dir) {
        if path.exists() {
            files.push(path);
        }
    }
    if let Some(trace) = trace {
        files.push(trace);
    }
    validate_run_bundle_integrity(&files, run)
}

pub(super) fn resolve_trace_path(config: &Config, run: &str) -> FozzyResult<PathBuf> {
    let input = crate::normalize_trace_path(&PathBuf::from(run));
    if input.exists()
        && input.is_file()
        && input
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("fozzy"))
    {
        return Ok(input);
    }
    let artifacts_dir = crate::resolve_artifacts_dir(config, run)?;
    let Some(trace) = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)? else {
        return Err(FozzyError::InvalidArgument(format!(
            "no recorded trace found for {run:?}; expected {}",
            artifacts_dir.join("trace.fozzy").display()
        )));
    };
    Ok(trace)
}

pub(super) fn is_direct_trace_input(run: &str) -> bool {
    let p = PathBuf::from(crate::normalize_run_or_trace_selector(run));
    p.exists() && p.is_file() && crate::is_trace_path(&p)
}

pub(super) fn push_if_exists(
    out: &mut Vec<ArtifactEntry>,
    kind: ArtifactKind,
    path: PathBuf,
) -> FozzyResult<()> {
    if !path.exists() {
        return Ok(());
    }
    let md = std::fs::metadata(&path)?;
    out.push(ArtifactEntry {
        kind,
        path: path.to_string_lossy().to_string(),
        size_bytes: Some(md.len()),
    });
    Ok(())
}
