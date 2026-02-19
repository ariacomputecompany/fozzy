//! Memory artifact/report commands (`fozzy memory ...`).

use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::{Config, FozzyError, FozzyResult, MemoryGraph, MemoryLeak, MemorySummary, TraceFile};

#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    /// Show/export allocation graph for a run or trace
    Graph {
        #[arg(value_name = "RUN_OR_TRACE")]
        run: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Compare memory outcomes between two runs/traces
    Diff {
        #[arg(value_name = "LEFT_RUN_OR_TRACE")]
        left: String,
        #[arg(value_name = "RIGHT_RUN_OR_TRACE")]
        right: String,
    },
    /// Show top leak records by leaked bytes
    Top {
        #[arg(value_name = "RUN_OR_TRACE")]
        run: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDiff {
    pub left: String,
    pub right: String,
    #[serde(rename = "leftLeakedBytes")]
    pub left_leaked_bytes: u64,
    #[serde(rename = "rightLeakedBytes")]
    pub right_leaked_bytes: u64,
    #[serde(rename = "leftLeakedAllocs")]
    pub left_leaked_allocs: u64,
    #[serde(rename = "rightLeakedAllocs")]
    pub right_leaked_allocs: u64,
    #[serde(rename = "leftPeakBytes")]
    pub left_peak_bytes: u64,
    #[serde(rename = "rightPeakBytes")]
    pub right_peak_bytes: u64,
    #[serde(rename = "deltaLeakedBytes")]
    pub delta_leaked_bytes: i64,
    #[serde(rename = "deltaLeakedAllocs")]
    pub delta_leaked_allocs: i64,
    #[serde(rename = "deltaPeakBytes")]
    pub delta_peak_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryTop {
    pub run: String,
    pub limit: usize,
    pub total: usize,
    pub leaks: Vec<MemoryLeak>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryGraphOutput {
    pub run: String,
    pub graph: MemoryGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryBundle {
    summary: MemorySummary,
    leaks: Vec<MemoryLeak>,
    graph: MemoryGraph,
}

pub fn memory_command(config: &Config, command: &MemoryCommand) -> FozzyResult<serde_json::Value> {
    match command {
        MemoryCommand::Graph { run, out } => {
            let bundle = load_memory_bundle(config, run)?;
            let payload = MemoryGraphOutput {
                run: run.clone(),
                graph: bundle.graph,
            };
            if let Some(out_path) = out {
                write_json(out_path, &payload)?;
            }
            Ok(serde_json::to_value(payload)?)
        }
        MemoryCommand::Diff { left, right } => {
            let l = load_memory_bundle(config, left)?;
            let r = load_memory_bundle(config, right)?;
            let out = MemoryDiff {
                left: left.clone(),
                right: right.clone(),
                left_leaked_bytes: l.summary.leaked_bytes,
                right_leaked_bytes: r.summary.leaked_bytes,
                left_leaked_allocs: l.summary.leaked_allocs,
                right_leaked_allocs: r.summary.leaked_allocs,
                left_peak_bytes: l.summary.peak_bytes,
                right_peak_bytes: r.summary.peak_bytes,
                delta_leaked_bytes: r.summary.leaked_bytes as i64 - l.summary.leaked_bytes as i64,
                delta_leaked_allocs: r.summary.leaked_allocs as i64
                    - l.summary.leaked_allocs as i64,
                delta_peak_bytes: r.summary.peak_bytes as i64 - l.summary.peak_bytes as i64,
            };
            Ok(serde_json::to_value(out)?)
        }
        MemoryCommand::Top { run, limit } => {
            let mut bundle = load_memory_bundle(config, run)?;
            bundle.leaks.sort_by(|a, b| {
                b.bytes
                    .cmp(&a.bytes)
                    .then_with(|| a.alloc_id.cmp(&b.alloc_id))
            });
            let out = MemoryTop {
                run: run.clone(),
                limit: *limit,
                total: bundle.leaks.len(),
                leaks: bundle.leaks.into_iter().take(*limit).collect(),
            };
            Ok(serde_json::to_value(out)?)
        }
    }
}

fn load_memory_bundle(config: &Config, run: &str) -> FozzyResult<MemoryBundle> {
    let input = PathBuf::from(run);
    if input.exists()
        && input.is_file()
        && input
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("fozzy"))
    {
        return load_from_trace(&input, run);
    }

    let artifacts_dir = crate::resolve_artifacts_dir(config, run)?;
    let leaks_path = artifacts_dir.join("memory.leaks.json");
    let graph_path = artifacts_dir.join("memory.graph.json");
    if leaks_path.exists() || graph_path.exists() {
        let summary = load_summary_from_report(&artifacts_dir)?;
        let leaks: Vec<MemoryLeak> = if leaks_path.exists() {
            serde_json::from_slice(&std::fs::read(leaks_path)?)?
        } else {
            Vec::new()
        };
        let graph: MemoryGraph = if graph_path.exists() {
            serde_json::from_slice(&std::fs::read(graph_path)?)?
        } else {
            MemoryGraph::default()
        };
        return Ok(MemoryBundle {
            summary,
            leaks,
            graph,
        });
    }

    let trace_path = artifacts_dir.join("trace.fozzy");
    if trace_path.exists() {
        return load_from_trace(&trace_path, run);
    }

    Err(FozzyError::InvalidArgument(format!(
        "no memory data found for {run:?}"
    )))
}

fn load_summary_from_report(artifacts_dir: &Path) -> FozzyResult<MemorySummary> {
    let report_path = artifacts_dir.join("report.json");
    if !report_path.exists() {
        return Ok(MemorySummary::default());
    }
    let summary: crate::RunSummary = serde_json::from_slice(&std::fs::read(report_path)?)?;
    Ok(summary.memory.unwrap_or_default())
}

fn load_from_trace(path: &Path, run_name: &str) -> FozzyResult<MemoryBundle> {
    let trace = TraceFile::read_json(path)?;
    let Some(memory) = trace.memory else {
        return Err(FozzyError::InvalidArgument(format!(
            "trace {run_name:?} does not contain memory data"
        )));
    };
    Ok(MemoryBundle {
        summary: memory.summary,
        leaks: memory.leaks,
        graph: MemoryGraph::default(),
    })
}

fn write_json(path: &Path, value: &impl Serialize) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_sorts_descending_by_bytes() {
        let mut leaks = vec![
            MemoryLeak {
                alloc_id: 1,
                bytes: 10,
                callsite_hash: "a".to_string(),
                tag: None,
            },
            MemoryLeak {
                alloc_id: 2,
                bytes: 50,
                callsite_hash: "b".to_string(),
                tag: None,
            },
            MemoryLeak {
                alloc_id: 3,
                bytes: 20,
                callsite_hash: "c".to_string(),
                tag: None,
            },
        ];
        leaks.sort_by(|a, b| b.bytes.cmp(&a.bytes));
        assert_eq!(leaks[0].bytes, 50);
        assert_eq!(leaks[1].bytes, 20);
        assert_eq!(leaks[2].bytes, 10);
    }
}
