use super::*;

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
