use super::*;

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
    explicit
        .write_json(&explicit_trace)
        .expect("write explicit trace");

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
    sibling
        .write_json(&sibling_trace)
        .expect("write sibling trace");
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
    explicit
        .write_json(&explicit_trace)
        .expect("write explicit trace");

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
    sibling
        .write_json(&sibling_trace)
        .expect("write sibling trace");
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
