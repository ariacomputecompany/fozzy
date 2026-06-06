//! Memory artifact/report commands (`fozzy memory ...`).

use clap::Subcommand;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::{
    Config, ExitStatus, FozzyError, FozzyResult, MemoryGraph, MemoryLeak, MemorySummary, TraceFile,
};

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
    artifacts_dir: &Path,
    run: &str,
) -> FozzyResult<Option<MemoryBundle>> {
    let leaks_path = artifacts_dir.join("memory.leaks.json");
    let graph_path = artifacts_dir.join("memory.graph.json");
    if !leaks_path.exists() && !graph_path.exists() {
        return Ok(None);
    }

    let summary = load_summary_from_report(artifacts_dir)?;
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
        && let Some(trace_path) = crate::resolve_trace_path_from_artifacts_dir(artifacts_dir)?
    {
        let trace_bundle = load_from_trace(&trace_path, run)?;
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
    let input = PathBuf::from(crate::normalize_run_or_trace_selector(run));
    if input.exists() && input.is_file() && crate::is_trace_path(&input) {
        return load_from_trace(&input, run);
    }

    let artifacts_dir = if is_memory_alias(run) {
        if let Some(dir) = resolve_memory_alias_dir(config, run)? {
            dir
        } else {
            crate::resolve_artifacts_dir(config, run)?
        }
    } else {
        match crate::resolve_artifacts_dir(config, run) {
            Ok(dir) => dir,
            Err(err) => {
                if let Some(dir) = resolve_memory_alias_dir(config, run)? {
                    dir
                } else {
                    return Err(err);
                }
            }
        }
    };
    if let Some(bundle) = load_memory_bundle_from_sidecars(&artifacts_dir, run)? {
        return Ok(bundle);
    }

    if crate::load_checked_manifest_trace_summary_from_artifacts_dir(&artifacts_dir, run)?
        .is_some()
    {
        let trace_path = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)?
            .expect("checked manifest-trace summary resolved trace");
        return load_from_trace(&trace_path, run);
    }

    if let Some(dir) = resolve_memory_alias_dir(config, run)? {
        if let Some(bundle) = load_memory_bundle_from_sidecars(&dir, run)? {
            return Ok(bundle);
        }
        if crate::load_checked_manifest_trace_summary_from_artifacts_dir(&dir, run)?.is_some() {
            let trace_path = crate::resolve_trace_path_from_artifacts_dir(&dir)?
                .expect("checked manifest-trace summary resolved trace");
            return load_from_trace(&trace_path, run);
        }
    }

    Err(FozzyError::InvalidArgument(format!(
        "no memory data found for {run:?}"
    )))
}

fn resolve_memory_alias_dir(config: &Config, run: &str) -> FozzyResult<Option<PathBuf>> {
    let key = run.trim().to_ascii_lowercase();
    if !is_memory_alias(&key) {
        return Ok(None);
    }
    let runs_dir = config.runs_dir();
    if !runs_dir.exists() {
        return Ok(None);
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
        return Ok(None);
    }
    for (dir, _) in run_dirs {
        let summary = match crate::load_checked_run_summary_from_artifacts_dir(
            &dir,
            &dir.display().to_string(),
        ) {
            Ok(summary) => summary,
            Err(_) => continue,
        };
        let has_memory_trace = crate::resolve_trace_path_from_artifacts_dir(&dir)?
            .as_deref()
            .is_some_and(trace_has_memory_data);
        let has_memory = summary.as_ref().and_then(|s| s.memory.as_ref()).is_some()
            || dir.join("memory.leaks.json").exists()
            || dir.join("memory.graph.json").exists()
            || has_memory_trace;
        if !has_memory {
            continue;
        }
        if key == "latest" {
            return Ok(Some(dir));
        }
        let status = summary
            .as_ref()
            .map(|s| s.status)
            .unwrap_or(ExitStatus::Fail);
        if (key == "last-pass" && status == ExitStatus::Pass)
            || (key == "last-fail" && status != ExitStatus::Pass)
        {
            return Ok(Some(dir));
        }
    }
    Ok(None)
}

fn is_memory_alias(run: &str) -> bool {
    let key = run.trim().to_ascii_lowercase();
    key == "latest" || key == "last-pass" || key == "last-fail"
}

fn trace_has_memory_data(path: &Path) -> bool {
    TraceFile::read_json(path)
        .map(|trace| trace.memory.is_some() || trace.summary.memory.is_some())
        .unwrap_or(false)
}

fn load_summary_from_report(artifacts_dir: &Path) -> FozzyResult<MemorySummary> {
    let summary = if let Some(summary) = crate::load_checked_report_summary_from_artifacts_dir(
        artifacts_dir,
        &artifacts_dir.display().to_string(),
    )? {
        summary
    } else if let Some(summary) = crate::load_checked_manifest_trace_summary_from_artifacts_dir(
        artifacts_dir,
        &artifacts_dir.display().to_string(),
    )? {
        summary
    } else {
        return Err(FozzyError::InvalidArgument(format!(
            "no coherent report/manifest pair found for memory artifacts in {}",
            artifacts_dir.display()
        )));
    };
    Ok(summary.memory.unwrap_or_default())
}

fn trusted_declared_memory_graph_path(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    let trace = TraceFile::read_json(trace_path)?;
    let Some(artifacts_dir) = trace
        .summary
        .identity
        .artifacts_dir
        .as_deref()
        .map(PathBuf::from)
        .filter(|dir| dir.exists() && dir.is_dir())
    else {
        return Ok(None);
    };
    let summary = if let Some(summary) = crate::load_checked_report_summary_from_artifacts_dir(
        &artifacts_dir,
        &trace_path.display().to_string(),
    )? {
        summary
    } else if let Some(summary) = crate::load_checked_manifest_trace_summary_from_artifacts_dir(
        &artifacts_dir,
        &trace_path.display().to_string(),
    )? {
        summary
    } else {
        return Ok(None);
    };
    let Some(resolved_trace) = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)? else {
        return Ok(None);
    };
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

    let graph_path = artifacts_dir.join("memory.graph.json");
    if graph_path.exists() {
        Ok(Some(graph_path))
    } else {
        Ok(None)
    }
}

fn trusted_declared_memory_leaks_path(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    let trace = TraceFile::read_json(trace_path)?;
    let Some(artifacts_dir) = trace
        .summary
        .identity
        .artifacts_dir
        .as_deref()
        .map(PathBuf::from)
        .filter(|dir| dir.exists() && dir.is_dir())
    else {
        return Ok(None);
    };
    let summary = if let Some(summary) = crate::load_checked_report_summary_from_artifacts_dir(
        &artifacts_dir,
        &trace_path.display().to_string(),
    )? {
        summary
    } else if let Some(summary) = crate::load_checked_manifest_trace_summary_from_artifacts_dir(
            &artifacts_dir,
            &trace_path.display().to_string(),
        )? {
        summary
    } else {
        return Ok(None);
    };
    let Some(resolved_trace) = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)? else {
        return Ok(None);
    };
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

    let leaks_path = artifacts_dir.join("memory.leaks.json");
    if leaks_path.exists() {
        Ok(Some(leaks_path))
    } else {
        Ok(None)
    }
}

fn trusted_explicit_memory_graph_path(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    if let Some(graph_path) = trusted_declared_memory_graph_path(trace_path)? {
        return Ok(Some(graph_path));
    }

    let Some(parent) = trace_path.parent() else {
        return Ok(None);
    };
    let Some(summary) =
        crate::load_checked_run_summary_from_artifacts_dir(parent, &trace_path.display().to_string())?
    else {
        return Ok(None);
    };
    let Some(resolved_trace) = crate::resolve_trace_path_from_artifacts_dir(parent)? else {
        return Ok(None);
    };
    let expected_trace =
        std::fs::canonicalize(trace_path).unwrap_or_else(|_| trace_path.to_path_buf());
    let actual_trace =
        std::fs::canonicalize(&resolved_trace).unwrap_or_else(|_| resolved_trace.clone());
    if actual_trace != expected_trace {
        return Ok(None);
    }
    let trace = TraceFile::read_json(trace_path)?;
    if summary.identity.run_id != trace.summary.identity.run_id
        || summary.identity.seed != trace.summary.identity.seed
    {
        return Ok(None);
    }

    let graph_path = parent.join("memory.graph.json");
    if graph_path.exists() {
        Ok(Some(graph_path))
    } else {
        Ok(None)
    }
}

fn trusted_explicit_memory_leaks_path(trace_path: &Path) -> FozzyResult<Option<PathBuf>> {
    if let Some(leaks_path) = trusted_declared_memory_leaks_path(trace_path)? {
        return Ok(Some(leaks_path));
    }

    let Some(parent) = trace_path.parent() else {
        return Ok(None);
    };
    let Some(summary) =
        crate::load_checked_run_summary_from_artifacts_dir(parent, &trace_path.display().to_string())?
    else {
        return Ok(None);
    };
    let Some(resolved_trace) = crate::resolve_trace_path_from_artifacts_dir(parent)? else {
        return Ok(None);
    };
    let expected_trace =
        std::fs::canonicalize(trace_path).unwrap_or_else(|_| trace_path.to_path_buf());
    let actual_trace =
        std::fs::canonicalize(&resolved_trace).unwrap_or_else(|_| resolved_trace.clone());
    if actual_trace != expected_trace {
        return Ok(None);
    }
    let trace = TraceFile::read_json(trace_path)?;
    if summary.identity.run_id != trace.summary.identity.run_id
        || summary.identity.seed != trace.summary.identity.seed
    {
        return Ok(None);
    }

    let leaks_path = parent.join("memory.leaks.json");
    if leaks_path.exists() {
        Ok(Some(leaks_path))
    } else {
        Ok(None)
    }
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
        let graph: MemoryGraph = serde_json::from_slice(&std::fs::read(&graph_path)?)?;
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
    let embedded_graph = trace_memory
        .map(|memory| memory.graph)
        .unwrap_or_default();
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
mod tests {
    use super::*;
    use crate::{
        ExitStatus, MemoryGraphNode, MemoryOptions, MemorySummary, RunIdentity, RunMode, RunSummary,
    };

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

    #[test]
    fn memory_diff_from_trace_inputs() {
        let root = std::env::temp_dir().join(format!("fozzy-memory-cmd-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("mkdir");
        let mk_trace = |path: &Path, leaked: u64| {
            let trace = crate::TraceFile {
                format: crate::TRACE_FORMAT.to_string(),
                version: crate::CURRENT_TRACE_VERSION,
                engine: crate::version_info(),
                mode: RunMode::Run,
                scenario_path: None,
                scenario: Some(crate::ScenarioV1Steps {
                    version: 1,
                    name: "x".to_string(),
                    steps: Vec::new(),
                }),
                fuzz: None,
                explore: None,
                memory: Some(crate::MemoryTrace {
                    options: MemoryOptions::default(),
                    summary: MemorySummary {
                        leaked_bytes: leaked,
                        leaked_allocs: if leaked > 0 { 1 } else { 0 },
                        ..MemorySummary::default()
                    },
                    leaks: Vec::new(),
                    graph: MemoryGraph::default(),
                }),
                decisions: Vec::new(),
                events: Vec::new(),
                summary: RunSummary {
                    status: ExitStatus::Pass,
                    mode: RunMode::Run,
                    identity: RunIdentity {
                        run_id: "r1".to_string(),
                        seed: 1,
                        trace_path: None,
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
            };
            trace.write_json(path).expect("write trace");
        };
        let left = root.join("left.fozzy");
        let right = root.join("right.fozzy");
        mk_trace(&left, 10);
        mk_trace(&right, 30);

        let cfg = Config::default();
        let out = memory_command(
            &cfg,
            &MemoryCommand::Diff {
                left: left.display().to_string(),
                right: right.display().to_string(),
            },
        )
        .expect("diff");
        let obj = out.as_object().expect("object");
        assert_eq!(
            obj.get("deltaLeakedBytes").and_then(|v| v.as_i64()),
            Some(20)
        );
    }

    #[test]
    fn direct_trace_uses_summary_memory_when_embedded_memory_block_is_absent() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-summary-only-trace-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("trace.fozzy");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
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
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
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
                memory: Some(MemorySummary {
                    leaked_bytes: 0,
                    leaked_allocs: 0,
                    peak_bytes: 128,
                    alloc_count: 1,
                    free_count: 1,
                    failed_alloc_count: 0,
                    in_use_bytes: 0,
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");

        let bundle = load_from_trace(&trace_path, &trace_path.to_string_lossy()).expect("bundle");
        assert_eq!(bundle.summary.peak_bytes, 128);
        assert!(bundle.leaks.is_empty());
        assert!(bundle.graph.nodes.is_empty());
    }

    #[test]
    fn direct_trace_uses_declared_artifacts_dir_for_memory_graph() {
        let root =
            std::env::temp_dir().join(format!("fozzy-memory-trace-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("mkdir");
        let detached = root.join("trace.memory-artifacts");
        std::fs::create_dir_all(&detached).expect("detached dir");
        let trace_path = root.join("trace.fozzy");
        let report_path = detached.join("report.json");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 41,
                    bytes: 16,
                    callsite_hash: "alloc:embedded".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(detached.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &detached).expect("write manifest");
        std::fs::write(
            detached.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![
                    MemoryGraphNode {
                        id: "alloc:a".to_string(),
                        kind: "alloc".to_string(),
                        label: "a".to_string(),
                    },
                    MemoryGraphNode {
                        id: "callsite:exact".to_string(),
                        kind: "callsite".to_string(),
                        label: "exact".to_string(),
                    },
                ],
                edges: vec![crate::MemoryGraphEdge {
                    from: "callsite:exact".to_string(),
                    to: "alloc:a".to_string(),
                    kind: "allocates".to_string(),
                }],
            })
            .expect("graph bytes"),
        )
        .expect("write graph");

        let bundle = load_from_trace(&trace_path, &trace_path.to_string_lossy()).expect("bundle");
        assert_eq!(bundle.graph.nodes.len(), 2);
        assert!(bundle.graph.nodes.iter().any(|node| node.id == "alloc:a"));
    }

    #[test]
    fn direct_trace_ignores_forged_declared_memory_artifacts_dir() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-forged-declared-artifacts-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let forged = root.join("forged.memory-artifacts");
        std::fs::create_dir_all(&forged).expect("forged dir");
        let trace_path = root.join("trace.fozzy");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 41,
                    bytes: 16,
                    callsite_hash: "alloc:embedded".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: None,
                    artifacts_dir: Some(forged.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            forged.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![MemoryGraphNode {
                    id: "alloc:forged".to_string(),
                    kind: "alloc".to_string(),
                    label: "forged".to_string(),
                }],
                edges: Vec::new(),
            })
            .expect("graph bytes"),
        )
        .expect("write graph");

        let bundle = load_from_trace(&trace_path, &trace_path.to_string_lossy()).expect("bundle");
        assert!(bundle.graph.nodes.is_empty());
        assert!(bundle.graph.edges.is_empty());
    }

    #[test]
    fn direct_trace_ignores_unchecked_sibling_memory_graph() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-unchecked-sibling-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("trace.fozzy");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 41,
                    bytes: 16,
                    callsite_hash: "alloc:embedded".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
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
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            root.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![MemoryGraphNode {
                    id: "alloc:stale".to_string(),
                    kind: "alloc".to_string(),
                    label: "stale".to_string(),
                }],
                edges: Vec::new(),
            })
            .expect("graph bytes"),
        )
        .expect("write graph");

        let bundle = load_from_trace(&trace_path, &trace_path.to_string_lossy()).expect("bundle");
        assert!(bundle.graph.nodes.is_empty());
        assert!(bundle.graph.edges.is_empty());
    }

    #[test]
    fn direct_trace_ignores_unchecked_sibling_memory_leaks() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-unchecked-sibling-leaks-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("trace.fozzy");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
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
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            root.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&vec![MemoryLeak {
                alloc_id: 77,
                bytes: 64,
                callsite_hash: "alloc:stale".to_string(),
                tag: None,
            }])
            .expect("leaks bytes"),
        )
        .expect("write leaks");

        let bundle = load_from_trace(&trace_path, &trace_path.to_string_lossy()).expect("bundle");
        assert!(bundle.leaks.is_empty());
    }

    #[test]
    fn direct_trace_uses_exact_coherent_sibling_memory_graph() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-exact-sibling-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        let report_path = root.join("report.json");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 41,
                    bytes: 16,
                    callsite_hash: "alloc:embedded".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(root.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &root).expect("write manifest");
        std::fs::write(
            root.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![
                    MemoryGraphNode {
                        id: "alloc:exact".to_string(),
                        kind: "alloc".to_string(),
                        label: "exact".to_string(),
                    },
                    MemoryGraphNode {
                        id: "callsite:exact".to_string(),
                        kind: "callsite".to_string(),
                        label: "exact".to_string(),
                    },
                ],
                edges: vec![crate::MemoryGraphEdge {
                    from: "callsite:exact".to_string(),
                    to: "alloc:exact".to_string(),
                    kind: "allocates".to_string(),
                }],
            })
            .expect("graph bytes"),
        )
        .expect("write graph");

        let bundle = load_from_trace(&trace_path, &trace_path.to_string_lossy()).expect("bundle");
        assert_eq!(bundle.graph.nodes.len(), 2);
        assert!(bundle.graph.nodes.iter().any(|node| node.id == "alloc:exact"));
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 41);
    }

    #[test]
    fn direct_trace_rejects_partial_trusted_memory_graph_without_embedded_leaks() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-partial-sibling-graph-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        let report_path = root.join("report.json");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(root.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &root).expect("write manifest");
        std::fs::write(
            root.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![
                    MemoryGraphNode {
                        id: "alloc:exact".to_string(),
                        kind: "alloc".to_string(),
                        label: "exact".to_string(),
                    },
                    MemoryGraphNode {
                        id: "callsite:exact".to_string(),
                        kind: "callsite".to_string(),
                        label: "exact".to_string(),
                    },
                ],
                edges: vec![crate::MemoryGraphEdge {
                    from: "callsite:exact".to_string(),
                    to: "alloc:exact".to_string(),
                    kind: "allocates".to_string(),
                }],
            })
            .expect("graph bytes"),
        )
        .expect("write graph");

        let err = load_from_trace(&trace_path, &trace_path.to_string_lossy())
            .expect_err("must reject partial graph without embedded leaks");
        assert!(
            err.to_string().contains("does not match summary"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn direct_trace_rejects_mismatched_trusted_memory_graph_sidecar() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-mismatched-sibling-graph-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        let report_path = root.join("report.json");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 1,
                    peak_bytes: 32,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(root.to_string_lossy().to_string()),
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
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &root).expect("write manifest");
        std::fs::write(
            root.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![MemoryGraphNode {
                    id: "alloc:1".to_string(),
                    kind: "alloc".to_string(),
                    label: "1".to_string(),
                }],
                edges: Vec::new(),
            })
            .expect("graph bytes"),
        )
        .expect("write graph");

        let err = load_from_trace(&trace_path, &trace_path.to_string_lossy())
            .expect_err("must reject stale graph");
        assert!(
            err.to_string().contains("does not match summary"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn direct_trace_uses_exact_coherent_sibling_memory_leaks() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-exact-sibling-leaks-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        let report_path = root.join("report.json");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 32,
                    leaked_allocs: 1,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph {
                    nodes: vec![
                        MemoryGraphNode {
                            id: "alloc:41".to_string(),
                            kind: "alloc".to_string(),
                            label: "41".to_string(),
                        },
                        MemoryGraphNode {
                            id: "callsite:embedded".to_string(),
                            kind: "callsite".to_string(),
                            label: "embedded".to_string(),
                        },
                    ],
                    edges: vec![crate::MemoryGraphEdge {
                        from: "callsite:embedded".to_string(),
                        to: "alloc:41".to_string(),
                        kind: "allocates".to_string(),
                    }],
                },
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(root.to_string_lossy().to_string()),
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
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &root).expect("write manifest");
        std::fs::write(
            root.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&vec![MemoryLeak {
                alloc_id: 31,
                bytes: 32,
                callsite_hash: "alloc:exact".to_string(),
                tag: None,
            }])
            .expect("leaks bytes"),
        )
        .expect("write leaks");

        let bundle = load_from_trace(&trace_path, &trace_path.to_string_lossy()).expect("bundle");
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 31);
        assert_eq!(bundle.graph.nodes.len(), 2);
        assert!(bundle.graph.nodes.iter().any(|node| node.id == "alloc:41"));
    }

    #[test]
    fn direct_trace_rejects_partial_trusted_memory_leaks_without_embedded_graph() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-partial-sibling-leaks-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        let report_path = root.join("report.json");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 32,
                    leaked_allocs: 1,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(root.to_string_lossy().to_string()),
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
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &root).expect("write manifest");
        std::fs::write(
            root.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&vec![MemoryLeak {
                alloc_id: 31,
                bytes: 32,
                callsite_hash: "alloc:exact".to_string(),
                tag: None,
            }])
            .expect("leaks bytes"),
        )
        .expect("write leaks");

        let err = load_from_trace(&trace_path, &trace_path.to_string_lossy())
            .expect_err("must reject partial leaks without embedded graph");
        assert!(
            err.to_string().contains("does not match summary"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn direct_trace_rejects_mismatched_trusted_memory_leaks_sidecar() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-mismatched-sibling-leaks-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("direct.trace.fozzy");
        let report_path = root.join("report.json");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 32,
                    leaked_allocs: 1,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(root.to_string_lossy().to_string()),
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
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &root).expect("write manifest");
        std::fs::write(
            root.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&vec![MemoryLeak {
                alloc_id: 31,
                bytes: 99,
                callsite_hash: "alloc:stale".to_string(),
                tag: None,
            }])
            .expect("leaks bytes"),
        )
        .expect("write leaks");

        let err = load_from_trace(&trace_path, &trace_path.to_string_lossy())
            .expect_err("must reject stale leaks");
        assert!(
            err.to_string().contains("does not match summary"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn direct_trace_ignores_coherent_foreign_sibling_memory_graph() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-foreign-sibling-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let explicit_trace = root.join("direct.trace.fozzy");
        let sibling_trace = root.join("trace.fozzy");
        let report_path = root.join("report.json");

        let explicit = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "explicit-run".to_string(),
                    seed: 1,
                    trace_path: Some(explicit_trace.to_string_lossy().to_string()),
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(root.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        explicit.write_json(&explicit_trace).expect("write explicit trace");

        let sibling_summary = RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "sibling-run".to_string(),
                seed: 1,
                trace_path: Some(sibling_trace.to_string_lossy().to_string()),
                report_path: Some(report_path.to_string_lossy().to_string()),
                artifacts_dir: Some(root.to_string_lossy().to_string()),
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: None,
            findings: Vec::new(),
        };
        let sibling = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: sibling_summary.clone(),
            checksum: None,
        };
        sibling.write_json(&sibling_trace).expect("write sibling trace");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&sibling_summary).expect("report bytes"),
        )
        .expect("write report");
        crate::write_run_manifest(&sibling_summary, &root).expect("write manifest");
        std::fs::write(
            root.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![MemoryGraphNode {
                    id: "alloc:foreign".to_string(),
                    kind: "alloc".to_string(),
                    label: "foreign".to_string(),
                }],
                edges: Vec::new(),
            })
            .expect("graph bytes"),
        )
        .expect("write graph");

        let bundle =
            load_from_trace(&explicit_trace, &explicit_trace.to_string_lossy()).expect("bundle");
        assert!(bundle.graph.nodes.is_empty());
        assert!(bundle.graph.edges.is_empty());
    }

    #[test]
    fn direct_trace_ignores_coherent_foreign_sibling_memory_leaks() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-foreign-sibling-leaks-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let explicit_trace = root.join("direct.trace.fozzy");
        let sibling_trace = root.join("trace.fozzy");
        let report_path = root.join("report.json");

        let explicit = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "explicit-run".to_string(),
                    seed: 1,
                    trace_path: Some(explicit_trace.to_string_lossy().to_string()),
                    report_path: Some(report_path.to_string_lossy().to_string()),
                    artifacts_dir: Some(root.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        explicit.write_json(&explicit_trace).expect("write explicit trace");

        let sibling_summary = RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "sibling-run".to_string(),
                seed: 1,
                trace_path: Some(sibling_trace.to_string_lossy().to_string()),
                report_path: Some(report_path.to_string_lossy().to_string()),
                artifacts_dir: Some(root.to_string_lossy().to_string()),
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: None,
            findings: Vec::new(),
        };
        let sibling = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: sibling_summary.clone(),
            checksum: None,
        };
        sibling.write_json(&sibling_trace).expect("write sibling trace");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&sibling_summary).expect("report bytes"),
        )
        .expect("write report");
        crate::write_run_manifest(&sibling_summary, &root).expect("write manifest");
        std::fs::write(
            root.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&vec![MemoryLeak {
                alloc_id: 88,
                bytes: 64,
                callsite_hash: "alloc:foreign".to_string(),
                tag: None,
            }])
            .expect("leaks bytes"),
        )
        .expect("write leaks");

        let bundle =
            load_from_trace(&explicit_trace, &explicit_trace.to_string_lossy()).expect("bundle");
        assert!(bundle.leaks.is_empty());
    }

    #[test]
    fn run_id_uses_report_declared_external_trace_path_for_memory_bundle() {
        let root =
            std::env::temp_dir().join(format!("fozzy-memory-runid-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let external_trace = root.join("external.trace.fozzy");
        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 64,
                    leaked_allocs: 1,
                    peak_bytes: 128,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 7,
                    bytes: 64,
                    callsite_hash: "alloc:external".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(external_trace.to_string_lossy().to_string()),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 64,
                    leaked_allocs: 1,
                    peak_bytes: 128,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&external_trace).expect("write trace");
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&trace.summary).expect("report json"),
        )
        .expect("write report");
        std::fs::write(
            run_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&crate::RunManifest {
                schema_version: "fozzy.run_manifest.v1".to_string(),
                run_id: "r1".to_string(),
                mode: RunMode::Run,
                status: ExitStatus::Pass,
                seed: 1,
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                trace_path: Some(external_trace.to_string_lossy().to_string()),
                findings_count: 0,
                tests_passed: None,
                tests_failed: None,
                tests_skipped: None,
                memory_leaked_bytes: Some(64),
                memory_leaked_allocs: Some(1),
                memory_peak_bytes: Some(128),
                profile_capabilities: Vec::new(),
                profile_artifacts: std::collections::BTreeMap::new(),
                profile_schema_versions: std::collections::BTreeMap::new(),
            })
            .expect("manifest json"),
        )
        .expect("write manifest");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let bundle = load_memory_bundle(&cfg, "r1").expect("bundle");
        assert_eq!(bundle.summary.leaked_bytes, 64);
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 7);
    }

    #[test]
    fn run_id_with_only_memory_graph_sidecar_still_uses_trace_leaks() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-runid-graph-only-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let external_trace = root.join("external.trace.fozzy");
        let graph = MemoryGraph {
            nodes: vec![
                MemoryGraphNode {
                    id: "alloc:11".to_string(),
                    kind: "alloc".to_string(),
                    label: "11".to_string(),
                },
                MemoryGraphNode {
                    id: "callsite:graph-only".to_string(),
                    kind: "callsite".to_string(),
                    label: "graph-only".to_string(),
                },
            ],
            edges: vec![crate::MemoryGraphEdge {
                from: "callsite:graph-only".to_string(),
                to: "alloc:11".to_string(),
                kind: "allocates".to_string(),
            }],
        };
        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 11,
                    bytes: 40,
                    callsite_hash: "alloc:graph-only".to_string(),
                    tag: None,
                }],
                graph: graph.clone(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(external_trace.to_string_lossy().to_string()),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&external_trace).expect("write trace");
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&trace.summary).expect("report json"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &run_dir).expect("write manifest");
        std::fs::write(
            run_dir.join("memory.graph.json"),
            serde_json::to_vec_pretty(&graph).expect("graph json"),
        )
        .expect("write graph");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let bundle = load_memory_bundle(&cfg, "r1").expect("bundle");
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 11);
        assert_eq!(bundle.graph.nodes.len(), 2);
    }

    #[test]
    fn run_id_with_only_memory_graph_sidecar_and_no_trace_is_rejected() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-runid-graph-only-no-trace-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let graph = MemoryGraph {
            nodes: vec![
                MemoryGraphNode {
                    id: "alloc:11".to_string(),
                    kind: "alloc".to_string(),
                    label: "11".to_string(),
                },
                MemoryGraphNode {
                    id: "callsite:graph-only".to_string(),
                    kind: "callsite".to_string(),
                    label: "graph-only".to_string(),
                },
            ],
            edges: vec![crate::MemoryGraphEdge {
                from: "callsite:graph-only".to_string(),
                to: "alloc:11".to_string(),
                kind: "allocates".to_string(),
            }],
        };
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: None,
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            })
            .expect("report json"),
        )
        .expect("write report");
        crate::write_run_manifest(
            &RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: None,
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            &run_dir,
        )
        .expect("write manifest");
        std::fs::write(
            run_dir.join("memory.graph.json"),
            serde_json::to_vec_pretty(&graph).expect("graph json"),
        )
        .expect("write graph");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let err = load_memory_bundle(&cfg, "r1").expect_err("must reject trace-less partial sidecar");
        assert!(
            err.to_string().contains("partial memory sidecars"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn run_id_uses_manifest_declared_external_trace_path_for_memory_bundle_without_report() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-runid-manifest-only-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let external_trace = root.join("external.trace.fozzy");
        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 11,
                    bytes: 40,
                    callsite_hash: "alloc:manifest-only-external".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph {
                    nodes: vec![
                        MemoryGraphNode {
                            id: "alloc:11".to_string(),
                            kind: "alloc".to_string(),
                            label: "11".to_string(),
                        },
                        MemoryGraphNode {
                            id: "callsite:manifest-only-external".to_string(),
                            kind: "callsite".to_string(),
                            label: "manifest-only-external".to_string(),
                        },
                    ],
                    edges: vec![crate::MemoryGraphEdge {
                        from: "callsite:manifest-only-external".to_string(),
                        to: "alloc:11".to_string(),
                        kind: "allocates".to_string(),
                    }],
                },
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(external_trace.to_string_lossy().to_string()),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&external_trace).expect("write trace");
        std::fs::write(
            run_dir.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&vec![MemoryLeak {
                alloc_id: 11,
                bytes: 40,
                callsite_hash: "alloc:manifest-only-external".to_string(),
                tag: None,
            }])
            .expect("leaks json"),
        )
        .expect("write leaks");
        std::fs::write(
            run_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&crate::RunManifest {
                schema_version: "fozzy.run_manifest.v1".to_string(),
                run_id: "r1".to_string(),
                mode: RunMode::Run,
                status: ExitStatus::Pass,
                seed: 1,
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                trace_path: Some(external_trace.to_string_lossy().to_string()),
                findings_count: 0,
                tests_passed: None,
                tests_failed: None,
                tests_skipped: None,
                memory_leaked_bytes: Some(40),
                memory_leaked_allocs: Some(1),
                memory_peak_bytes: Some(96),
                profile_capabilities: Vec::new(),
                profile_artifacts: std::collections::BTreeMap::new(),
                profile_schema_versions: std::collections::BTreeMap::new(),
            })
            .expect("manifest json"),
        )
        .expect("write manifest");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let bundle = load_memory_bundle(&cfg, "r1").expect("bundle");
        assert_eq!(bundle.summary.leaked_bytes, 40);
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 11);
    }

    #[test]
    fn run_id_with_only_memory_leaks_sidecar_still_uses_trace_graph() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-runid-leaks-only-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let external_trace = root.join("external.trace.fozzy");
        let graph = MemoryGraph {
            nodes: vec![
                MemoryGraphNode {
                    id: "alloc:11".to_string(),
                    kind: "alloc".to_string(),
                    label: "11".to_string(),
                },
                MemoryGraphNode {
                    id: "callsite:leaks-only".to_string(),
                    kind: "callsite".to_string(),
                    label: "leaks-only".to_string(),
                },
            ],
            edges: vec![crate::MemoryGraphEdge {
                from: "callsite:leaks-only".to_string(),
                to: "alloc:11".to_string(),
                kind: "allocates".to_string(),
            }],
        };
        let leaks = vec![MemoryLeak {
            alloc_id: 11,
            bytes: 40,
            callsite_hash: "alloc:leaks-only".to_string(),
            tag: None,
        }];
        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                },
                leaks: leaks.clone(),
                graph: graph.clone(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(external_trace.to_string_lossy().to_string()),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&external_trace).expect("write trace");
        std::fs::write(
            run_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&crate::RunManifest {
                schema_version: "fozzy.run_manifest.v1".to_string(),
                run_id: "r1".to_string(),
                mode: RunMode::Run,
                status: ExitStatus::Pass,
                seed: 1,
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                trace_path: Some(external_trace.to_string_lossy().to_string()),
                findings_count: 0,
                tests_passed: None,
                tests_failed: None,
                tests_skipped: None,
                memory_leaked_bytes: Some(40),
                memory_leaked_allocs: Some(1),
                memory_peak_bytes: Some(96),
                profile_capabilities: Vec::new(),
                profile_artifacts: std::collections::BTreeMap::new(),
                profile_schema_versions: std::collections::BTreeMap::new(),
            })
            .expect("manifest json"),
        )
        .expect("write manifest");
        std::fs::write(
            run_dir.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&leaks).expect("leaks json"),
        )
        .expect("write leaks");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let bundle = load_memory_bundle(&cfg, "r1").expect("bundle");
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.graph.nodes.len(), 2);
        assert!(bundle.graph.nodes.iter().any(|node| node.id == "alloc:11"));
    }

    #[test]
    fn run_id_with_only_memory_leaks_sidecar_and_no_trace_is_rejected() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-runid-leaks-only-no-trace-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let leaks = vec![MemoryLeak {
            alloc_id: 11,
            bytes: 40,
            callsite_hash: "alloc:leaks-only".to_string(),
            tag: None,
        }];
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: None,
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            })
            .expect("report json"),
        )
        .expect("write report");
        crate::write_run_manifest(
            &RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: None,
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            &run_dir,
        )
        .expect("write manifest");
        std::fs::write(
            run_dir.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&leaks).expect("leaks json"),
        )
        .expect("write leaks");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let err = load_memory_bundle(&cfg, "r1").expect_err("must reject trace-less partial sidecar");
        assert!(
            err.to_string().contains("partial memory sidecars"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn memory_run_id_rejects_mismatched_memory_graph_sidecar() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-runid-graph-mismatch-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let external_trace = root.join("external.trace.fozzy");
        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 1,
                    peak_bytes: 32,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(external_trace.to_string_lossy().to_string()),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 1,
                    peak_bytes: 32,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&external_trace).expect("write trace");
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&trace.summary).expect("report json"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &run_dir).expect("write manifest");
        std::fs::write(
            run_dir.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![MemoryGraphNode {
                    id: "alloc:1".to_string(),
                    kind: "alloc".to_string(),
                    label: "1".to_string(),
                }],
                edges: Vec::new(),
            })
            .expect("graph json"),
        )
        .expect("write graph");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let err = load_memory_bundle(&cfg, "r1").expect_err("must reject stale graph sidecar");
        assert!(
            err.to_string().contains("does not match summary"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn memory_run_id_rejects_mismatched_memory_leaks_sidecar() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-runid-mismatch-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let external_trace = root.join("external.trace.fozzy");
        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(external_trace.to_string_lossy().to_string()),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 40,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&external_trace).expect("write trace");
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&trace.summary).expect("report json"),
        )
        .expect("write report");
        crate::write_run_manifest(&trace.summary, &run_dir).expect("write manifest");
        std::fs::write(
            run_dir.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&vec![MemoryLeak {
                alloc_id: 17,
                bytes: 999,
                callsite_hash: "alloc:stale".to_string(),
                tag: None,
            }])
            .expect("leaks json"),
        )
        .expect("write leaks");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let err = load_memory_bundle(&cfg, "r1").expect_err("must reject stale leaks sidecar");
        assert!(
            err.to_string().contains("does not match summary"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn latest_alias_uses_report_declared_external_trace_path_for_memory_bundle() {
        let root =
            std::env::temp_dir().join(format!("fozzy-memory-latest-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let external_trace = root.join("external.trace.fozzy");
        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 32,
                    leaked_allocs: 1,
                    peak_bytes: 96,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 9,
                    bytes: 32,
                    callsite_hash: "alloc:latest".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(external_trace.to_string_lossy().to_string()),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&external_trace).expect("write trace");
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&trace.summary).expect("report json"),
        )
        .expect("write report");
        std::fs::write(
            run_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&crate::RunManifest {
                schema_version: "fozzy.run_manifest.v1".to_string(),
                run_id: "r1".to_string(),
                mode: RunMode::Run,
                status: ExitStatus::Pass,
                seed: 1,
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                trace_path: Some(external_trace.to_string_lossy().to_string()),
                findings_count: 0,
                tests_passed: None,
                tests_failed: None,
                tests_skipped: None,
                memory_leaked_bytes: Some(16),
                memory_leaked_allocs: Some(1),
                memory_peak_bytes: Some(16),
                profile_capabilities: Vec::new(),
                profile_artifacts: std::collections::BTreeMap::new(),
                profile_schema_versions: std::collections::BTreeMap::new(),
            })
            .expect("manifest json"),
        )
        .expect("write manifest");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let bundle = load_memory_bundle(&cfg, "latest").expect("bundle");
        assert_eq!(bundle.summary.leaked_bytes, 32);
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 9);
    }

    #[test]
    fn latest_alias_uses_manifest_declared_external_trace_path_for_memory_bundle_without_report() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-latest-manifest-only-{}",
            uuid::Uuid::new_v4()
        ));
        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let stale_dir = cfg.runs_dir().join("older");
        let newer_dir = cfg.runs_dir().join("newer");
        std::fs::create_dir_all(&stale_dir).expect("older dir");
        std::fs::create_dir_all(&newer_dir).expect("newer dir");

        std::fs::write(
            stale_dir.join("report.json"),
            serde_json::to_vec_pretty(&RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "older".to_string(),
                    seed: 1,
                    trace_path: Some(root.join("older.trace.fozzy").to_string_lossy().to_string()),
                    report_path: Some(stale_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(stale_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 99,
                    leaked_allocs: 1,
                    peak_bytes: 99,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            })
            .expect("older report json"),
        )
        .expect("write older report");

        let external_trace = root.join("newer.trace.fozzy");
        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 24,
                    leaked_allocs: 1,
                    peak_bytes: 64,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 13,
                    bytes: 24,
                    callsite_hash: "alloc:latest-manifest-only".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph {
                    nodes: vec![
                        MemoryGraphNode {
                            id: "alloc:13".to_string(),
                            kind: "alloc".to_string(),
                            label: "13".to_string(),
                        },
                        MemoryGraphNode {
                            id: "callsite:latest-manifest-only".to_string(),
                            kind: "callsite".to_string(),
                            label: "latest-manifest-only".to_string(),
                        },
                    ],
                    edges: vec![crate::MemoryGraphEdge {
                        from: "callsite:latest-manifest-only".to_string(),
                        to: "alloc:13".to_string(),
                        kind: "allocates".to_string(),
                    }],
                },
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "newer".to_string(),
                    seed: 1,
                    trace_path: Some(external_trace.to_string_lossy().to_string()),
                    report_path: Some(newer_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(newer_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 24,
                    leaked_allocs: 1,
                    peak_bytes: 64,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&external_trace).expect("write trace");
        std::fs::write(
            newer_dir.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&vec![MemoryLeak {
                alloc_id: 13,
                bytes: 24,
                callsite_hash: "alloc:latest-manifest-only".to_string(),
                tag: None,
            }])
            .expect("leaks json"),
        )
        .expect("write leaks");
        std::fs::write(
            newer_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&crate::RunManifest {
                schema_version: "fozzy.run_manifest.v1".to_string(),
                run_id: "newer".to_string(),
                mode: RunMode::Run,
                status: ExitStatus::Pass,
                seed: 1,
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                report_path: Some(newer_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(newer_dir.to_string_lossy().to_string()),
                trace_path: Some(external_trace.to_string_lossy().to_string()),
                findings_count: 0,
                tests_passed: None,
                tests_failed: None,
                tests_skipped: None,
                memory_leaked_bytes: Some(24),
                memory_leaked_allocs: Some(1),
                memory_peak_bytes: Some(64),
                profile_capabilities: Vec::new(),
                profile_artifacts: std::collections::BTreeMap::new(),
                profile_schema_versions: std::collections::BTreeMap::new(),
            })
            .expect("manifest json"),
        )
        .expect("write manifest");

        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(newer_dir.join("mtime.touch"), b"newer").expect("touch newer");

        let bundle = load_memory_bundle(&cfg, "latest").expect("bundle");
        assert_eq!(bundle.summary.leaked_bytes, 24);
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 13);
    }

    #[test]
    fn memory_artifacts_reject_stale_report_without_manifest() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-stale-report-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(
                        root.join("external.trace.fozzy")
                            .to_string_lossy()
                            .to_string(),
                    ),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 16,
                    leaked_allocs: 1,
                    peak_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            })
            .expect("report json"),
        )
        .expect("write report");
        std::fs::write(
            run_dir.join("memory.leaks.json"),
            serde_json::to_vec_pretty(&vec![MemoryLeak {
                alloc_id: 1,
                bytes: 16,
                callsite_hash: "alloc:stale".to_string(),
                tag: None,
            }])
            .expect("leaks json"),
        )
        .expect("write leaks");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let err = load_memory_bundle(&cfg, "r1").expect_err("must reject stale report");
        assert!(
            err.to_string()
                .contains("missing required files: manifest.json")
        );
    }

    #[test]
    fn direct_trace_uses_manifest_only_declared_artifacts_dir_for_memory_graph() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-direct-manifest-only-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let trace_path = root.join("trace.fozzy");
        let artifacts_dir = root.join("artifacts");
        std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 16,
                    bytes: 16,
                    callsite_hash: "alloc:manifest-only".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(artifacts_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");
        std::fs::write(
            artifacts_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&crate::RunManifest {
                schema_version: "fozzy.run_manifest.v1".to_string(),
                run_id: "r1".to_string(),
                mode: RunMode::Run,
                status: ExitStatus::Pass,
                seed: 1,
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                report_path: Some(artifacts_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
                trace_path: Some(trace_path.to_string_lossy().to_string()),
                findings_count: 0,
                tests_passed: None,
                tests_failed: None,
                tests_skipped: None,
                memory_leaked_bytes: Some(16),
                memory_leaked_allocs: Some(1),
                memory_peak_bytes: Some(16),
                profile_capabilities: Vec::new(),
                profile_artifacts: std::collections::BTreeMap::new(),
                profile_schema_versions: std::collections::BTreeMap::new(),
            })
            .expect("manifest json"),
        )
        .expect("write manifest");
        std::fs::write(
            artifacts_dir.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![
                    MemoryGraphNode {
                        id: "alloc:manifest-only".to_string(),
                        kind: "alloc".to_string(),
                        label: "manifest-only".to_string(),
                    },
                    MemoryGraphNode {
                        id: "callsite:manifest-only".to_string(),
                        kind: "callsite".to_string(),
                        label: "manifest-only".to_string(),
                    },
                ],
                edges: vec![crate::MemoryGraphEdge {
                    from: "callsite:manifest-only".to_string(),
                    to: "alloc:manifest-only".to_string(),
                    kind: "allocates".to_string(),
                }],
            })
            .expect("graph bytes"),
        )
        .expect("write graph");

        let bundle = load_from_trace(&trace_path, &trace_path.to_string_lossy()).expect("bundle");
        assert_eq!(bundle.graph.nodes.len(), 2);
        assert!(
            bundle
                .graph
                .nodes
                .iter()
                .any(|node| node.id == "alloc:manifest-only")
        );
    }

    #[test]
    fn memory_run_id_rejects_incoherent_manifest_only_wrapper() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-manifest-only-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let trace_path = run_dir.join("trace.fozzy");

        let trace = TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 8,
                    leaked_allocs: 1,
                    peak_bytes: 8,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 1,
                    bytes: 8,
                    callsite_hash: "alloc:manifest-only".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 8,
                    leaked_allocs: 1,
                    peak_bytes: 8,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");
        let manifest = crate::RunManifest {
            schema_version: "fozzy.run_manifest.v1".to_string(),
            run_id: "r1".to_string(),
            mode: RunMode::Run,
            status: ExitStatus::Pass,
            seed: 99,
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            trace_path: Some(trace_path.to_string_lossy().to_string()),
            report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
            artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
            findings_count: 0,
            tests_passed: None,
            tests_failed: None,
            tests_skipped: None,
            memory_leaked_bytes: Some(8),
            memory_leaked_allocs: Some(1),
            memory_peak_bytes: Some(8),
            profile_capabilities: Vec::new(),
            profile_artifacts: std::collections::BTreeMap::new(),
            profile_schema_versions: std::collections::BTreeMap::new(),
        };
        std::fs::write(
            run_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).expect("manifest json"),
        )
        .expect("write manifest");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let err =
            load_memory_bundle(&cfg, "r1").expect_err("must reject incoherent manifest-only wrapper");
        assert!(
            err.to_string()
                .contains("manifest/trace identity mismatch")
        );
    }

    #[test]
    fn latest_memory_alias_skips_newer_stale_report_only_run() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-latest-stale-{}",
            uuid::Uuid::new_v4()
        ));
        let runs_dir = root.join(".fozzy").join("runs");
        let healthy_dir = runs_dir.join("healthy");
        std::fs::create_dir_all(&healthy_dir).expect("healthy dir");
        let external_trace = root.join("healthy-external.trace.fozzy");
        let healthy_summary = RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "healthy".to_string(),
                seed: 1,
                trace_path: Some(external_trace.to_string_lossy().to_string()),
                report_path: Some(healthy_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(healthy_dir.to_string_lossy().to_string()),
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: None,
            findings: Vec::new(),
        };
        let healthy_trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 24,
                    leaked_allocs: 1,
                    peak_bytes: 24,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 3,
                    bytes: 24,
                    callsite_hash: "alloc:healthy".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: healthy_summary.clone(),
            checksum: None,
        };
        healthy_trace
            .write_json(&external_trace)
            .expect("write healthy trace");
        std::fs::write(
            healthy_dir.join("report.json"),
            serde_json::to_vec_pretty(&healthy_summary).expect("healthy report json"),
        )
        .expect("write healthy report");
        std::fs::write(
            healthy_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&crate::RunManifest {
                schema_version: "fozzy.run_manifest.v1".to_string(),
                run_id: "healthy".to_string(),
                mode: RunMode::Run,
                status: ExitStatus::Pass,
                seed: 1,
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                report_path: Some(
                    healthy_dir.join("report.json").to_string_lossy().to_string(),
                ),
                artifacts_dir: Some(healthy_dir.to_string_lossy().to_string()),
                trace_path: Some(external_trace.to_string_lossy().to_string()),
                findings_count: 0,
                tests_passed: None,
                tests_failed: None,
                tests_skipped: None,
                memory_leaked_bytes: None,
                memory_leaked_allocs: None,
                memory_peak_bytes: None,
                profile_capabilities: Vec::new(),
                profile_artifacts: std::collections::BTreeMap::new(),
                profile_schema_versions: std::collections::BTreeMap::new(),
            })
            .expect("healthy manifest json"),
        )
        .expect("write healthy manifest");

        std::thread::sleep(std::time::Duration::from_millis(1100));

        let stale_dir = runs_dir.join("stale");
        std::fs::create_dir_all(&stale_dir).expect("stale dir");
        std::fs::write(
            stale_dir.join("report.json"),
            serde_json::to_vec_pretty(&RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "stale".to_string(),
                    seed: 1,
                    trace_path: Some("/tmp/missing-stale.trace.fozzy".to_string()),
                    report_path: Some(stale_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(stale_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    leaked_bytes: 999,
                    leaked_allocs: 1,
                    peak_bytes: 999,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            })
            .expect("stale report json"),
        )
        .expect("write stale report");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let bundle = load_memory_bundle(&cfg, "latest").expect("bundle");
        assert_eq!(bundle.summary.leaked_bytes, 24);
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 3);
    }

    #[test]
    fn latest_memory_alias_skips_newer_trace_without_memory_data() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-latest-nonmemory-{}",
            uuid::Uuid::new_v4()
        ));
        let runs_dir = root.join(".fozzy").join("runs");
        let older_dir = runs_dir.join("older");
        let newer_dir = runs_dir.join("newer");
        std::fs::create_dir_all(&older_dir).expect("older dir");
        std::fs::create_dir_all(&newer_dir).expect("newer dir");

        let older_trace_path = root.join("older.trace.fozzy");
        let older_summary = RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "older".to_string(),
                seed: 1,
                trace_path: Some(older_trace_path.to_string_lossy().to_string()),
                report_path: Some(older_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(older_dir.to_string_lossy().to_string()),
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: Some(MemorySummary {
                leaked_bytes: 24,
                leaked_allocs: 1,
                peak_bytes: 24,
                ..MemorySummary::default()
            }),
            findings: Vec::new(),
        };
        let older_trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "older".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 24,
                    leaked_allocs: 1,
                    peak_bytes: 24,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 17,
                    bytes: 24,
                    callsite_hash: "alloc:older".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: older_summary.clone(),
            checksum: None,
        };
        older_trace
            .write_json(&older_trace_path)
            .expect("write older trace");
        std::fs::write(
            older_dir.join("report.json"),
            serde_json::to_vec_pretty(&older_summary).expect("older report"),
        )
        .expect("write older report");
        crate::write_run_manifest(&older_summary, &older_dir).expect("older manifest");

        std::thread::sleep(std::time::Duration::from_millis(1100));

        let newer_trace_path = root.join("newer.trace.fozzy");
        let newer_summary = RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "newer".to_string(),
                seed: 1,
                trace_path: Some(newer_trace_path.to_string_lossy().to_string()),
                report_path: Some(newer_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(newer_dir.to_string_lossy().to_string()),
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: None,
            findings: Vec::new(),
        };
        let newer_trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "newer".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: None,
            decisions: Vec::new(),
            events: Vec::new(),
            summary: newer_summary.clone(),
            checksum: None,
        };
        newer_trace
            .write_json(&newer_trace_path)
            .expect("write newer trace");
        std::fs::write(
            newer_dir.join("report.json"),
            serde_json::to_vec_pretty(&newer_summary).expect("newer report"),
        )
        .expect("write newer report");
        crate::write_run_manifest(&newer_summary, &newer_dir).expect("newer manifest");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let bundle = load_memory_bundle(&cfg, "latest").expect("bundle");
        assert_eq!(bundle.summary.leaked_bytes, 24);
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 17);
    }

    #[test]
    fn latest_memory_alias_skips_newer_timeline_only_wrapper() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-memory-latest-timeline-only-{}",
            uuid::Uuid::new_v4()
        ));
        let runs_dir = root.join(".fozzy").join("runs");
        let older_dir = runs_dir.join("older");
        let newer_dir = runs_dir.join("newer");
        std::fs::create_dir_all(&older_dir).expect("older dir");
        std::fs::create_dir_all(&newer_dir).expect("newer dir");

        let older_trace_path = root.join("older.trace.fozzy");
        let older_summary = RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "older".to_string(),
                seed: 1,
                trace_path: Some(older_trace_path.to_string_lossy().to_string()),
                report_path: Some(older_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(older_dir.to_string_lossy().to_string()),
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: Some(MemorySummary {
                leaked_bytes: 24,
                leaked_allocs: 1,
                peak_bytes: 24,
                ..MemorySummary::default()
            }),
            findings: Vec::new(),
        };
        let older_trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "older".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 24,
                    leaked_allocs: 1,
                    peak_bytes: 24,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 23,
                    bytes: 24,
                    callsite_hash: "alloc:older-timeline".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: older_summary.clone(),
            checksum: None,
        };
        older_trace
            .write_json(&older_trace_path)
            .expect("write older trace");
        std::fs::write(
            older_dir.join("report.json"),
            serde_json::to_vec_pretty(&older_summary).expect("older report"),
        )
        .expect("write older report");
        crate::write_run_manifest(&older_summary, &older_dir).expect("older manifest");

        std::thread::sleep(std::time::Duration::from_millis(1100));

        let newer_trace_path = root.join("newer.trace.fozzy");
        let newer_summary = RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "newer".to_string(),
                seed: 1,
                trace_path: Some(newer_trace_path.to_string_lossy().to_string()),
                report_path: Some(newer_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(newer_dir.to_string_lossy().to_string()),
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: None,
            findings: Vec::new(),
        };
        let newer_trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "newer".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: None,
            decisions: Vec::new(),
            events: Vec::new(),
            summary: newer_summary.clone(),
            checksum: None,
        };
        newer_trace
            .write_json(&newer_trace_path)
            .expect("write newer trace");
        std::fs::write(
            newer_dir.join("report.json"),
            serde_json::to_vec_pretty(&newer_summary).expect("newer report"),
        )
        .expect("write newer report");
        crate::write_run_manifest(&newer_summary, &newer_dir).expect("newer manifest");
        std::fs::write(newer_dir.join("memory.timeline.json"), b"[]").expect("timeline");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let bundle = load_memory_bundle(&cfg, "latest").expect("bundle");
        assert_eq!(bundle.summary.leaked_bytes, 24);
        assert_eq!(bundle.leaks.len(), 1);
        assert_eq!(bundle.leaks[0].alloc_id, 23);
    }

    #[test]
    fn memory_run_id_rejects_trace_only_wrapper_without_report_manifest() {
        let root =
            std::env::temp_dir().join(format!("fozzy-memory-trace-only-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let trace_path = run_dir.join("trace.fozzy");

        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: 24,
                    leaked_allocs: 1,
                    peak_bytes: 24,
                    ..MemorySummary::default()
                },
                leaks: vec![MemoryLeak {
                    alloc_id: 3,
                    bytes: 24,
                    callsite_hash: "alloc:trace-only".to_string(),
                    tag: None,
                }],
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: Some(trace_path.to_string_lossy().to_string()),
                    report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                    artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: Some(MemorySummary {
                    alloc_count: 1,
                    free_count: 0,
                    leaked_allocs: 1,
                    leaked_bytes: 16,
                    peak_bytes: 16,
                    in_use_bytes: 16,
                    ..MemorySummary::default()
                }),
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(&trace_path).expect("write trace");

        let cfg = Config {
            base_dir: root.join(".fozzy"),
            ..Config::default()
        };
        let err = load_memory_bundle(&cfg, "r1").expect_err("must reject trace-only wrapper");
        assert!(
            err.to_string()
                .contains("no coherent report/manifest pair found for memory trace artifacts")
                || err
                    .to_string()
                    .contains("missing required files: report.json, manifest.json")
                || err.to_string().contains("no memory data found")
        );
    }
}
