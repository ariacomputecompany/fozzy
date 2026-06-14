use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::{Config, ExitStatus, FozzyError, FozzyResult, RunManifest, RunSummary, TraceFile};

pub(crate) fn resolve_artifacts_dir(config: &Config, run: &str) -> FozzyResult<PathBuf> {
    let path = PathBuf::from(crate::normalize_run_or_trace_selector(run));
    if path.exists() {
        if path.is_dir() {
            return Ok(path);
        }

        if path.is_file() && crate::is_trace_path(&path) {
            if let Some(artifacts_dir) = trusted_trace_declared_artifacts_dir(&path)? {
                return Ok(artifacts_dir);
            }
            if let Some(parent) = path.parent() {
                return Ok(parent.to_path_buf());
            }
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
        && let Ok(summary) = crate::read_cached_run_summary(&report_path)
        && let Some(path) = summary.identity.trace_path
    {
        let trace_path = PathBuf::from(path);
        trace_path.exists().then_some(trace_path)
    } else {
        None
    };

    let manifest_path = artifacts_dir.join("manifest.json");
    let manifest_trace = if manifest_path.exists()
        && let Ok(manifest) = crate::read_cached_run_manifest(&manifest_path)
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

    Ok(Some(crate::read_cached_run_summary(&report_path)?))
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
        crate::read_cached_run_manifest(&manifest_path).map_err(|e| match e {
            crate::FozzyError::Json(_) | crate::FozzyError::Trace(_) => {
                FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: {} ({e})",
                    manifest_path.display()
                ))
            }
            other => other,
        })?;
    if manifest.schema_version != "fozzy.run_manifest.v1" {
        return Err(FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: unsupported schemaVersion {}",
            manifest.schema_version
        )));
    }
    let trace: TraceFile = crate::read_cached_trace_file(&trace_path).map_err(|e| {
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

#[derive(Debug, Clone)]
struct RunAliasEntry {
    dir: PathBuf,
    summary: RunSummary,
    directory_modified_ns: u128,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RunAliasIndex {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    entries: Vec<RunAliasIndexEntryRecord>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RunAliasIndexEntryRecord {
    #[serde(rename = "runId")]
    run_id: String,
    status: ExitStatus,
    #[serde(rename = "finishedAt")]
    finished_at: String,
    #[serde(rename = "artifactsDir")]
    artifacts_dir: String,
}

fn run_index_cache()
-> &'static Mutex<std::collections::HashMap<PathBuf, (crate::FileFingerprint, Vec<RunAliasEntry>)>>
{
    static CACHE: OnceLock<
        Mutex<std::collections::HashMap<PathBuf, (crate::FileFingerprint, Vec<RunAliasEntry>)>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn load_run_index(runs_dir: &Path) -> FozzyResult<Vec<RunAliasEntry>> {
    let fingerprint = crate::FileFingerprint::for_path(runs_dir)?;
    if let Some((cached_fingerprint, cached)) = run_index_cache()
        .lock()
        .expect("run index cache poisoned")
        .get(runs_dir)
        .cloned()
        .filter(|(cached_fingerprint, _)| *cached_fingerprint == fingerprint)
    {
        let _ = cached_fingerprint;
        return Ok(cached);
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(runs_dir)?.filter_map(Result::ok) {
        if !entry
            .file_type()
            .ok()
            .is_some_and(|file_type| file_type.is_dir())
        {
            continue;
        }
        let dir = entry.path();
        let selector = dir.display().to_string();
        let summary = match load_checked_run_summary_from_artifacts_dir(&dir, &selector) {
            Ok(Some(summary)) => summary,
            Ok(None) | Err(_) => continue,
        };
        entries.push(RunAliasEntry {
            dir,
            summary,
            directory_modified_ns: crate::FileFingerprint::for_path(&entry.path())?.modified_ns,
        });
    }
    entries.sort_by(|a, b| {
        b.summary
            .finished_at
            .cmp(&a.summary.finished_at)
            .then_with(|| b.directory_modified_ns.cmp(&a.directory_modified_ns))
            .then_with(|| b.summary.identity.run_id.cmp(&a.summary.identity.run_id))
            .then_with(|| a.dir.cmp(&b.dir))
    });
    let _ = write_run_alias_index_entries(runs_dir, &entries);
    run_index_cache()
        .lock()
        .expect("run index cache poisoned")
        .insert(runs_dir.to_path_buf(), (fingerprint, entries.clone()));
    Ok(entries)
}

pub(crate) fn resolve_filtered_run_alias(
    config: &Config,
    run: &str,
    accept: impl Fn(&Path, &RunSummary) -> bool,
) -> FozzyResult<Option<PathBuf>> {
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

    let run_index = if let Some(index) = read_run_alias_index(config)? {
        index
    } else {
        load_run_index(&runs_dir)?
    };
    if run_index.is_empty() {
        return Err(FozzyError::InvalidArgument(format!(
            "run alias {run:?} cannot be resolved: no coherent completed runs found"
        )));
    }

    for entry in run_index {
        if !accept(&entry.dir, &entry.summary) {
            continue;
        }
        if key == "latest" {
            return Ok(Some(entry.dir));
        }
        if (key == "last-pass" && entry.summary.status == ExitStatus::Pass)
            || (key == "last-fail" && entry.summary.status != ExitStatus::Pass)
        {
            return Ok(Some(entry.dir));
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

fn resolve_run_alias(config: &Config, run: &str) -> FozzyResult<Option<PathBuf>> {
    resolve_filtered_run_alias(config, run, |_dir, _summary| true)
}

pub(crate) fn update_run_alias_index(
    summary: &RunSummary,
    artifacts_dir: &Path,
) -> FozzyResult<()> {
    let Some(runs_dir) = artifacts_dir.parent() else {
        return Ok(());
    };
    let index_path = runs_dir
        .parent()
        .unwrap_or(runs_dir)
        .join("run-alias-index.json");
    let mut index = read_run_alias_index_file(&index_path).unwrap_or(RunAliasIndex {
        schema_version: "fozzy.run_alias_index.v1".to_string(),
        entries: Vec::new(),
    });
    let record = RunAliasIndexEntryRecord {
        run_id: summary.identity.run_id.clone(),
        status: summary.status,
        finished_at: summary.finished_at.clone(),
        artifacts_dir: artifacts_dir.to_string_lossy().to_string(),
    };
    index.entries.retain(|entry| entry.run_id != record.run_id);
    index.entries.push(record);
    index.entries.sort_by(|a, b| {
        b.finished_at
            .cmp(&a.finished_at)
            .then_with(|| b.run_id.cmp(&a.run_id))
            .then_with(|| a.artifacts_dir.cmp(&b.artifacts_dir))
    });
    index
        .entries
        .retain(|entry| Path::new(&entry.artifacts_dir).exists());
    write_run_alias_index_file(&index_path, &index)
}

fn read_run_alias_index(config: &Config) -> FozzyResult<Option<Vec<RunAliasEntry>>> {
    let index = match read_run_alias_index_file(&config.run_alias_index_path()) {
        Ok(index) => index,
        Err(_) => return Ok(None),
    };
    let mut entries = Vec::new();
    for record in index.entries {
        let dir = PathBuf::from(&record.artifacts_dir);
        if !dir.exists() {
            continue;
        }
        let selector = dir.display().to_string();
        let summary = match load_checked_run_summary_from_artifacts_dir(&dir, &selector) {
            Ok(Some(summary)) => summary,
            Ok(None) | Err(_) => continue,
        };
        let directory_modified_ns = crate::FileFingerprint::for_path(&dir)?.modified_ns;
        entries.push(RunAliasEntry {
            dir,
            summary,
            directory_modified_ns,
        });
    }
    Ok(Some(entries))
}

fn write_run_alias_index_entries(runs_dir: &Path, entries: &[RunAliasEntry]) -> FozzyResult<()> {
    let index_path = runs_dir
        .parent()
        .unwrap_or(runs_dir)
        .join("run-alias-index.json");
    let index = RunAliasIndex {
        schema_version: "fozzy.run_alias_index.v1".to_string(),
        entries: entries
            .iter()
            .map(|entry| RunAliasIndexEntryRecord {
                run_id: entry.summary.identity.run_id.clone(),
                status: entry.summary.status,
                finished_at: entry.summary.finished_at.clone(),
                artifacts_dir: entry.dir.to_string_lossy().to_string(),
            })
            .collect(),
    };
    write_run_alias_index_file(&index_path, &index)
}

fn read_run_alias_index_file(path: &Path) -> FozzyResult<RunAliasIndex> {
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn write_run_alias_index_file(path: &Path, index: &RunAliasIndex) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(index)?)?;
    Ok(())
}
