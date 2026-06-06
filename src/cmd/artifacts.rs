//! Artifact management (`fozzy artifacts ...`).

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::{Config, ExitStatus, FozzyError, FozzyResult, RunManifest, RunSummary, TraceFile};

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
        let allow_sidecars_without_metadata =
            trusted_trace_declared_artifacts_dir(&run_path)?.is_some();
        let artifacts_dir = resolve_artifacts_dir(config, run)?;
        if artifacts_dir != run_path {
            for (kind, path) in [
                (ArtifactKind::Timeline, artifacts_dir.join("timeline.json")),
                (
                    ArtifactKind::Profile,
                    artifacts_dir.join("profile.timeline.json"),
                ),
                (ArtifactKind::Profile, artifacts_dir.join("profile.cpu.json")),
                (ArtifactKind::Profile, artifacts_dir.join("profile.heap.json")),
                (
                    ArtifactKind::Profile,
                    artifacts_dir.join("profile.latency.json"),
                ),
                (
                    ArtifactKind::Profile,
                    artifacts_dir.join("profile.metrics.json"),
                ),
                (ArtifactKind::Profile, artifacts_dir.join("symbols.json")),
                (
                    ArtifactKind::Memory,
                    artifacts_dir.join("memory.timeline.json"),
                ),
                (ArtifactKind::Memory, artifacts_dir.join("memory.leaks.json")),
                (ArtifactKind::Memory, artifacts_dir.join("memory.graph.json")),
                (ArtifactKind::Memory, artifacts_dir.join("memory.delta.json")),
                (ArtifactKind::Report, artifacts_dir.join("report.json")),
                (ArtifactKind::Events, artifacts_dir.join("events.json")),
                (ArtifactKind::Coverage, artifacts_dir.join("coverage.json")),
                (ArtifactKind::Manifest, artifacts_dir.join("manifest.json")),
                (ArtifactKind::Report, artifacts_dir.join("report.html")),
                (ArtifactKind::Report, artifacts_dir.join("junit.xml")),
            ] {
                if path.exists() && path.is_file() {
                    files.push(path.clone());
                }
                push_if_exists(&mut out, kind, path)?;
            }
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

    let artifacts_dir = resolve_artifacts_dir(config, run)?;
    if !artifacts_dir.exists() {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "run artifacts not found: {}",
            artifacts_dir.display()
        )));
    }
    validate_run_artifacts_for_listing(&artifacts_dir, run)?;
    let mut out = Vec::new();

    if let Some(trace_path) = resolve_trace_path_from_artifacts_dir(&artifacts_dir)? {
        push_if_exists(&mut out, ArtifactKind::Trace, trace_path)?;
    }
    push_if_exists(
        &mut out,
        ArtifactKind::Timeline,
        artifacts_dir.join("timeline.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Profile,
        artifacts_dir.join("profile.timeline.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Profile,
        artifacts_dir.join("profile.cpu.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Profile,
        artifacts_dir.join("profile.heap.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Profile,
        artifacts_dir.join("profile.latency.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Profile,
        artifacts_dir.join("profile.metrics.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Profile,
        artifacts_dir.join("symbols.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Memory,
        artifacts_dir.join("memory.timeline.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Memory,
        artifacts_dir.join("memory.leaks.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Memory,
        artifacts_dir.join("memory.graph.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Memory,
        artifacts_dir.join("memory.delta.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Report,
        artifacts_dir.join("report.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Events,
        artifacts_dir.join("events.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Coverage,
        artifacts_dir.join("coverage.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Manifest,
        artifacts_dir.join("manifest.json"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Report,
        artifacts_dir.join("report.html"),
    )?;
    push_if_exists(
        &mut out,
        ArtifactKind::Report,
        artifacts_dir.join("junit.xml"),
    )?;

    Ok(out)
}

fn validate_run_artifacts_for_listing(artifacts_dir: &Path, run: &str) -> FozzyResult<()> {
    let report = artifacts_dir.join("report.json");
    let manifest = artifacts_dir.join("manifest.json");
    let trace = resolve_trace_path_from_artifacts_dir(artifacts_dir)?;
    if !report.exists() && !manifest.exists() {
        if trace.is_some() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "incomplete artifacts for {run:?}; missing required files: report.json, manifest.json"
            )));
        }
        return Ok(());
    }

    let mut files = Vec::new();
    if report.exists() {
        files.push(report);
    }
    if manifest.exists() {
        files.push(manifest);
    }
    if let Some(trace) = trace {
        files.push(trace);
    }
    validate_required_bundle_files(&files, run)?;
    validate_manifest_integrity(&files, run)
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
        validate_required_bundle_files(&files, run)?;
        validate_manifest_integrity(&files, run)?;
    } else {
        let trace_path = crate::normalize_trace_path(&PathBuf::from(run));
        validate_direct_trace_bundle(
            &files,
            run,
            trusted_trace_declared_artifacts_dir(&trace_path)?.is_some(),
        )?;
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
    let artifacts_dir = resolve_artifacts_dir(config, run)?;

    let mut source_files: Vec<PathBuf> = vec![trace_path.clone()];
    for name in ["report.json", "manifest.json"] {
        let path = artifacts_dir.join(name);
        if path.exists() && path.is_file() {
            source_files.push(path);
        }
    }
    source_files.sort();
    source_files.dedup();
    if direct_trace_input {
        validate_direct_trace_bundle(
            &source_files,
            run,
            trusted_trace_declared_artifacts_dir(&trace_path)?.is_some(),
        )?;
    } else {
        validate_required_bundle_files(&source_files, run)?;
        validate_manifest_integrity(&source_files, run)?;
    }

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
        validate_required_bundle_files(&files, run)?;
        validate_manifest_integrity(&files, run)?;
    } else {
        let trace_path = crate::normalize_trace_path(&PathBuf::from(run));
        validate_direct_trace_bundle(
            &files,
            run,
            trusted_trace_declared_artifacts_dir(&trace_path)?.is_some(),
        )?;
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
    let artifacts_dir = resolve_artifacts_dir(config, run)?;
    let Some(trace) = resolve_trace_path_from_artifacts_dir(&artifacts_dir)? else {
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
        files.push(ArtifactFileDelta {
            key,
            left_path,
            right_path,
            left_size_bytes: left_size,
            right_size_bytes: right_size,
            changed: left_size != right_size || l.is_none() || r.is_none(),
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
    let input = crate::normalize_trace_path(&PathBuf::from(run));
    if input.exists()
        && input.is_file()
        && input
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("fozzy"))
    {
        return Ok(Some(TraceFile::read_json(&input)?.summary));
    }

    let artifacts_dir = resolve_artifacts_dir(config, run)?;
    if let Some(summary) = load_checked_report_summary_from_artifacts_dir(&artifacts_dir, run)? {
        return Ok(Some(summary));
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
        let artifacts_dir = resolve_artifacts_dir(config, run)?;
        let Some(trace_path) = resolve_trace_path_from_artifacts_dir(&artifacts_dir)? else {
            return Ok(None);
        };
        trace_path
    };

    if !trace_path.exists() {
        return Ok(None);
    }
    Ok(Some(TraceFile::read_json(&trace_path)?))
}

pub(crate) fn resolve_artifacts_dir(config: &Config, run: &str) -> FozzyResult<PathBuf> {
    // `run` can be:
    // - a run id (directory `.fozzy/runs/<runId>`)
    // - a trace path (`*.fozzy`) that either is `.../trace.fozzy` or points to a trace file.
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

        // A direct trace file, or any file within the artifacts dir.
        if let Some(parent) = path.parent() {
            return Ok(parent.to_path_buf());
        }
    }

    if let Some(alias_path) = resolve_run_alias(config, run)? {
        return Ok(alias_path);
    }

    Ok(config.runs_dir().join(run))
}

fn trace_declared_artifacts_dir(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    let trace = TraceFile::read_json(trace_path)?;
    Ok(trace
        .summary
        .identity
        .artifacts_dir
        .map(PathBuf::from)
        .filter(|path| path.exists() && path.is_dir()))
}

fn trusted_trace_declared_artifacts_dir(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    let Some(artifacts_dir) = trace_declared_artifacts_dir(trace_path)? else {
        return Ok(None);
    };
    let Some(summary) = load_checked_report_summary_from_artifacts_dir(
        &artifacts_dir,
        &trace_path.display().to_string(),
    )?
    else {
        return Ok(None);
    };
    let Some(resolved_trace) = resolve_trace_path_from_artifacts_dir(&artifacts_dir)? else {
        return Ok(None);
    };
    let trace = TraceFile::read_json(trace_path)?;
    let expected_trace =
        std::fs::canonicalize(trace_path).unwrap_or_else(|_| trace_path.to_path_buf());
    let actual_trace =
        std::fs::canonicalize(&resolved_trace).unwrap_or_else(|_| resolved_trace.clone());
    if actual_trace != expected_trace {
        return Ok(None);
    }
    if summary.identity.run_id != trace.summary.identity.run_id
        || summary.identity.seed != trace.summary.identity.seed
    {
        return Ok(None);
    }
    Ok(Some(artifacts_dir))
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
        if trace_path.exists() {
            Some(trace_path)
        } else {
            None
        }
    } else {
        None
    };

    let manifest_path = artifacts_dir.join("manifest.json");
    let manifest_trace = if manifest_path.exists()
        && let Ok(manifest) = serde_json::from_slice::<RunManifest>(&std::fs::read(&manifest_path)?)
        && let Some(path) = manifest.trace_path
    {
        let trace_path = PathBuf::from(path);
        if trace_path.exists() {
            Some(trace_path)
        } else {
            None
        }
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

    if let Some(declared_trace) = declared_trace {
        return Ok(Some(declared_trace));
    }

    if let Some(local_trace) = local_trace {
        return Ok(Some(local_trace));
    }

    Ok(None)
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
    validate_manifest_integrity(&files, run)?;

    Ok(Some(serde_json::from_slice(&std::fs::read(report_path)?)?))
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
        let summary = match load_checked_report_summary_from_artifacts_dir(
            &dir,
            &dir.display().to_string(),
        ) {
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

fn validate_required_bundle_files(files: &[PathBuf], run: &str) -> FozzyResult<()> {
    let present: BTreeSet<String> = files
        .iter()
        .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .collect();
    // Run-id exports must work for normal runs where trace/events are optional.
    // The stable minimum contract is report + manifest.
    let required = ["report.json", "manifest.json"];
    let missing: Vec<&str> = required
        .into_iter()
        .filter(|name| !present.contains(*name))
        .collect();
    if !missing.is_empty() {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "incomplete artifacts for {run:?}; missing required files: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

fn validate_direct_trace_bundle(
    files: &[PathBuf],
    run: &str,
    allow_sidecars_without_metadata: bool,
) -> FozzyResult<()> {
    let present: BTreeSet<String> = files
        .iter()
        .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .collect();
    let has_report = present.contains("report.json");
    let has_manifest = present.contains("manifest.json");
    if has_report != has_manifest {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "incomplete direct trace artifacts for {run:?}; report.json and manifest.json must appear together"
        )));
    }
    if !allow_sidecars_without_metadata && !has_report && !has_manifest && files.len() > 1 {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "untrusted sibling artifacts for direct trace {run:?}; report.json and manifest.json are required to trust sibling files"
        )));
    }
    if has_report && has_manifest {
        validate_manifest_integrity(files, run)?;
    }
    Ok(())
}

fn validate_manifest_integrity(files: &[PathBuf], run: &str) -> FozzyResult<()> {
    let manifest_path = files
        .iter()
        .find(|p| p.file_name().and_then(|s| s.to_str()) == Some("manifest.json"))
        .ok_or_else(|| {
            crate::FozzyError::InvalidArgument(format!(
                "incomplete artifacts for {run:?}; missing required files: manifest.json"
            ))
        })?;
    let bytes = std::fs::read(manifest_path)?;
    let manifest: RunManifest = serde_json::from_slice(&bytes).map_err(|e| {
        crate::FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: {} ({e})",
            manifest_path.display()
        ))
    })?;
    if manifest.schema_version != "fozzy.run_manifest.v1" {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: unsupported schemaVersion {}",
            manifest.schema_version
        )));
    }
    let report_path = files
        .iter()
        .find(|p| p.file_name().and_then(|s| s.to_str()) == Some("report.json"))
        .ok_or_else(|| {
            crate::FozzyError::InvalidArgument(format!(
                "incomplete artifacts for {run:?}; missing required files: report.json"
            ))
        })?;
    let report_bytes = std::fs::read(report_path)?;
    let report: RunSummary = serde_json::from_slice(&report_bytes).map_err(|e| {
        crate::FozzyError::InvalidArgument(format!(
            "invalid report for {run:?}: {} ({e})",
            report_path.display()
        ))
    })?;
    let expected_artifacts_dir = report_path
        .parent()
        .ok_or_else(|| {
            crate::FozzyError::InvalidArgument(format!(
                "invalid report for {run:?}: {} has no parent directory",
                report_path.display()
            ))
        })?
        .to_string_lossy()
        .to_string();
    let report_path_string = report_path.to_string_lossy().to_string();
    let trace_path = files
        .iter()
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("fozzy"));
    let report_trace_path = report.identity.trace_path.clone();
    let manifest_trace_path = manifest.trace_path.clone();
    if manifest.run_id != report.identity.run_id
        || manifest.mode != report.mode
        || manifest.status != report.status
        || manifest.seed != report.identity.seed
        || manifest.duration_ms != report.duration_ms
        || manifest.duration_ns != report.duration_ns
        || manifest.findings_count != report.findings.len()
        || manifest.report_path.as_ref() != Some(&report_path_string)
        || manifest.artifacts_dir.as_ref() != Some(&expected_artifacts_dir)
        || report.identity.report_path.as_ref() != Some(&report_path_string)
        || report.identity.artifacts_dir.as_ref() != Some(&expected_artifacts_dir)
        || manifest_trace_path != report_trace_path
    {
        return Err(crate::FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: report/manifest identity mismatch"
        )));
    }
    match manifest_trace_path {
        Some(ref expected_trace) => {
            let actual_trace = trace_path.ok_or_else(|| {
                crate::FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: missing declared trace artifact {expected_trace}"
                ))
            })?;
            if actual_trace.to_string_lossy() != expected_trace.as_str() {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: declared trace artifact mismatch"
                )));
            }
            let trace_bytes = std::fs::read(actual_trace)?;
            let trace: TraceFile = serde_json::from_slice(&trace_bytes).map_err(|e| {
                crate::FozzyError::InvalidArgument(format!(
                    "invalid trace for {run:?}: {} ({e})",
                    actual_trace.display()
                ))
            })?;
            if trace.summary.identity.trace_path.as_ref() != Some(expected_trace) {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: trace/report identity mismatch"
                )));
            }
            if manifest.mode == crate::RunMode::Replay {
                if trace.summary.status != manifest.status
                    || trace.summary.identity.seed != manifest.seed
                {
                    return Err(crate::FozzyError::InvalidArgument(format!(
                        "invalid manifest for {run:?}: replay source trace mismatch"
                    )));
                }
            } else if trace.summary.identity.run_id != manifest.run_id
                || trace.summary.mode != manifest.mode
                || trace.summary.status != manifest.status
                || trace.summary.identity.seed != manifest.seed
                || trace.summary.identity.report_path.as_ref() != Some(&report_path_string)
                || trace.summary.identity.artifacts_dir.as_ref() != Some(&expected_artifacts_dir)
            {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: trace/report identity mismatch"
                )));
            }
        }
        None => {
            if trace_path.is_some() {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: undeclared trace artifact present"
                )));
            }
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
mod tests {
    use super::*;

    fn valid_manifest_json(run_id: &str) -> String {
        format!(
            r#"{{"schemaVersion":"fozzy.run_manifest.v1","runId":"{run_id}","mode":"run","status":"pass","seed":1,"startedAt":"2026-01-01T00:00:00Z","finishedAt":"2026-01-01T00:00:00Z","durationMs":0,"findingsCount":0}}"#
        )
    }

    fn valid_report_json(run_id: &str, report_path: &Path, artifacts_dir: &Path) -> String {
        format!(
            r#"{{
  "status":"pass",
  "mode":"run",
  "identity":{{
    "runId":"{run_id}",
    "seed":1,
    "reportPath":"{}",
    "artifactsDir":"{}"
  }},
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "findings":[]
}}"#,
            report_path.display(),
            artifacts_dir.display()
        )
    }

    fn valid_report_and_manifest_json(
        run_id: &str,
        report_path: &Path,
        artifacts_dir: &Path,
        trace_path: Option<&Path>,
    ) -> (String, String) {
        let trace_json = trace_path.map(|path| format!(r#","tracePath":"{}""#, path.display()));
        let report = format!(
            r#"{{
  "status":"pass",
  "mode":"run",
  "identity":{{
    "runId":"{run_id}",
    "seed":1,
    "reportPath":"{}",
    "artifactsDir":"{}"{}
  }},
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "findings":[]
}}"#,
            report_path.display(),
            artifacts_dir.display(),
            trace_json.clone().unwrap_or_default()
        );
        let manifest = format!(
            r#"{{
  "schemaVersion":"fozzy.run_manifest.v1",
  "runId":"{run_id}",
  "mode":"run",
  "status":"pass",
  "seed":1,
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "reportPath":"{}",
  "artifactsDir":"{}",
  "findingsCount":0{}
}}"#,
            report_path.display(),
            artifacts_dir.display(),
            trace_path
                .map(|path| format!(r#","tracePath":"{}""#, path.display()))
                .unwrap_or_default()
        );
        (report, manifest)
    }

    fn valid_trace_json(
        run_id: &str,
        trace_path: &Path,
        report_path: &Path,
        artifacts_dir: &Path,
    ) -> String {
        format!(
            r#"{{
  "format":"fozzy-trace",
  "version":4,
  "engine":{{"version":"0.1.0"}},
  "mode":"run",
  "scenario_path":null,
  "scenario":{{"version":1,"name":"x","steps":[]}},
  "decisions":[],
  "events":[],
  "summary":{{
    "status":"pass",
    "mode":"run",
    "identity":{{
      "runId":"{run_id}",
      "seed":1,
      "tracePath":"{}",
      "reportPath":"{}",
      "artifactsDir":"{}"
    }},
    "startedAt":"2026-01-01T00:00:00Z",
    "finishedAt":"2026-01-01T00:00:00Z",
    "durationMs":0,
    "durationNs":0
  }}
}}"#,
            trace_path.display(),
            report_path.display(),
            artifacts_dir.display()
        )
    }

    #[test]
    fn export_zip_normalizes_unicode_filenames_to_ascii() {
        let root =
            std::env::temp_dir().join(format!("fozzy-artifacts-unicode-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        let src_a = root.join("résumé-😀.json");
        let src_b = root.join("résumé 👀.json");
        std::fs::write(&src_a, b"{}").expect("write source a");
        std::fs::write(&src_b, b"{}").expect("write source b");
        let out = root.join("out.zip");

        export_artifacts_zip(&[src_a, src_b], &out).expect("zip export");

        let file = std::fs::File::open(&out).expect("open zip");
        let mut archive = zip::ZipArchive::new(file).expect("parse zip");
        let a = archive.by_index(0).expect("entry 0").name().to_string();
        let b = archive.by_index(1).expect("entry 1").name().to_string();

        assert!(a.is_ascii());
        assert!(b.is_ascii());
        assert_ne!(a, b);
        assert!(a.ends_with(".json"));
        assert!(b.ends_with(".json"));
    }

    #[test]
    fn export_missing_input_returns_error_and_does_not_create_zip() {
        let root =
            std::env::temp_dir().join(format!("fozzy-artifacts-missing-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        let out = root.join("missing-input.zip");

        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let err =
            export_artifacts(&cfg, "does-not-exist-input.fozzy", &out).expect_err("must fail");
        assert!(err.to_string().contains("not found"));
        assert!(!out.exists(), "zip should not exist on failure");
    }

    #[test]
    fn export_empty_run_errors() {
        let root =
            std::env::temp_dir().join(format!("fozzy-artifacts-empty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        let run_id = "empty-run";
        std::fs::create_dir_all(root.join(".fozzy").join("runs").join(run_id))
            .expect("create run dir");
        let out = root.join("empty.zip");

        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let err = export_artifacts(&cfg, run_id, &out).expect_err("must fail");
        assert!(err.to_string().contains("no artifacts found"));
        assert!(!out.exists(), "zip should not exist on failure");
    }

    #[test]
    fn pack_includes_runtime_metadata_files() {
        let root = std::env::temp_dir().join(format!("fozzy-pack-test-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let trace_path = run_dir.join("trace.fozzy");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("events.json"), b"[]").expect("events");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");
        let out = root.join("pack.zip");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        export_reproducer_pack(&cfg, "r1", &out).expect("pack");
        let file = std::fs::File::open(&out).expect("zip");
        let mut z = zip::ZipArchive::new(file).expect("zip parse");
        let mut names = Vec::new();
        for i in 0..z.len() {
            names.push(z.by_index(i).expect("entry").name().to_string());
        }
        assert!(names.iter().any(|n| n == "env.json"));
        assert!(names.iter().any(|n| n == "version.json"));
        assert!(names.iter().any(|n| n == "commandline.json"));
    }

    #[cfg(unix)]
    #[test]
    fn pack_dir_rejects_symlink_target_overwrite() {
        use std::os::unix::fs::symlink;

        let root =
            std::env::temp_dir().join(format!("fozzy-pack-symlink-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let trace_path = run_dir.join("trace.fozzy");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let outside = root.join("outside.json");
        std::fs::write(&outside, br#"{"victim":true}"#).expect("outside");
        let out_dir = root.join("out");
        std::fs::create_dir_all(&out_dir).expect("out");
        symlink(&outside, out_dir.join("report.json")).expect("symlink");

        let err = export_reproducer_pack(&cfg, "r1", &out_dir)
            .expect_err("must reject symlink overwrite");
        assert!(err.to_string().contains("symlinked output file"));
        let victim = std::fs::read_to_string(&outside).expect("read victim");
        assert!(victim.contains("victim"));
    }

    #[cfg(unix)]
    #[test]
    fn pack_dir_failure_atomic_on_symlink_error() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!("fozzy-pack-atomic-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let trace_path = run_dir.join("trace.fozzy");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let outside = root.join("outside.json");
        std::fs::write(&outside, br#"{"victim":true}"#).expect("outside");
        let out_dir = root.join("out");
        std::fs::create_dir_all(&out_dir).expect("out");
        symlink(&outside, out_dir.join("manifest.json")).expect("symlink");

        let err = export_reproducer_pack(&cfg, "r1", &out_dir)
            .expect_err("must reject symlink overwrite");
        assert!(err.to_string().contains("symlinked output file"));
        assert_eq!(
            std::fs::read(&outside).expect("victim read"),
            br#"{"victim":true}"#
        );
        assert!(
            !out_dir.join("report.json").exists(),
            "partial file should not be written"
        );
        assert!(
            !out_dir.join("events.json").exists(),
            "partial file should not be written"
        );
    }

    #[test]
    fn pack_zip_is_byte_deterministic_for_same_run() {
        let root =
            std::env::temp_dir().join(format!("fozzy-pack-deterministic-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let trace_path = run_dir.join("trace.fozzy");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");

        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let out_a = root.join("a.zip");
        let out_b = root.join("b.zip");
        export_reproducer_pack(&cfg, "r1", &out_a).expect("pack a");
        export_reproducer_pack(&cfg, "r1", &out_b).expect("pack b");

        let a = std::fs::read(&out_a).expect("read a");
        let b = std::fs::read(&out_b).expect("read b");
        assert_eq!(
            a, b,
            "repeated pack exports for same run must be byte-identical"
        );
    }

    #[test]
    fn export_and_pack_reject_incomplete_run_directory() {
        let root =
            std::env::temp_dir().join(format!("fozzy-pack-incomplete-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        std::fs::write(run_dir.join("manifest.json"), valid_manifest_json("r1")).expect("manifest");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let out_pack = root.join("pack.zip");
        let out_export = root.join("export.zip");

        let err_pack = export_reproducer_pack(&cfg, "r1", &out_pack)
            .expect_err("pack must fail for incomplete run");
        assert!(err_pack.to_string().contains("incomplete artifacts"));
        assert!(
            !out_pack.exists(),
            "pack zip should not be created on incomplete run"
        );

        let err_export = export_artifacts(&cfg, "r1", &out_export)
            .expect_err("export must fail for incomplete run");
        assert!(err_export.to_string().contains("incomplete artifacts"));
        assert!(
            !out_export.exists(),
            "export zip should not be created on incomplete run"
        );
    }

    #[test]
    fn resolve_artifacts_dir_supports_latest_last_pass_last_fail_aliases() {
        let root = std::env::temp_dir().join(format!("fozzy-aliases-{}", uuid::Uuid::new_v4()));
        let runs = root.join(".fozzy").join("runs");
        std::fs::create_dir_all(&runs).expect("runs dir");
        let mk = |id: &str, status: &str, finished: &str| {
            let dir = runs.join(id);
            std::fs::create_dir_all(&dir).expect("run dir");
            let report_path = dir.join("report.json");
            let trace_path = dir.join("trace.fozzy");
            let (report, manifest) =
                valid_report_and_manifest_json(id, &report_path, &dir, Some(&trace_path));
            let report = report
                .replace(r#""status":"pass""#, &format!(r#""status":"{status}""#))
                .replace(
                    r#""finishedAt":"2026-01-01T00:00:00Z""#,
                    &format!(r#""finishedAt":"{finished}""#),
                );
            let manifest = manifest
                .replace(r#""status":"pass""#, &format!(r#""status":"{status}""#))
                .replace(
                    r#""finishedAt":"2026-01-01T00:00:00Z""#,
                    &format!(r#""finishedAt":"{finished}""#),
                );
            let trace = valid_trace_json(id, &trace_path, &report_path, &dir).replace(
                r#""status":"pass""#,
                &format!(r#""status":"{status}""#),
            );
            std::fs::write(
                &trace_path,
                trace,
            )
            .expect("trace");
            std::fs::write(&report_path, report).expect("report");
            std::fs::write(dir.join("manifest.json"), manifest).expect("manifest");
        };
        mk("r1", "pass", "2026-02-19T00:00:01Z");
        mk("r2", "fail", "2026-02-19T00:00:02Z");
        mk("r3", "pass", "2026-02-19T00:00:03Z");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let latest = resolve_artifacts_dir(&cfg, "latest").expect("latest");
        assert!(latest.ends_with("r3"));
        let last_pass = resolve_artifacts_dir(&cfg, "last-pass").expect("last-pass");
        assert!(last_pass.ends_with("r3"));
        let last_fail = resolve_artifacts_dir(&cfg, "last-fail").expect("last-fail");
        assert!(last_fail.ends_with("r2"));
    }

    #[test]
    fn direct_trace_uses_declared_artifacts_dir_and_lists_detached_profile_files() {
        let root =
            std::env::temp_dir().join(format!("fozzy-trace-artifacts-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("root dir");
        let trace = root.join("record.trace.min.fozzy");
        let detached_artifacts = root.join("record.trace.min.profile-artifacts");
        std::fs::create_dir_all(&detached_artifacts).expect("artifacts dir");
        let report_path = detached_artifacts.join("report.json");
        std::fs::write(
            &trace,
            valid_trace_json("r1", &trace, &report_path, &detached_artifacts),
        )
        .expect("write trace");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &detached_artifacts, Some(&trace));
        std::fs::write(&report_path, report).expect("write report");
        std::fs::write(detached_artifacts.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            detached_artifacts.join("profile.metrics.json"),
            br#"{"schemaVersion":"fozzy.profile_metrics.v1"}"#,
        )
        .expect("write metrics");

        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let resolved =
            resolve_artifacts_dir(&cfg, &trace.to_string_lossy()).expect("resolve artifacts dir");
        assert_eq!(resolved, detached_artifacts);

        let entries = artifacts_list(&cfg, &trace.to_string_lossy()).expect("artifacts list");
        assert!(entries.iter().any(|entry| {
            entry.path
                == detached_artifacts
                    .join("profile.metrics.json")
                    .to_string_lossy()
        }));
    }

    #[test]
    fn direct_trace_ignores_untrusted_declared_artifacts_dir() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-trace-untrusted-declared-artifacts-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("root dir");
        let trace = root.join("record.trace.fozzy");
        let forged_artifacts = root.join("forged-artifacts");
        std::fs::create_dir_all(&forged_artifacts).expect("forged dir");
        std::fs::write(
            &trace,
            format!(
                r#"{{
  "format":"fozzy-trace",
  "version":4,
  "engine":{{"version":"0.1.0"}},
  "mode":"run",
  "scenario_path":null,
  "scenario":{{"version":1,"name":"x","steps":[]}},
  "decisions":[],
  "events":[],
  "summary":{{
    "status":"pass",
    "mode":"run",
    "identity":{{
      "runId":"r1",
      "seed":7,
      "tracePath":"{}",
      "artifactsDir":"{}"
    }},
    "startedAt":"2026-01-01T00:00:00Z",
    "finishedAt":"2026-01-01T00:00:00Z",
    "durationMs":1
  }}
}}"#,
                trace.display(),
                forged_artifacts.display()
            ),
        )
        .expect("write trace");
        std::fs::write(
            forged_artifacts.join("profile.metrics.json"),
            br#"{"schemaVersion":"forged"}"#,
        )
        .expect("write forged metrics");

        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let resolved = resolve_artifacts_dir(&cfg, &trace.to_string_lossy()).expect("resolve");
        assert_eq!(resolved, root);

        let entries = artifacts_list(&cfg, &trace.to_string_lossy()).expect("artifacts list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, trace.to_string_lossy());
    }

    #[test]
    fn run_id_uses_report_declared_external_trace_path() {
        let root =
            std::env::temp_dir().join(format!("fozzy-artifacts-external-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let external_trace = root.join("external.trace.fozzy");
        let report_path = run_dir.join("report.json");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&external_trace));
        std::fs::write(
            &external_trace,
            valid_trace_json("r1", &external_trace, &report_path, &run_dir),
        )
        .expect("trace");
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let trace_path = resolve_trace_path(&cfg, "r1").expect("resolve trace path");
        assert_eq!(trace_path, external_trace);
        let entries = artifacts_list(&cfg, "r1").expect("artifacts list");
        assert!(entries.iter().any(|entry| {
            entry.path == external_trace.to_string_lossy()
                && matches!(entry.kind, ArtifactKind::Trace)
        }));
    }

    #[test]
    fn resolve_trace_path_rejects_conflicting_local_and_declared_trace_identities() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-conflict-local-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let local_trace = run_dir.join("trace.fozzy");
        let external_trace = root.join("external.trace.fozzy");
        let report_path = run_dir.join("report.json");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&external_trace));
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            &local_trace,
            valid_trace_json("r1", &local_trace, &report_path, &run_dir),
        )
        .expect("local trace");
        std::fs::write(
            &external_trace,
            valid_trace_json("r1", &external_trace, &report_path, &run_dir),
        )
        .expect("external trace");

        let err =
            resolve_trace_path_from_artifacts_dir(&run_dir).expect_err("must reject conflict");
        assert!(
            err.to_string()
                .contains("conflicting local and declared trace identities")
        );
    }

    #[test]
    fn resolve_trace_path_rejects_conflicting_report_and_manifest_trace_identities() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-conflict-declared-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_trace = root.join("report.trace.fozzy");
        let manifest_trace = root.join("manifest.trace.fozzy");
        let report_path = run_dir.join("report.json");
        std::fs::write(
            &report_path,
            valid_report_json("r1", &report_path, &run_dir).replace(
                &format!(r#""artifactsDir":"{}""#, run_dir.display()),
                &format!(
                    r#""artifactsDir":"{}","tracePath":"{}""#,
                    run_dir.display(),
                    report_trace.display()
                ),
            ),
        )
        .expect("report");
        std::fs::write(
            run_dir.join("manifest.json"),
            format!(
                r#"{{
  "schemaVersion":"fozzy.run_manifest.v1",
  "runId":"r1",
  "mode":"run",
  "status":"pass",
  "seed":1,
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "reportPath":"{}",
  "artifactsDir":"{}",
  "tracePath":"{}",
  "findingsCount":0
}}"#,
                report_path.display(),
                run_dir.display(),
                manifest_trace.display()
            ),
        )
        .expect("manifest");
        std::fs::write(
            &report_trace,
            valid_trace_json("r1", &report_trace, &report_path, &run_dir),
        )
        .expect("report trace");
        std::fs::write(
            &manifest_trace,
            valid_trace_json("r1", &manifest_trace, &report_path, &run_dir),
        )
        .expect("manifest trace");

        let err =
            resolve_trace_path_from_artifacts_dir(&run_dir).expect_err("must reject conflict");
        assert!(
            err.to_string()
                .contains("conflicting declared trace identities")
        );
    }

    #[test]
    fn export_and_pack_allow_run_dirs_without_trace_or_events() {
        let root =
            std::env::temp_dir().join(format!("fozzy-pack-minimal-run-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let (report, manifest) = valid_report_and_manifest_json("r1", &report_path, &run_dir, None);
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let out_pack = root.join("pack.zip");
        let out_export = root.join("export.zip");

        export_reproducer_pack(&cfg, "r1", &out_pack).expect("pack should succeed");
        export_artifacts(&cfg, "r1", &out_export).expect("export should succeed");
        assert!(out_pack.exists(), "pack zip should exist");
        assert!(out_export.exists(), "export zip should exist");
    }

    #[test]
    fn direct_trace_export_and_pack_reject_incomplete_declared_detached_metadata() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-direct-trace-partial-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        let artifacts_dir = root.join("artifacts");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        std::fs::write(
            &trace_path,
            valid_trace_json(
                "r1",
                &trace_path,
                &artifacts_dir.join("report.json"),
                &artifacts_dir,
            ),
        )
        .expect("trace");
        std::fs::write(
            artifacts_dir.join("report.json"),
            valid_report_json("r1", &artifacts_dir.join("report.json"), &artifacts_dir),
        )
        .expect("report");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let out_pack = root.join("pack.zip");
        let out_export = root.join("export.zip");

        let err_pack = export_reproducer_pack(&cfg, &trace_path.to_string_lossy(), &out_pack)
            .expect_err("pack must fail");
        assert!(
            err_pack
                .to_string()
                .contains("missing required files: manifest.json")
        );
        let err_export = export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
            .expect_err("export must fail");
        assert!(
            err_export
                .to_string()
                .contains("missing required files: manifest.json")
        );
    }

    #[test]
    fn direct_trace_list_rejects_incomplete_declared_detached_metadata() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-direct-trace-list-partial-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        let artifacts_dir = root.join("artifacts");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        std::fs::write(
            &trace_path,
            valid_trace_json(
                "r1",
                &trace_path,
                &artifacts_dir.join("report.json"),
                &artifacts_dir,
            ),
        )
        .expect("trace");
        std::fs::write(
            artifacts_dir.join("report.json"),
            valid_report_json("r1", &artifacts_dir.join("report.json"), &artifacts_dir),
        )
        .expect("report");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let err = artifacts_list(&cfg, &trace_path.to_string_lossy()).expect_err("list must fail");
        assert!(
            err.to_string()
                .contains("missing required files: manifest.json")
        );
    }

    #[test]
    fn direct_trace_list_rejects_unchecked_sibling_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-direct-trace-list-unchecked-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: crate::RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: None,
            decisions: Vec::new(),
            events: Vec::new(),
            summary: crate::RunSummary {
                status: crate::ExitStatus::Pass,
                mode: crate::RunMode::Run,
                identity: crate::RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: None,
                    artifacts_dir: None,
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: None,
                findings: Vec::new(),
            },
            checksum: None,
        }
        .write_json(&trace_path)
        .expect("trace");
        std::fs::write(root.join("memory.graph.json"), br#"{"nodes":[],"edges":[]}"#)
            .expect("graph");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let err = artifacts_list(&cfg, &trace_path.to_string_lossy()).expect_err("list must fail");
        assert!(
            err.to_string()
                .contains("report.json and manifest.json are required to trust sibling files")
        );
    }

    #[test]
    fn direct_trace_export_and_pack_reject_unchecked_sibling_artifacts() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-direct-trace-export-unchecked-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: crate::RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: None,
            decisions: Vec::new(),
            events: Vec::new(),
            summary: crate::RunSummary {
                status: crate::ExitStatus::Pass,
                mode: crate::RunMode::Run,
                identity: crate::RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: None,
                    artifacts_dir: None,
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: None,
                findings: Vec::new(),
            },
            checksum: None,
        }
        .write_json(&trace_path)
        .expect("trace");
        std::fs::write(root.join("profile.metrics.json"), br#"{"domains":[]}"#)
            .expect("profile");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let out_pack = root.join("pack.zip");
        let out_export = root.join("export.zip");

        let err_pack = export_reproducer_pack(&cfg, &trace_path.to_string_lossy(), &out_pack)
            .expect_err("pack must fail");
        assert!(
            err_pack
                .to_string()
                .contains("report.json and manifest.json are required to trust sibling files")
        );
        let err_export = export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
            .expect_err("export must fail");
        assert!(
            err_export
                .to_string()
                .contains("report.json and manifest.json are required to trust sibling files")
        );
    }

    #[test]
    fn direct_trace_export_and_pack_allow_valid_sibling_metadata() {
        let root =
            std::env::temp_dir().join(format!("fozzy-direct-trace-valid-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        let artifacts_dir = root.join("artifacts");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
        let report_path = artifacts_dir.join("report.json");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &artifacts_dir, Some(&trace_path));
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &artifacts_dir),
        )
        .expect("trace");
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(artifacts_dir.join("manifest.json"), manifest).expect("manifest");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let out_pack = root.join("pack.zip");
        let out_export = root.join("export.zip");

        export_reproducer_pack(&cfg, &trace_path.to_string_lossy(), &out_pack)
            .expect("pack should succeed");
        export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
            .expect("export should succeed");
        assert!(out_pack.exists());
        assert!(out_export.exists());
    }

    #[test]
    fn load_summary_prefers_explicit_trace_over_sibling_bundle() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-direct-summary-precedence-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");

        let explicit_trace = root.join("direct.trace.fozzy");
        let sibling_trace = root.join("trace.fozzy");
        let report_path = root.join("report.json");

        std::fs::write(
            &explicit_trace,
            valid_trace_json("explicit-run", &explicit_trace, &report_path, &root),
        )
        .expect("explicit trace");

        let (report, manifest) = valid_report_and_manifest_json(
            "sibling-run",
            &report_path,
            &root,
            Some(&sibling_trace),
        );
        std::fs::write(
            &sibling_trace,
            valid_trace_json("sibling-run", &sibling_trace, &report_path, &root),
        )
        .expect("sibling trace");
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(root.join("manifest.json"), manifest).expect("manifest");

        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let summary = load_summary(&cfg, &explicit_trace.to_string_lossy())
            .expect("load summary")
            .expect("summary");
        assert_eq!(summary.identity.run_id, "explicit-run");
    }

    #[test]
    fn checked_report_loader_allows_replay_runs_to_reference_source_trace() {
        let root =
            std::env::temp_dir().join(format!("fozzy-replay-source-trace-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let trace_path = root.join("source.trace.fozzy");
        std::fs::write(
            &trace_path,
            valid_trace_json(
                "source-run",
                &trace_path,
                &root.join(".fozzy/runs/source-run/report.json"),
                &root.join(".fozzy/runs/source-run"),
            ),
        )
        .expect("write source trace");
        let report_path = run_dir.join("report.json");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "status": "pass",
                "mode": "replay",
                "identity": {
                    "runId": "r1",
                    "seed": 1,
                    "tracePath": trace_path,
                    "reportPath": report_path,
                    "artifactsDir": run_dir
                },
                "startedAt": "2026-01-01T00:00:00Z",
                "finishedAt": "2026-01-01T00:00:00Z",
                "durationMs": 0,
                "durationNs": 0,
                "findings": []
            }))
            .expect("report json"),
        )
        .expect("write report");
        std::fs::write(
            run_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "fozzy.run_manifest.v1",
                "runId": "r1",
                "mode": "replay",
                "status": "pass",
                "seed": 1,
                "startedAt": "2026-01-01T00:00:00Z",
                "finishedAt": "2026-01-01T00:00:00Z",
                "durationMs": 0,
                "durationNs": 0,
                "tracePath": trace_path,
                "reportPath": report_path,
                "artifactsDir": run_dir,
                "findingsCount": 0
            }))
            .expect("manifest json"),
        )
        .expect("write manifest");

        let summary = load_checked_report_summary_from_artifacts_dir(&run_dir, "r1")
            .expect("checked report load")
            .expect("summary");
        assert_eq!(summary.mode, crate::RunMode::Replay);
        assert_eq!(summary.identity.run_id, "r1");
    }

    #[test]
    fn artifacts_diff_rejects_stale_report_without_manifest() {
        let root =
            std::env::temp_dir().join(format!("fozzy-artifacts-stale-diff-{}", uuid::Uuid::new_v4()));
        let base_dir = root.join(".fozzy");
        let left_dir = base_dir.join("runs").join("left");
        let right_dir = base_dir.join("runs").join("right");
        std::fs::create_dir_all(&left_dir).expect("left dir");
        std::fs::create_dir_all(&right_dir).expect("right dir");

        std::fs::write(
            left_dir.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "status": "pass",
                "mode": "run",
                "identity": {
                    "runId": "left",
                    "seed": 1,
                    "tracePath": "/tmp/missing-left.trace.fozzy",
                    "reportPath": left_dir.join("report.json"),
                    "artifactsDir": left_dir
                },
                "startedAt": "2026-01-01T00:00:00Z",
                "finishedAt": "2026-01-01T00:00:00Z",
                "durationMs": 0,
                "durationNs": 0,
                "findings": []
            }))
            .expect("left report json"),
        )
        .expect("write left report");

        let trace_path = right_dir.join("trace.fozzy");
        let report_path = right_dir.join("report.json");
        let (report, manifest) =
            valid_report_and_manifest_json("right", &report_path, &right_dir, Some(&trace_path));
        std::fs::write(
            &trace_path,
            valid_trace_json("right", &trace_path, &report_path, &right_dir),
        )
        .expect("write right trace");
        std::fs::write(&report_path, report).expect("write right report");
        std::fs::write(right_dir.join("manifest.json"), manifest).expect("write right manifest");

        let cfg = crate::Config {
            base_dir,
            ..crate::Config::default()
        };
        let err = artifacts_diff(&cfg, "left", "right").expect_err("must reject stale left");
        assert!(err.to_string().contains("missing required files: manifest.json"));
    }

    #[test]
    fn artifacts_list_rejects_stale_report_without_manifest() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-stale-list-{}",
            uuid::Uuid::new_v4()
        ));
        let base_dir = root.join(".fozzy");
        let run_dir = base_dir.join("runs").join("stale");
        std::fs::create_dir_all(&run_dir).expect("run dir");
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "status": "pass",
                "mode": "run",
                "identity": {
                    "runId": "stale",
                    "seed": 1,
                    "tracePath": "/tmp/missing-stale-list.trace.fozzy",
                    "reportPath": run_dir.join("report.json"),
                    "artifactsDir": run_dir
                },
                "startedAt": "2026-01-01T00:00:00Z",
                "finishedAt": "2026-01-01T00:00:00Z",
                "durationMs": 0,
                "durationNs": 0,
                "findings": []
            }))
            .expect("report json"),
        )
        .expect("write report");

        let cfg = crate::Config {
            base_dir,
            ..crate::Config::default()
        };
        let err = artifacts_list(&cfg, "stale").expect_err("must reject stale list");
        assert!(err.to_string().contains("missing required files: manifest.json"));
    }

    #[test]
    fn artifacts_list_rejects_trace_only_run_wrapper() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-trace-only-list-{}",
            uuid::Uuid::new_v4()
        ));
        let base_dir = root.join(".fozzy");
        let run_dir = base_dir.join("runs").join("trace-only");
        std::fs::create_dir_all(&run_dir).expect("run dir");
        let trace_path = run_dir.join("trace.fozzy");
        std::fs::write(
            &trace_path,
            valid_trace_json(
                "trace-only",
                &trace_path,
                &run_dir.join("report.json"),
                &run_dir,
            ),
        )
        .expect("write trace");

        let cfg = crate::Config {
            base_dir,
            ..crate::Config::default()
        };
        let err = artifacts_list(&cfg, "trace-only").expect_err("must reject trace-only list");
        assert!(
            err.to_string()
                .contains("missing required files: report.json, manifest.json")
        );
    }

    #[test]
    fn latest_alias_skips_newer_stale_report_only_run() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-latest-stale-{}",
            uuid::Uuid::new_v4()
        ));
        let base_dir = root.join(".fozzy");
        let runs_dir = base_dir.join("runs");
        let healthy_dir = runs_dir.join("healthy");
        std::fs::create_dir_all(&healthy_dir).expect("healthy dir");

        let healthy_trace = healthy_dir.join("trace.fozzy");
        let healthy_report = healthy_dir.join("report.json");
        let (report, manifest) = valid_report_and_manifest_json(
            "healthy",
            &healthy_report,
            &healthy_dir,
            Some(&healthy_trace),
        );
        std::fs::write(
            &healthy_trace,
            valid_trace_json("healthy", &healthy_trace, &healthy_report, &healthy_dir),
        )
        .expect("healthy trace");
        std::fs::write(&healthy_report, report).expect("healthy report");
        std::fs::write(healthy_dir.join("manifest.json"), manifest).expect("healthy manifest");

        std::thread::sleep(std::time::Duration::from_millis(1100));

        let stale_dir = runs_dir.join("stale");
        std::fs::create_dir_all(&stale_dir).expect("stale dir");
        std::fs::write(
            stale_dir.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "status": "pass",
                "mode": "run",
                "identity": {
                    "runId": "stale",
                    "seed": 1,
                    "tracePath": "/tmp/missing-stale.trace.fozzy",
                    "reportPath": stale_dir.join("report.json"),
                    "artifactsDir": stale_dir
                },
                "startedAt": "2026-01-01T00:00:00Z",
                "finishedAt": "2026-01-01T00:00:00Z",
                "durationMs": 0,
                "durationNs": 0,
                "findings": []
            }))
            .expect("stale report json"),
        )
        .expect("stale report");

        let cfg = crate::Config {
            base_dir,
            ..crate::Config::default()
        };
        let resolved = resolve_artifacts_dir(&cfg, "latest").expect("resolve latest");
        assert_eq!(resolved, healthy_dir);
    }

    #[test]
    fn pack_dir_prunes_stale_preexisting_files() {
        let root =
            std::env::temp_dir().join(format!("fozzy-pack-stale-dir-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let trace_path = run_dir.join("trace.fozzy");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let out_dir = root.join("out");
        std::fs::create_dir_all(&out_dir).expect("out");
        std::fs::write(out_dir.join("stale.txt"), b"old").expect("stale");

        export_reproducer_pack(&cfg, "r1", &out_dir).expect("pack should prune stale files");
        assert!(
            !out_dir.join("stale.txt").exists(),
            "stale entry should be removed"
        );
        assert!(
            out_dir.join("manifest.json").exists(),
            "expected artifact should exist"
        );
    }

    #[test]
    fn export_dir_prunes_stale_preexisting_files() {
        let root =
            std::env::temp_dir().join(format!("fozzy-export-stale-dir-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let (report, manifest) = valid_report_and_manifest_json("r1", &report_path, &run_dir, None);
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let out_dir = root.join("out");
        std::fs::create_dir_all(&out_dir).expect("out");
        std::fs::write(out_dir.join("stale.txt"), b"old").expect("stale");

        export_artifacts(&cfg, "r1", &out_dir).expect("export should prune stale files");
        assert!(
            !out_dir.join("stale.txt").exists(),
            "stale entry should be removed"
        );
        assert!(
            out_dir.join("manifest.json").exists(),
            "expected artifact should exist"
        );
    }

    #[test]
    fn pack_and_export_reject_invalid_manifest_bytes() {
        let root =
            std::env::temp_dir().join(format!("fozzy-pack-bad-manifest-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let trace_path = run_dir.join("trace.fozzy");
        std::fs::write(
            &report_path,
            valid_report_json("r1", &report_path, &run_dir),
        )
        .expect("report");
        std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
        std::fs::write(run_dir.join("manifest.json"), br#"not-json"#).expect("manifest");
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let out_pack = root.join("pack.zip");
        let out_export = root.join("export.zip");

        let err_pack = export_reproducer_pack(&cfg, "r1", &out_pack).expect_err("pack must fail");
        assert!(err_pack.to_string().contains("invalid manifest"));
        assert!(!out_pack.exists(), "pack zip should not be created");

        let err_export = export_artifacts(&cfg, "r1", &out_export).expect_err("export must fail");
        assert!(err_export.to_string().contains("invalid manifest"));
        assert!(!out_export.exists(), "export zip should not be created");
    }

    #[test]
    fn pack_and_export_reject_invalid_report_bytes() {
        let root =
            std::env::temp_dir().join(format!("fozzy-pack-bad-report-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let trace_path = run_dir.join("trace.fozzy");
        let (_, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
        std::fs::write(&report_path, br#"not-json"#).expect("report");
        std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let out_pack = root.join("pack.zip");
        let out_export = root.join("export.zip");

        let err_pack = export_reproducer_pack(&cfg, "r1", &out_pack).expect_err("pack must fail");
        assert!(err_pack.to_string().contains("invalid report"));
        assert!(!out_pack.exists(), "pack zip should not be created");

        let err_export = export_artifacts(&cfg, "r1", &out_export).expect_err("export must fail");
        assert!(err_export.to_string().contains("invalid report"));
        assert!(!out_export.exists(), "export zip should not be created");
    }

    #[test]
    fn pack_and_export_reject_manifest_report_identity_mismatch() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-pack-identity-mismatch-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let trace_path = run_dir.join("trace.fozzy");
        let (report, _) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
        let mismatched_manifest = format!(
            r#"{{
  "schemaVersion":"fozzy.run_manifest.v1",
  "runId":"r1",
  "mode":"run",
  "status":"pass",
  "seed":1,
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "reportPath":"{}",
  "artifactsDir":"{}",
  "tracePath":"{}",
  "findingsCount":0
}}"#,
            report_path.display(),
            run_dir.display(),
            root.join("wrong.trace.fozzy").display()
        );
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
        std::fs::write(run_dir.join("manifest.json"), mismatched_manifest).expect("manifest");
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let out_pack = root.join("pack.zip");
        let out_export = root.join("export.zip");

        let err_pack = export_reproducer_pack(&cfg, "r1", &out_pack).expect_err("pack must fail");
        assert!(err_pack.to_string().contains("identity mismatch"));
        assert!(!out_pack.exists(), "pack zip should not be created");

        let err_export = export_artifacts(&cfg, "r1", &out_export).expect_err("export must fail");
        assert!(err_export.to_string().contains("identity mismatch"));
        assert!(!out_export.exists(), "export zip should not be created");
    }

    #[cfg(unix)]
    #[test]
    fn zip_output_rejects_symlinked_parent_components() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "fozzy-pack-symlink-parent-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_path = run_dir.join("report.json");
        let trace_path = run_dir.join("trace.fozzy");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        let real_out_dir = root.join("real-out");
        std::fs::create_dir_all(&real_out_dir).expect("real out");
        let linked_parent = root.join("linked");
        symlink(&real_out_dir, &linked_parent).expect("symlink parent");
        let out_pack = linked_parent.join("pack.zip");
        let out_export = linked_parent.join("export.zip");

        let err_pack =
            export_reproducer_pack(&cfg, "r1", &out_pack).expect_err("must reject symlink parent");
        assert!(err_pack.to_string().contains("symlinked output path"));
        let err_export =
            export_artifacts(&cfg, "r1", &out_export).expect_err("must reject symlink parent");
        assert!(err_export.to_string().contains("symlinked output path"));
    }

    #[test]
    fn bundle_includes_replay_ci_and_env_reports() {
        let root = std::env::temp_dir().join(format!("fozzy-bundle-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace = root.join("trace.fozzy");
        let raw = r#"{
          "format":"fozzy-trace",
          "version":2,
          "engine":{"version":"0.1.0"},
          "mode":"run",
          "scenario_path":null,
          "scenario":{"version":1,"name":"x","steps":[]},
          "decisions":[],
          "events":[],
          "summary":{
            "status":"pass",
            "mode":"run",
            "identity":{"runId":"r1","seed":1},
            "startedAt":"2026-01-01T00:00:00Z",
            "finishedAt":"2026-01-01T00:00:00Z",
            "durationMs":0
          }
        }"#;
        std::fs::write(&trace, raw).expect("write trace");
        let out = root.join("bundle.zip");
        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };

        export_gate_bundle(&cfg, &trace.display().to_string(), &out).expect("bundle");
        let file = std::fs::File::open(&out).expect("zip");
        let mut z = zip::ZipArchive::new(file).expect("zip parse");
        let mut names = Vec::new();
        for i in 0..z.len() {
            names.push(z.by_index(i).expect("entry").name().to_string());
        }
        assert!(names.iter().any(|n| n == "trace.fozzy"));
        assert!(names.iter().any(|n| n == "replay.report.json"));
        assert!(names.iter().any(|n| n == "ci.report.json"));
        assert!(names.iter().any(|n| n == "env.json"));
    }

    #[test]
    fn bundle_rejects_stale_report_without_manifest() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-bundle-stale-report-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let trace_path = run_dir.join("trace.fozzy");
        let report_path = run_dir.join("report.json");

        std::fs::write(
            &trace_path,
            valid_trace_json("r1", &trace_path, &report_path, &run_dir),
        )
        .expect("trace");
        std::fs::write(
            &report_path,
            valid_report_json("r1", &report_path, &run_dir),
        )
        .expect("report");

        let cfg = crate::Config {
            base_dir: root.join(".fozzy"),
            reporter: crate::Reporter::Pretty,
            proc_backend: crate::ProcBackend::Scripted,
            fs_backend: crate::FsBackend::Virtual,
            http_backend: crate::HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        };
        let out = root.join("bundle.zip");

        let err = export_gate_bundle(&cfg, "r1", &out).expect_err("bundle must fail");
        assert!(err.to_string().contains("missing required files: manifest.json"));
    }
}
