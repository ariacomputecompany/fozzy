//! Artifact management (`fozzy artifacts ...`).

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};

use crate::{Config, FozzyError, FozzyResult, RunSummary, TraceFile};
#[cfg(test)]
use crate::{
    load_checked_report_summary_from_artifacts_dir, resolve_artifacts_dir,
    resolve_trace_path_from_artifacts_dir,
};
use crate::{validate_direct_trace_bundle, validate_run_bundle_integrity};

#[derive(Debug, Subcommand)]
pub enum ArtifactCommand {
    Ls {
        #[arg(value_name = "RUN_OR_TRACE")]
        run: String,
    },
    Diff {
        #[arg(value_name = "LEFT_RUN_OR_TRACE")]
        left: String,
        #[arg(value_name = "RIGHT_RUN_OR_TRACE")]
        right: String,
    },
    Export {
        #[arg(value_name = "RUN_OR_TRACE")]
        run: String,
        #[arg(long)]
        out: PathBuf,
    },
    Pack {
        #[arg(value_name = "RUN_OR_TRACE")]
        run: String,
        #[arg(long)]
        out: PathBuf,
    },
    Bundle {
        #[arg(value_name = "RUN_OR_TRACE")]
        run: String,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Trace,
    Timeline,
    Profile,
    Memory,
    Events,
    Report,
    Manifest,
    Coverage,
    MinRepro,
    Logs,
    Corpus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactEntry {
    pub kind: ArtifactKind,
    pub path: String,
    #[serde(rename = "sizeBytes", skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ArtifactOutput {
    List { entries: Vec<ArtifactEntry> },
    Diff { diff: Box<ArtifactDiff> },
    Exported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactDiff {
    pub left: String,
    pub right: String,
    pub files: Vec<ArtifactFileDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<ReportDelta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<TraceDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactFileDelta {
    pub key: String,
    #[serde(rename = "leftPath", skip_serializing_if = "Option::is_none")]
    pub left_path: Option<String>,
    #[serde(rename = "rightPath", skip_serializing_if = "Option::is_none")]
    pub right_path: Option<String>,
    #[serde(rename = "leftSizeBytes", skip_serializing_if = "Option::is_none")]
    pub left_size_bytes: Option<u64>,
    #[serde(rename = "rightSizeBytes", skip_serializing_if = "Option::is_none")]
    pub right_size_bytes: Option<u64>,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDelta {
    #[serde(rename = "leftStatus")]
    pub left_status: String,
    #[serde(rename = "rightStatus")]
    pub right_status: String,
    #[serde(rename = "leftMode")]
    pub left_mode: String,
    #[serde(rename = "rightMode")]
    pub right_mode: String,
    #[serde(rename = "leftFindings")]
    pub left_findings: usize,
    #[serde(rename = "rightFindings")]
    pub right_findings: usize,
    #[serde(rename = "leftDurationMs")]
    pub left_duration_ms: u64,
    #[serde(rename = "rightDurationMs")]
    pub right_duration_ms: u64,
    #[serde(rename = "findingTitlesChanged")]
    pub finding_titles_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceDelta {
    #[serde(rename = "leftMode")]
    pub left_mode: String,
    #[serde(rename = "rightMode")]
    pub right_mode: String,
    #[serde(rename = "leftDecisions")]
    pub left_decisions: usize,
    #[serde(rename = "rightDecisions")]
    pub right_decisions: usize,
    #[serde(rename = "leftEvents")]
    pub left_events: usize,
    #[serde(rename = "rightEvents")]
    pub right_events: usize,
    #[serde(
        rename = "firstDecisionDiffIndex",
        skip_serializing_if = "Option::is_none"
    )]
    pub first_decision_diff_index: Option<usize>,
    #[serde(
        rename = "firstEventDiffIndex",
        skip_serializing_if = "Option::is_none"
    )]
    pub first_event_diff_index: Option<usize>,
}

pub fn artifacts_command(
    config: &Config,
    command: &ArtifactCommand,
) -> FozzyResult<ArtifactOutput> {
    match command {
        ArtifactCommand::Ls { run } => Ok(ArtifactOutput::List {
            entries: artifacts_list(config, run)?,
        }),
        ArtifactCommand::Diff { left, right } => Ok(ArtifactOutput::Diff {
            diff: Box::new(artifacts_diff(config, left, right)?),
        }),
        ArtifactCommand::Export { run, out } => {
            export_artifacts(config, run, out)?;
            Ok(ArtifactOutput::Exported)
        }
        ArtifactCommand::Pack { run, out } => {
            export_reproducer_pack(config, run, out)?;
            Ok(ArtifactOutput::Exported)
        }
        ArtifactCommand::Bundle { run, out } => {
            export_gate_bundle(config, run, out)?;
            Ok(ArtifactOutput::Exported)
        }
    }
}

fn artifacts_list(config: &Config, run: &str) -> FozzyResult<Vec<ArtifactEntry>> {
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

fn validate_run_artifacts_for_listing(artifacts_dir: &Path, run: &str) -> FozzyResult<()> {
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

fn export_reproducer_pack(config: &Config, run: &str, out: &Path) -> FozzyResult<()> {
    let strict_bundle = !is_direct_trace_input(run);
    let entries = artifacts_list(config, run)?;
    let mut files: Vec<PathBuf> = entries
        .into_iter()
        .map(|e| PathBuf::from(e.path))
        .filter(|p| p.exists() && p.is_file())
        .collect();
    files.sort();
    files.dedup();

    if files.is_empty() {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "no artifacts found for {run:?}"
        )));
    }
    if strict_bundle {
        validate_run_bundle_integrity(&files, run)?;
    } else {
        let trace_path = crate::normalize_trace_path(&PathBuf::from(run));
        let trusted_artifacts_dir = crate::trusted_explicit_trace_artifacts_dir(&trace_path)?;
        validate_direct_trace_bundle(&files, run, trusted_artifacts_dir.is_some())?;
    }

    let meta_dir = std::env::temp_dir().join(format!("fozzy-pack-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&meta_dir)?;
    let meta_files = vec![
        (
            "env.json",
            serde_json::to_vec_pretty(&crate::env_info(config))?,
        ),
        (
            "version.json",
            serde_json::to_vec_pretty(&crate::version_info())?,
        ),
        (
            "commandline.json",
            serde_json::to_vec_pretty(&serde_json::json!({
                "command": "fozzy artifacts pack",
                "target": run,
            }))?,
        ),
    ];
    for (name, bytes) in meta_files {
        std::fs::write(meta_dir.join(name), bytes)?;
    }
    files.push(meta_dir.join("env.json"));
    files.push(meta_dir.join("version.json"));
    files.push(meta_dir.join("commandline.json"));

    let res = if out
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("zip"))
    {
        export_artifacts_zip(&files, out)
    } else {
        export_artifacts_dir_exact(&files, out)
    };
    let _ = std::fs::remove_dir_all(meta_dir);
    res
}

fn export_gate_bundle(config: &Config, run: &str, out: &Path) -> FozzyResult<()> {
    let direct_trace_input = is_direct_trace_input(run);
    let trace_path = resolve_trace_path(config, run)?;
    let trace_input = trace_path.to_string_lossy().to_string();
    let mut source_files: Vec<PathBuf> = vec![trace_path.clone()];
    if direct_trace_input {
        let trusted_artifacts_dir = crate::trusted_explicit_trace_artifacts_dir(&trace_path)?;
        if let Some(artifacts_dir) = trusted_artifacts_dir.clone() {
            for name in ["report.json", "manifest.json"] {
                let path = artifacts_dir.join(name);
                if path.exists() && path.is_file() {
                    source_files.push(path);
                }
            }
        }
        validate_direct_trace_bundle(&source_files, run, trusted_artifacts_dir.is_some())?;
    } else {
        let artifacts_dir = crate::resolve_artifacts_dir(config, run)?;
        for name in ["report.json", "manifest.json"] {
            let path = artifacts_dir.join(name);
            if path.exists() && path.is_file() {
                source_files.push(path);
            }
        }
        source_files.sort();
        source_files.dedup();
        validate_run_bundle_integrity(&source_files, run)?;
    }
    source_files.sort();
    source_files.dedup();

    let replay = crate::replay_trace(
        config,
        crate::TracePath::new(trace_path.clone()),
        &crate::ReplayOptions {
            step: false,
            until: None,
            dump_events: false,
            profile_capture: crate::ProfileCaptureLevel::Baseline,
            reporter: crate::Reporter::Json,
        },
    )?;
    let ci = crate::ci_evaluate(
        config,
        &crate::CiOptions {
            trace: trace_path.clone(),
            flake_runs: Vec::new(),
            flake_budget_pct: None,
            perf_baseline: None,
            max_p99_delta_pct: None,
            strict: true,
        },
    )?;
    let verify = crate::verify_trace_file(&trace_path)?;

    let mut files = source_files;

    let meta_dir = std::env::temp_dir().join(format!("fozzy-bundle-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&meta_dir)?;
    let meta_files = vec![
        (
            "env.json",
            serde_json::to_vec_pretty(&crate::env_info(config))?,
        ),
        (
            "version.json",
            serde_json::to_vec_pretty(&crate::version_info())?,
        ),
        (
            "trace_verify.report.json",
            serde_json::to_vec_pretty(&verify)?,
        ),
        (
            "replay.report.json",
            serde_json::to_vec_pretty(&replay.summary)?,
        ),
        ("ci.report.json", serde_json::to_vec_pretty(&ci)?),
        (
            "bundle.json",
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "fozzy.bundle_report.v1",
                "source": run,
                "trace": trace_input,
                "ciOk": ci.ok
            }))?,
        ),
    ];
    for (name, bytes) in meta_files {
        std::fs::write(meta_dir.join(name), bytes)?;
    }
    for name in [
        "env.json",
        "version.json",
        "trace_verify.report.json",
        "replay.report.json",
        "ci.report.json",
        "bundle.json",
    ] {
        files.push(meta_dir.join(name));
    }
    files.sort();
    files.dedup();
    let res = if out
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("zip"))
    {
        export_artifacts_zip(&files, out)
    } else {
        export_artifacts_dir_exact(&files, out)
    };
    let _ = std::fs::remove_dir_all(meta_dir);
    res
}

fn export_artifacts(config: &Config, run: &str, out: &Path) -> FozzyResult<()> {
    let strict_bundle = !is_direct_trace_input(run);
    let entries = artifacts_list(config, run)?;
    let mut files: Vec<PathBuf> = entries
        .into_iter()
        .map(|e| PathBuf::from(e.path))
        .filter(|p| p.exists() && p.is_file())
        .collect();
    files.sort();
    files.dedup();

    if files.is_empty() {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "no artifacts found for {run:?}"
        )));
    }
    if strict_bundle {
        validate_run_bundle_integrity(&files, run)?;
    } else {
        let trace_path = crate::normalize_trace_path(&PathBuf::from(run));
        let trusted_artifacts_dir = crate::trusted_explicit_trace_artifacts_dir(&trace_path)?;
        validate_direct_trace_bundle(&files, run, trusted_artifacts_dir.is_some())?;
    }

    if out
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("zip"))
    {
        export_artifacts_zip(&files, out)?;
        return Ok(());
    }

    export_artifacts_dir_exact(&files, out)
}

fn resolve_trace_path(config: &Config, run: &str) -> FozzyResult<PathBuf> {
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

fn artifacts_diff(config: &Config, left: &str, right: &str) -> FozzyResult<ArtifactDiff> {
    let left_entries = artifacts_list(config, left)?;
    let right_entries = artifacts_list(config, right)?;

    let mut left_map = BTreeMap::new();
    for entry in left_entries {
        let file = PathBuf::from(&entry.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&entry.path)
            .to_string();
        let key = format!("{:?}:{file}", entry.kind);
        left_map.insert(key, entry);
    }
    let mut right_map = BTreeMap::new();
    for entry in right_entries {
        let file = PathBuf::from(&entry.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&entry.path)
            .to_string();
        let key = format!("{:?}:{file}", entry.kind);
        right_map.insert(key, entry);
    }

    let mut keys: Vec<String> = left_map.keys().chain(right_map.keys()).cloned().collect();
    keys.sort();
    keys.dedup();

    let mut files = Vec::new();
    for key in keys {
        let l = left_map.get(&key);
        let r = right_map.get(&key);
        let left_path = l.map(|e| e.path.clone());
        let right_path = r.map(|e| e.path.clone());
        let left_size = l.and_then(|e| e.size_bytes);
        let right_size = r.and_then(|e| e.size_bytes);
        let changed = file_delta_changed(left_path.as_deref(), right_path.as_deref())?;
        files.push(ArtifactFileDelta {
            key,
            left_path,
            right_path,
            left_size_bytes: left_size,
            right_size_bytes: right_size,
            changed,
        });
    }

    let report = match (load_summary(config, left)?, load_summary(config, right)?) {
        (Some(l), Some(r)) => Some(report_delta(&l, &r)),
        _ => None,
    };
    let trace = match (load_trace(config, left)?, load_trace(config, right)?) {
        (Some(l), Some(r)) => Some(trace_delta(&l, &r)),
        _ => None,
    };

    Ok(ArtifactDiff {
        left: left.to_string(),
        right: right.to_string(),
        files,
        report,
        trace,
    })
}

fn file_delta_changed(left: Option<&str>, right: Option<&str>) -> FozzyResult<bool> {
    let Some(left) = left else {
        return Ok(right.is_some());
    };
    let Some(right) = right else {
        return Ok(true);
    };
    let left = Path::new(left);
    let right = Path::new(right);
    if !left.exists() || !right.exists() {
        return Ok(left.exists() != right.exists());
    }
    let left_md = std::fs::metadata(left)?;
    let right_md = std::fs::metadata(right)?;
    if left_md.len() != right_md.len() {
        return Ok(true);
    }
    Ok(!files_equal(left, right)?)
}

fn files_equal(left: &Path, right: &Path) -> FozzyResult<bool> {
    let mut left_file = std::fs::File::open(left)?;
    let mut right_file = std::fs::File::open(right)?;
    let mut left_buf = [0u8; 64 * 1024];
    let mut right_buf = [0u8; 64 * 1024];
    loop {
        let left_n = left_file.read(&mut left_buf)?;
        let right_n = right_file.read(&mut right_buf)?;
        if left_n != right_n {
            return Ok(false);
        }
        if left_n == 0 {
            return Ok(true);
        }
        if left_buf[..left_n] != right_buf[..right_n] {
            return Ok(false);
        }
    }
}

fn report_delta(left: &RunSummary, right: &RunSummary) -> ReportDelta {
    let left_titles: Vec<&str> = left.findings.iter().map(|f| f.title.as_str()).collect();
    let right_titles: Vec<&str> = right.findings.iter().map(|f| f.title.as_str()).collect();
    ReportDelta {
        left_status: format!("{:?}", left.status).to_lowercase(),
        right_status: format!("{:?}", right.status).to_lowercase(),
        left_mode: format!("{:?}", left.mode).to_lowercase(),
        right_mode: format!("{:?}", right.mode).to_lowercase(),
        left_findings: left.findings.len(),
        right_findings: right.findings.len(),
        left_duration_ms: left.duration_ms,
        right_duration_ms: right.duration_ms,
        finding_titles_changed: left_titles != right_titles,
    }
}

fn trace_delta(left: &TraceFile, right: &TraceFile) -> TraceDelta {
    let first_decision_diff_index = left
        .decisions
        .iter()
        .zip(right.decisions.iter())
        .position(|(a, b)| a != b)
        .or_else(|| {
            if left.decisions.len() != right.decisions.len() {
                Some(left.decisions.len().min(right.decisions.len()))
            } else {
                None
            }
        });

    let first_event_diff_index = left
        .events
        .iter()
        .zip(right.events.iter())
        .position(|(a, b)| a.time_ms != b.time_ms || a.name != b.name || a.fields != b.fields)
        .or_else(|| {
            if left.events.len() != right.events.len() {
                Some(left.events.len().min(right.events.len()))
            } else {
                None
            }
        });

    TraceDelta {
        left_mode: format!("{:?}", left.mode).to_lowercase(),
        right_mode: format!("{:?}", right.mode).to_lowercase(),
        left_decisions: left.decisions.len(),
        right_decisions: right.decisions.len(),
        left_events: left.events.len(),
        right_events: right.events.len(),
        first_decision_diff_index,
        first_event_diff_index,
    }
}

fn load_summary(config: &Config, run: &str) -> FozzyResult<Option<RunSummary>> {
    if let Some(view) = crate::resolve_artifact_selector_view(config, run)? {
        return Ok(Some(match view {
            crate::ArtifactSelectorView::DirectTrace { trace, .. } => trace.summary,
            crate::ArtifactSelectorView::ValidatedBundle(bundle) => bundle.summary,
        }));
    }

    let trace = load_trace(config, run)?;
    Ok(trace.map(|t| t.summary))
}

fn load_trace(config: &Config, run: &str) -> FozzyResult<Option<TraceFile>> {
    let input = crate::normalize_trace_path(&PathBuf::from(run));
    let trace_path = if input.exists()
        && input.is_file()
        && input
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("fozzy"))
    {
        input
    } else {
        let artifacts_dir = crate::resolve_artifacts_dir(config, run)?;
        let Some(trace_path) = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)? else {
            return Ok(None);
        };
        trace_path
    };

    if !trace_path.exists() {
        return Ok(None);
    }
    Ok(Some(TraceFile::read_json(&trace_path)?))
}

fn push_if_exists(
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

fn export_artifacts_zip(files: &[PathBuf], out_zip: &Path) -> FozzyResult<()> {
    use std::fs::File;
    use std::io::{Read as _, Write as _};

    validate_output_file_path_secure(out_zip)?;
    if let Some(parent) = out_zip.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_name = out_zip
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("artifacts.zip");
    let tmp_name = format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4()
    );
    let tmp_path = out_zip
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(tmp_name);

    let write_result = (|| -> FozzyResult<()> {
        let file = File::create(&tmp_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o644);
        let mut used_names: BTreeSet<String> = BTreeSet::new();

        for src in files {
            let name = zip_entry_name_for_path(src, &mut used_names);
            zip.start_file(name, options)?;
            let mut in_file = File::open(src)?;
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = in_file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buf[..n])?;
            }
        }

        zip.finish()?;
        Ok(())
    })();

    match write_result {
        Ok(()) => {
            std::fs::rename(&tmp_path, out_zip)?;
            Ok(())
        }
        Err(err) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(err)
        }
    }
}

fn zip_entry_name_for_path(path: &Path, used: &mut BTreeSet<String>) -> String {
    let base = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "artifact".to_string());

    let mut safe: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    while safe.contains("__") {
        safe = safe.replace("__", "_");
    }
    safe = safe.trim_matches('_').to_string();
    if safe.is_empty() {
        safe = "artifact".to_string();
    }

    let (stem, ext) = match safe.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && !e.is_empty() => (s.to_string(), Some(e.to_string())),
        _ => (safe.clone(), None),
    };

    if used.insert(safe.clone()) {
        return safe;
    }

    for i in 2..=10_000usize {
        let candidate = match &ext {
            Some(ext) => format!("{stem}.{i}.{ext}"),
            None => format!("{stem}.{i}"),
        };
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    "artifact.overflow".to_string()
}

fn copy_file_into_dir_secure(src: &Path, out_dir: &Path) -> FozzyResult<()> {
    if out_dir.exists() {
        let out_md = std::fs::symlink_metadata(out_dir)?;
        if out_md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to write into symlinked output directory: {}",
                out_dir.display()
            )));
        }
    }

    let name = src.file_name().ok_or_else(|| {
        crate::FozzyError::InvalidArgument(format!("invalid artifact path: {}", src.display()))
    })?;
    let dst = out_dir.join(name);
    if dst.exists() {
        let dst_md = std::fs::symlink_metadata(&dst)?;
        if dst_md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to overwrite symlinked output file: {}",
                dst.display()
            )));
        }
        if !dst_md.is_file() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to overwrite non-file output path: {}",
                dst.display()
            )));
        }
        std::fs::remove_file(&dst)?;
    }

    let tmp_name = format!(
        ".{}.{}.{}.tmp",
        dst.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("artifact"),
        std::process::id(),
        uuid::Uuid::new_v4()
    );
    let tmp = out_dir.join(tmp_name);
    std::fs::copy(src, &tmp)?;
    std::fs::rename(&tmp, &dst)?;
    Ok(())
}

fn validate_copy_targets_secure(files: &[PathBuf], out_dir: &Path) -> FozzyResult<()> {
    validate_output_dir_path_secure(out_dir)?;
    if out_dir.exists() {
        let out_md = std::fs::symlink_metadata(out_dir)?;
        if out_md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to write into symlinked output directory: {}",
                out_dir.display()
            )));
        }
    }

    let mut seen = std::collections::BTreeSet::<String>::new();
    for src in files {
        let name = src
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                crate::FozzyError::InvalidArgument(format!(
                    "invalid artifact path: {}",
                    src.display()
                ))
            })?
            .to_string();
        if !seen.insert(name.clone()) {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "duplicate output file target detected: {name}"
            )));
        }
        let dst = out_dir.join(&name);
        if dst.exists() {
            let dst_md = std::fs::symlink_metadata(&dst)?;
            if dst_md.file_type().is_symlink() {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "refusing to overwrite symlinked output file: {}",
                    dst.display()
                )));
            }
            if !dst_md.is_file() {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "refusing to overwrite non-file output path: {}",
                    dst.display()
                )));
            }
        }
    }

    Ok(())
}

fn export_artifacts_dir_exact(files: &[PathBuf], out_dir: &Path) -> FozzyResult<()> {
    std::fs::create_dir_all(out_dir)?;
    validate_copy_targets_secure(files, out_dir)?;
    prune_stale_output_entries(files, out_dir)?;
    for src in files {
        copy_file_into_dir_secure(src, out_dir)?;
    }
    Ok(())
}

fn prune_stale_output_entries(files: &[PathBuf], out_dir: &Path) -> FozzyResult<()> {
    let expected: BTreeSet<String> = files
        .iter()
        .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .collect();
    for entry in std::fs::read_dir(out_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if expected.contains(&name) {
            continue;
        }
        let path = entry.path();
        let md = std::fs::symlink_metadata(&path)?;
        if md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to remove symlinked stale output entry: {}",
                path.display()
            )));
        }
        if md.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn is_direct_trace_input(run: &str) -> bool {
    let p = PathBuf::from(crate::normalize_run_or_trace_selector(run));
    p.exists() && p.is_file() && crate::is_trace_path(&p)
}

fn validate_output_file_path_secure(out_file: &Path) -> FozzyResult<()> {
    if out_file.exists() {
        let md = std::fs::symlink_metadata(out_file)?;
        if md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to overwrite symlinked output file: {}",
                out_file.display()
            )));
        }
    }
    validate_output_dir_path_secure(out_file.parent().unwrap_or_else(|| Path::new(".")))
}

fn validate_output_dir_path_secure(path: &Path) -> FozzyResult<()> {
    let is_abs = path.is_absolute();
    let mut cur = if is_abs {
        PathBuf::from(Path::new(std::path::MAIN_SEPARATOR_STR))
    } else {
        std::env::current_dir()?
    };
    let mut normal_seen = 0usize;
    for comp in path.components() {
        use std::path::Component;
        match comp {
            Component::Prefix(prefix) => cur.push(prefix.as_os_str()),
            Component::RootDir => {}
            Component::CurDir => continue,
            Component::ParentDir => cur.push(".."),
            Component::Normal(seg) => {
                normal_seen += 1;
                cur.push(seg);
            }
        }
        if cur.exists() {
            let md = std::fs::symlink_metadata(&cur)?;
            let skip_abs_top_component = is_abs && normal_seen == 1;
            if md.file_type().is_symlink() && !skip_abs_top_component {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "refusing to write through symlinked output path: {}",
                    cur.display()
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "artifacts/tests.rs"]
mod tests;
