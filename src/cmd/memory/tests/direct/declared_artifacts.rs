use super::*;

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
    let root = std::env::temp_dir().join(format!("fozzy-memory-trace-{}", uuid::Uuid::new_v4()));
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
