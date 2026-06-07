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

#[derive(Debug, Clone)]
struct ResolvedMemoryArtifacts {
    artifacts_dir: PathBuf,
    validated_bundle: Option<crate::ValidatedArtifactBundle>,
}

#[derive(Debug, Clone)]
enum ResolvedMemorySource {
    DirectTrace(PathBuf),
    Artifacts(ResolvedMemoryArtifacts),
}

fn validate_leaks_against_summary(
    summary: &MemorySummary,
    leaks: &[MemoryLeak],
    source: &Path,
) -> FozzyResult<()> {
    let leaked_allocs = leaks.len() as u64;
    let leaked_bytes: u64 = leaks.iter().map(|leak| leak.bytes).sum();
    if leaked_allocs != summary.leaked_allocs || leaked_bytes != summary.leaked_bytes {
        return Err(FozzyError::InvalidArgument(format!(
            "memory leak sidecar {} does not match summary: summary leaked_bytes={} leaked_allocs={}, sidecar leaked_bytes={} leaked_allocs={}",
            source.display(),
            summary.leaked_bytes,
            summary.leaked_allocs,
            leaked_bytes,
            leaked_allocs
        )));
    }
    Ok(())
}

fn validate_graph_against_summary(
    summary: &MemorySummary,
    graph: &MemoryGraph,
    source: &Path,
) -> FozzyResult<()> {
    let alloc_nodes = graph
        .nodes
        .iter()
        .filter(|node| node.kind == "alloc")
        .count() as u64;
    let free_nodes = graph
        .nodes
        .iter()
        .filter(|node| node.kind == "free")
        .count() as u64;
    let allocates_edges = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == "allocates")
        .count() as u64;
    let freed_by_edges = graph
        .edges
        .iter()
        .filter(|edge| edge.kind == "freed_by")
        .count() as u64;
    let successful_allocs = summary.free_count.saturating_add(summary.leaked_allocs);
    if alloc_nodes != successful_allocs
        || free_nodes != summary.free_count
        || allocates_edges != successful_allocs
        || freed_by_edges != summary.free_count
    {
        return Err(FozzyError::InvalidArgument(format!(
            "memory graph sidecar {} does not match summary: summary successful_allocs={} free_count={} leaked_allocs={}, graph alloc_nodes={} free_nodes={} allocates_edges={} freed_by_edges={}",
            source.display(),
            successful_allocs,
            summary.free_count,
            summary.leaked_allocs,
            alloc_nodes,
            free_nodes,
            allocates_edges,
            freed_by_edges
        )));
    }
    Ok(())
}

fn load_memory_bundle_from_sidecars(
    source: &ResolvedMemoryArtifacts,
    run: &str,
) -> FozzyResult<Option<MemoryBundle>> {
    let artifacts_dir = &source.artifacts_dir;
    let leaks_path = artifacts_dir.join("memory.leaks.json");
    let graph_path = artifacts_dir.join("memory.graph.json");
    if !leaks_path.exists() && !graph_path.exists() {
        return Ok(None);
    }

    let summary = load_summary_from_validated_artifacts(source)?;
    let mut leaks: Vec<MemoryLeak> = if leaks_path.exists() {
        serde_json::from_slice(&std::fs::read(&leaks_path)?)?
    } else {
        Vec::new()
    };
    if leaks_path.exists() {
        validate_leaks_against_summary(&summary, &leaks, &leaks_path)?;
    }

    let mut graph: MemoryGraph = if graph_path.exists() {
        serde_json::from_slice(&std::fs::read(&graph_path)?)?
    } else {
        MemoryGraph::default()
    };
    if graph_path.exists() {
        validate_graph_against_summary(&summary, &graph, &graph_path)?;
    }

    let missing_leaks = !leaks_path.exists();
    let missing_graph = !graph_path.exists();
    let mut hydrated_missing_memory = false;
    if (missing_leaks || missing_graph)
        && let Some(trace_path) = source
            .validated_bundle
            .as_ref()
            .and_then(|bundle| bundle.trace_path.as_ref())
    {
        let trace_bundle = load_from_trace(trace_path, run)?;
        if missing_leaks {
            leaks = trace_bundle.leaks;
        }
        if missing_graph {
            graph = trace_bundle.graph;
        }
        hydrated_missing_memory = true;
    }

    if (missing_leaks || missing_graph) && !hydrated_missing_memory {
        return Err(FozzyError::InvalidArgument(format!(
            "partial memory sidecars in {} require a trusted trace artifact to supply missing memory evidence",
            artifacts_dir.display()
        )));
    }

    Ok(Some(MemoryBundle {
        summary,
        leaks,
        graph,
    }))
}

pub fn memory_command(config: &Config, command: &MemoryCommand) -> FozzyResult<serde_json::Value> {
    match command {
        MemoryCommand::Graph { run, out } => {
            let run_label = crate::normalize_run_or_trace_selector(run);
            let bundle = load_memory_bundle(config, run)?;
            let payload = MemoryGraphOutput {
                run: run_label,
                graph: bundle.graph,
            };
            if let Some(out_path) = out {
                write_json(out_path, &payload)?;
            }
            Ok(serde_json::to_value(payload)?)
        }
        MemoryCommand::Diff { left, right } => {
            let left_label = crate::normalize_run_or_trace_selector(left);
            let right_label = crate::normalize_run_or_trace_selector(right);
            let l = load_memory_bundle(config, left)?;
            let r = load_memory_bundle(config, right)?;
            let out = MemoryDiff {
                left: left_label,
                right: right_label,
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
            let run_label = crate::normalize_run_or_trace_selector(run);
            let mut bundle = load_memory_bundle(config, run)?;
            bundle.leaks.sort_by(|a, b| {
                b.bytes
                    .cmp(&a.bytes)
                    .then_with(|| a.alloc_id.cmp(&b.alloc_id))
            });
            let out = MemoryTop {
                run: run_label,
                limit: *limit,
                total: bundle.leaks.len(),
                leaks: bundle.leaks.into_iter().take(*limit).collect(),
            };
            Ok(serde_json::to_value(out)?)
        }
    }
}

fn load_memory_bundle(config: &Config, run: &str) -> FozzyResult<MemoryBundle> {
    match resolve_memory_source(config, run)? {
        ResolvedMemorySource::DirectTrace(trace_path) => load_from_trace(&trace_path, run),
        ResolvedMemorySource::Artifacts(source) => {
            if let Some(bundle) = load_memory_bundle_from_sidecars(&source, run)? {
                return Ok(bundle);
            }
            if let Some(trace_path) = source
                .validated_bundle
                .as_ref()
                .and_then(|bundle| bundle.trace_path.as_ref())
            {
                return load_from_trace(trace_path, run);
            }
            Err(FozzyError::InvalidArgument(format!(
                "no memory data found for {run:?}"
            )))
        }
    }
}

fn resolve_memory_source(config: &Config, run: &str) -> FozzyResult<ResolvedMemorySource> {
    let input = PathBuf::from(crate::normalize_run_or_trace_selector(run));
    if input.exists() && input.is_file() && crate::is_trace_path(&input) {
        return Ok(ResolvedMemorySource::DirectTrace(input));
    }

    let artifacts_dir = if let Some(dir) =
        crate::resolve_filtered_run_alias(config, run, |dir, summary| {
            summary.memory.is_some()
                || dir.join("memory.leaks.json").exists()
                || dir.join("memory.graph.json").exists()
                || crate::resolve_trace_path_from_artifacts_dir(dir)
                    .ok()
                    .flatten()
                    .is_some_and(|trace_path| {
                        crate::read_cached_trace_file(&trace_path)
                            .map(|trace| trace.memory.is_some() || trace.summary.memory.is_some())
                            .unwrap_or(false)
                    })
        })? {
        dir
    } else {
        crate::resolve_artifacts_dir(config, run)?
    };
    let validated_bundle = crate::load_validated_artifact_bundle_from_dir(&artifacts_dir, run)?;
    Ok(ResolvedMemorySource::Artifacts(ResolvedMemoryArtifacts {
        artifacts_dir,
        validated_bundle,
    }))
}

fn load_summary_from_validated_artifacts(
    source: &ResolvedMemoryArtifacts,
) -> FozzyResult<MemorySummary> {
    let Some(bundle) = source.validated_bundle.as_ref() else {
        return Err(FozzyError::InvalidArgument(format!(
            "no coherent report/manifest pair found for memory artifacts in {}",
            source.artifacts_dir.display()
        )));
    };
    Ok(bundle.summary.memory.clone().unwrap_or_default())
}

fn trusted_explicit_memory_graph_path(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    crate::trusted_sidecar_path_for_trace(trace_path, "memory.graph.json")
}

fn trusted_explicit_memory_leaks_path(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    crate::trusted_sidecar_path_for_trace(trace_path, "memory.leaks.json")
}

fn load_from_trace(path: &Path, run_name: &str) -> FozzyResult<MemoryBundle> {
    let trace = TraceFile::read_json(path)?;
    let trace_memory = trace.memory;
    let summary = trace_memory
        .as_ref()
        .map(|memory| memory.summary.clone())
        .or(trace.summary.memory.clone());
    let Some(summary) = summary else {
        return Err(FozzyError::InvalidArgument(format!(
            "trace {run_name:?} does not contain memory data"
        )));
    };
    let trusted_leaks = trusted_explicit_memory_leaks_path(path)?;
    let trusted_graph = trusted_explicit_memory_graph_path(path)?;
    let leaks = if let Some(leaks_path) = trusted_leaks.as_ref() {
        let leaks: Vec<MemoryLeak> = serde_json::from_slice(&std::fs::read(&leaks_path)?)?;
        validate_leaks_against_summary(&summary, &leaks, &leaks_path)?;
        leaks
    } else {
        Vec::new()
    };
    let graph = if let Some(graph_path) = trusted_graph.as_ref() {
        let graph = crate::read_cached_memory_graph(&graph_path)?;
        crate::validate_memory_graph_structure(&graph, &graph_path)?;
        validate_graph_against_summary(&summary, &graph, &graph_path)?;
        graph
    } else {
        MemoryGraph::default()
    };
    let embedded_leaks = trace_memory
        .as_ref()
        .map(|memory| memory.leaks.clone())
        .unwrap_or_default();
    let embedded_leaks_empty = embedded_leaks.is_empty();
    let embedded_graph = trace_memory.map(|memory| memory.graph).unwrap_or_default();
    let embedded_graph_empty = embedded_graph.is_empty();
    let final_leaks = if embedded_leaks.is_empty() {
        leaks
    } else {
        embedded_leaks
    };
    let final_graph = if embedded_graph.is_empty() {
        graph
    } else {
        embedded_graph
    };

    if trusted_leaks.is_some() != trusted_graph.is_some() {
        if trusted_leaks.is_none() && embedded_leaks_empty {
            validate_leaks_against_summary(&summary, &final_leaks, path)?;
        }
        if trusted_graph.is_none() && embedded_graph_empty {
            validate_graph_against_summary(&summary, &final_graph, path)?;
        }
    }

    Ok(MemoryBundle {
        summary,
        leaks: final_leaks,
        graph: final_graph,
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
#[path = "memory/spec.rs"]
mod tests;
