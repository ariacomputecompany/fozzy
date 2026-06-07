//! CLI report commands (`fozzy report ...`).

use clap::Subcommand;
use serde::{Deserialize, Serialize};

#[cfg(test)]
use crate::TraceFile;
use crate::{
    Config, FlakeBudget, FozzyError, FozzyResult, Reporter, RunSummary, render_html,
    render_junit_xml,
};

#[path = "report/flaky.rs"]
mod flaky;
#[path = "report/query.rs"]
mod query;

use flaky::flaky_command;
use query::{list_query_paths, query_value};

#[derive(Debug, Subcommand)]
pub enum ReportCommand {
    Show {
        run: String,
        #[arg(long, default_value = "pretty")]
        format: Reporter,
    },
    Query {
        run: String,
        #[arg(long = "path")]
        path_expr: Option<String>,
        #[arg(long, default_value_t = false)]
        list_paths: bool,
    },
    Flaky {
        runs: Vec<String>,
        #[arg(long)]
        flake_budget: Option<FlakeBudget>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEnvelope {
    pub format: Reporter,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakyReport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "runCount")]
    pub run_count: usize,
    #[serde(rename = "statusCounts")]
    pub status_counts: std::collections::BTreeMap<String, usize>,
    #[serde(rename = "findingTitleSets")]
    pub finding_title_sets: Vec<Vec<String>>,
    #[serde(rename = "isFlaky")]
    pub is_flaky: bool,
    #[serde(rename = "flakeRatePct")]
    pub flake_rate_pct: f64,
}

pub fn report_command(config: &Config, command: &ReportCommand) -> FozzyResult<serde_json::Value> {
    match command {
        ReportCommand::Show { run, format } => {
            let summary = load_summary(config, run)?;
            let doc = report_doc(&summary);
            match format {
                Reporter::Json => Ok(doc),
                Reporter::Pretty => Ok(serde_json::to_value(ReportEnvelope {
                    format: *format,
                    content: summary.pretty(),
                })?),
                Reporter::Junit => Ok(serde_json::to_value(ReportEnvelope {
                    format: *format,
                    content: render_junit_xml(&summary),
                })?),
                Reporter::Html => Ok(serde_json::to_value(ReportEnvelope {
                    format: *format,
                    content: render_html(&summary),
                })?),
            }
        }
        ReportCommand::Query {
            run,
            path_expr,
            list_paths,
        } => {
            let summary = load_summary(config, run)?;
            let value = report_doc(&summary);
            if *list_paths {
                return Ok(serde_json::json!({
                    "paths": list_query_paths(&value)
                }));
            }
            let expr = path_expr.as_deref().ok_or_else(|| {
                FozzyError::Report(
                    "missing --path expression (or pass --list-paths to inspect available paths)"
                        .to_string(),
                )
            })?;
            query_value(&value, expr)
        }
        ReportCommand::Flaky { runs, flake_budget } => flaky_command(config, runs, *flake_budget),
    }
}

fn report_doc(summary: &RunSummary) -> serde_json::Value {
    serde_json::to_value(summary).unwrap_or_else(|_| serde_json::json!({}))
}

fn load_summary(config: &Config, run: &str) -> FozzyResult<RunSummary> {
    if let Some(view) = crate::resolve_artifact_selector_view(config, run)? {
        return Ok(match view {
            crate::ArtifactSelectorView::DirectTrace { trace, .. } => trace.summary,
            crate::ArtifactSelectorView::ValidatedBundle(bundle) => bundle.summary,
        });
    }

    let artifacts_dir = crate::resolve_artifacts_dir(config, run)?;

    Err(FozzyError::Report(format!(
        "no report found for {run:?} (looked for {} and trace resolved from artifacts identity)",
        artifacts_dir.join("report.json").display()
    )))
}

#[cfg(test)]
#[path = "report/tests.rs"]
mod tests;
