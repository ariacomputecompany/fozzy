//! Artifact management (`fozzy artifacts ...`).

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use std::path::PathBuf;

use crate::{Config, FozzyResult};
#[cfg(test)]
#[cfg(test)]
#[allow(unused_imports)]
use crate::{
    load_checked_report_summary_from_artifacts_dir, resolve_artifacts_dir,
    resolve_trace_path_from_artifacts_dir,
};

#[path = "artifacts/diff.rs"]
mod diff;
#[path = "artifacts/export.rs"]
mod export;
#[path = "artifacts/list.rs"]
mod list;
#[path = "artifacts/output.rs"]
mod output;

use diff::artifacts_diff;
#[cfg(test)]
#[allow(unused_imports)]
use diff::load_summary;
use export::{export_artifacts, export_gate_bundle, export_reproducer_pack};
use list::artifacts_list;
#[cfg(test)]
#[allow(unused_imports)]
use list::resolve_trace_path;
#[cfg(test)]
use output::export_artifacts_zip;

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

#[cfg(test)]
#[path = "artifacts/spec.rs"]
mod tests;
