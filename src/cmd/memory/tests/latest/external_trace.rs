use super::*;

#[test]
fn latest_alias_uses_report_declared_external_trace_path_for_memory_bundle() {
    let root = std::env::temp_dir().join(format!("fozzy-memory-latest-{}", uuid::Uuid::new_v4()));
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
