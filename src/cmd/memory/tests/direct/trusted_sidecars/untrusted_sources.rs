#[allow(unused_imports)]
use super::*;

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
