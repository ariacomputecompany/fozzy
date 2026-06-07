use super::*;

#[test]
fn memory_run_id_rejects_trace_only_wrapper_without_report_manifest() {
    let root =
        std::env::temp_dir().join(format!("fozzy-memory-trace-only-{}", uuid::Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let trace_path = run_dir.join("trace.fozzy");

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
                peak_bytes: 24,
                ..MemorySummary::default()
            },
            leaks: vec![MemoryLeak {
                alloc_id: 3,
                bytes: 24,
                callsite_hash: "alloc:trace-only".to_string(),
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

    let cfg = Config {
        base_dir: root.join(".fozzy"),
        ..Config::default()
    };
    let err = load_memory_bundle(&cfg, "r1").expect_err("must reject trace-only wrapper");
    assert!(
        err.to_string()
            .contains("no coherent report/manifest pair found for memory trace artifacts")
            || err
                .to_string()
                .contains("missing required files: report.json, manifest.json")
            || err.to_string().contains("no memory data found")
    );
}
