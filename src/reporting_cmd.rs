//! CLI report commands (`fozzy report ...`).

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use std::path::PathBuf;

use crate::{render_html, render_junit_xml, Config, FozzyError, FozzyResult, Reporter, RunSummary, TraceFile};

#[derive(Debug, Subcommand)]
pub enum ReportCommand {
    Show {
        run: String,
        #[arg(long, default_value = "pretty")]
        format: Reporter,
    },
    Query {
        run: String,
        #[arg(long)]
        jq: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEnvelope {
    pub format: Reporter,
    pub content: String,
}

pub fn report_command(config: &Config, command: &ReportCommand) -> FozzyResult<serde_json::Value> {
    match command {
        ReportCommand::Show { run, format } => {
            let summary = load_summary(config, run)?;
            match format {
                Reporter::Json => Ok(serde_json::to_value(summary)?),
                Reporter::Pretty => Ok(serde_json::to_value(ReportEnvelope { format: *format, content: summary.pretty() })?),
                Reporter::Junit => Ok(serde_json::to_value(ReportEnvelope {
                    format: *format,
                    content: render_junit_xml(&summary),
                })?),
                Reporter::Html => Ok(serde_json::to_value(ReportEnvelope { format: *format, content: render_html(&summary) })?),
            }
        }

        ReportCommand::Query { run: _, jq: _ } => Err(FozzyError::Report(
            "report query --jq is not implemented in v0.1 (use `report show --format json` and query externally)".to_string(),
        )),
    }
}

fn load_summary(config: &Config, run: &str) -> FozzyResult<RunSummary> {
    let artifacts_dir = crate::resolve_artifacts_dir(config, run)?;
    let report_json = artifacts_dir.join("report.json");
    if report_json.exists() {
        let bytes = std::fs::read(report_json)?;
        let summary: RunSummary = serde_json::from_slice(&bytes)?;
        return Ok(summary);
    }

    let trace_path = if PathBuf::from(run).exists() {
        PathBuf::from(run)
    } else {
        artifacts_dir.join("trace.fozzy")
    };
    if trace_path.exists() {
        let trace = TraceFile::read_json(&trace_path)?;
        return Ok(trace.summary);
    }

    Err(FozzyError::Report(format!(
        "no report found for {run:?} (looked for {} and {})",
        report_json.display(),
        trace_path.display()
    )))
}
