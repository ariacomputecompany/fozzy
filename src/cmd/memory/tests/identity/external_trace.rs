use super::*;

#[test]
fn run_id_uses_report_declared_external_trace_path_for_memory_bundle() {
    let root = std::env::temp_dir().join(format!("fozzy-memory-runid-{}", uuid::Uuid::new_v4()));
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
