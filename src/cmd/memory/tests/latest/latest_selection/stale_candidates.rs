#[allow(unused_imports)]
use super::*;

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
            report_path: Some(
                healthy_dir
                    .join("report.json")
                    .to_string_lossy()
                    .to_string(),
            ),
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
                healthy_dir
                    .join("report.json")
                    .to_string_lossy()
                    .to_string(),
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
