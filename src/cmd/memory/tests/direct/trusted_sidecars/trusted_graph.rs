use super::*;

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
    assert!(bundle
        .graph
        .nodes
        .iter()
        .any(|node| node.id == "alloc:exact"));
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
