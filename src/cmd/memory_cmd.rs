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

    let artifacts_dir = match crate::resolve_artifacts_dir(config, run) {
        Ok(dir) => dir,
        Err(err) => {
            if let Some(dir) = resolve_memory_alias_dir(config, run)? {
                dir
            } else {
                return Err(err);
            }
        }
    };
    let has_manifest = artifacts_dir.join("manifest.json").exists();
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

    if let Some(trace_path) = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)? {
        if has_manifest {
            return load_from_trace(&trace_path, run);
        }
        return Err(FozzyError::InvalidArgument(format!(
            "no coherent report/manifest pair found for memory trace artifacts in {}",
            artifacts_dir.display()
        )));
    }

    if let Some(dir) = resolve_memory_alias_dir(config, run)? {
        let leaks_path = dir.join("memory.leaks.json");
        let graph_path = dir.join("memory.graph.json");
        if leaks_path.exists() || graph_path.exists() {
            let summary = load_summary_from_report(&dir)?;
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
        if let Some(trace_path) = crate::resolve_trace_path_from_artifacts_dir(&dir)? {
            if dir.join("manifest.json").exists() {
                return load_from_trace(&trace_path, run);
            }
            return Err(FozzyError::InvalidArgument(format!(
                "no coherent report/manifest pair found for memory trace artifacts in {}",
                dir.display()
            )));
        }
    }

    Err(FozzyError::InvalidArgument(format!(
        "no memory data found for {run:?}"
    )))
}

fn resolve_memory_alias_dir(config: &Config, run: &str) -> FozzyResult<Option<PathBuf>> {
    let key = run.trim().to_ascii_lowercase();
    if key != "latest" && key != "last-pass" && key != "last-fail" {
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
        let summary = match crate::load_checked_report_summary_from_artifacts_dir(
            &dir,
            &dir.display().to_string(),
        ) {
            Ok(summary) => summary,
            Err(_) => continue,
        };
        let has_resolvable_trace = crate::resolve_trace_path_from_artifacts_dir(&dir)?.is_some();
        let has_memory = summary.as_ref().and_then(|s| s.memory.as_ref()).is_some()
            || dir.join("memory.leaks.json").exists()
            || dir.join("memory.timeline.json").exists()
            || dir.join("memory.graph.json").exists()
            || has_resolvable_trace;
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

fn load_summary_from_report(artifacts_dir: &Path) -> FozzyResult<MemorySummary> {
    let Some(summary) = crate::load_checked_report_summary_from_artifacts_dir(
        artifacts_dir,
        &artifacts_dir.display().to_string(),
    )?
    else {
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
    let Some(summary) = crate::load_checked_report_summary_from_artifacts_dir(
        &artifacts_dir,
        &trace_path.display().to_string(),
    )?
    else {
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

fn load_from_trace(path: &Path, run_name: &str) -> FozzyResult<MemoryBundle> {
    let trace = TraceFile::read_json(path)?;
    let Some(memory) = trace.memory else {
        return Err(FozzyError::InvalidArgument(format!(
            "trace {run_name:?} does not contain memory data"
        )));
    };
    let declared_graph = trusted_declared_memory_graph_path(path)?;
    let graph = if let Some(graph_path) = declared_graph {
        serde_json::from_slice(&std::fs::read(graph_path)?)?
    } else {
        MemoryGraph::default()
    };
    Ok(MemoryBundle {
        summary: memory.summary,
        leaks: memory.leaks,
        graph: if memory.graph.is_empty() {
            graph
        } else {
            memory.graph
        },
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
                summary: MemorySummary::default(),
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
                    artifacts_dir: Some(detached.to_string_lossy().to_string()),
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
        crate::write_run_manifest(&trace.summary, &detached).expect("write manifest");
        std::fs::write(
            detached.join("memory.graph.json"),
            serde_json::to_vec_pretty(&MemoryGraph {
                nodes: vec![MemoryGraphNode {
                    id: "alloc:a".to_string(),
                    kind: "alloc".to_string(),
                    label: "a".to_string(),
                }],
                edges: Vec::new(),
            })
            .expect("graph bytes"),
        )
        .expect("write graph");

        let bundle = load_from_trace(&trace_path, &trace_path.to_string_lossy()).expect("bundle");
        assert_eq!(bundle.graph.nodes.len(), 1);
        assert_eq!(bundle.graph.nodes[0].id, "alloc:a");
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
                summary: MemorySummary::default(),
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
                    artifacts_dir: Some(forged.to_string_lossy().to_string()),
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
                summary: MemorySummary::default(),
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
                memory: None,
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
                memory: None,
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
                memory_leaked_bytes: None,
                memory_leaked_allocs: None,
                memory_peak_bytes: None,
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
                memory: None,
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
        );
    }
}
