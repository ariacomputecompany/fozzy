use super::*;
use crate::TraceFile;

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
                report_path: Some(
                    artifacts_dir
                        .join("report.json")
                        .to_string_lossy()
                        .to_string(),
                ),
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
            report_path: Some(
                artifacts_dir
                    .join("report.json")
                    .to_string_lossy()
                    .to_string(),
            ),
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
    assert!(err.to_string().contains("manifest/trace identity mismatch"));
}
