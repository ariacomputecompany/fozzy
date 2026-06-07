use super::*;

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
