use super::*;

#[test]
fn latest_memory_alias_skips_newer_trace_without_memory_data() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-memory-latest-nonmemory-{}",
        uuid::Uuid::new_v4()
    ));
    let runs_dir = root.join(".fozzy").join("runs");
    let older_dir = runs_dir.join("older");
    let newer_dir = runs_dir.join("newer");
    std::fs::create_dir_all(&older_dir).expect("older dir");
    std::fs::create_dir_all(&newer_dir).expect("newer dir");

    let older_trace_path = root.join("older.trace.fozzy");
    let older_summary = RunSummary {
        status: ExitStatus::Pass,
        mode: RunMode::Run,
        identity: RunIdentity {
            run_id: "older".to_string(),
            seed: 1,
            trace_path: Some(older_trace_path.to_string_lossy().to_string()),
            report_path: Some(older_dir.join("report.json").to_string_lossy().to_string()),
            artifacts_dir: Some(older_dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 0,
        duration_ns: 0,
        tests: None,
        memory: Some(MemorySummary {
            leaked_bytes: 24,
            leaked_allocs: 1,
            peak_bytes: 24,
            ..MemorySummary::default()
        }),
        findings: Vec::new(),
    };
    let older_trace = crate::TraceFile {
        format: crate::TRACE_FORMAT.to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Run,
        scenario_path: None,
        scenario: Some(crate::ScenarioV1Steps {
            version: 1,
            name: "older".to_string(),
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
                alloc_id: 17,
                bytes: 24,
                callsite_hash: "alloc:older".to_string(),
                tag: None,
            }],
            graph: MemoryGraph::default(),
        }),
        decisions: Vec::new(),
        events: Vec::new(),
        summary: older_summary.clone(),
        checksum: None,
    };
    older_trace
        .write_json(&older_trace_path)
        .expect("write older trace");
    std::fs::write(
        older_dir.join("report.json"),
        serde_json::to_vec_pretty(&older_summary).expect("older report"),
    )
    .expect("write older report");
    crate::write_run_manifest(&older_summary, &older_dir).expect("older manifest");

    std::thread::sleep(std::time::Duration::from_millis(1100));

    let newer_trace_path = root.join("newer.trace.fozzy");
    let newer_summary = RunSummary {
        status: ExitStatus::Pass,
        mode: RunMode::Run,
        identity: RunIdentity {
            run_id: "newer".to_string(),
            seed: 1,
            trace_path: Some(newer_trace_path.to_string_lossy().to_string()),
            report_path: Some(newer_dir.join("report.json").to_string_lossy().to_string()),
            artifacts_dir: Some(newer_dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 0,
        duration_ns: 0,
        tests: None,
        memory: None,
        findings: Vec::new(),
    };
    let newer_trace = crate::TraceFile {
        format: crate::TRACE_FORMAT.to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Run,
        scenario_path: None,
        scenario: Some(crate::ScenarioV1Steps {
            version: 1,
            name: "newer".to_string(),
            steps: Vec::new(),
        }),
        fuzz: None,
        explore: None,
        memory: None,
        decisions: Vec::new(),
        events: Vec::new(),
        summary: newer_summary.clone(),
        checksum: None,
    };
    newer_trace
        .write_json(&newer_trace_path)
        .expect("write newer trace");
    std::fs::write(
        newer_dir.join("report.json"),
        serde_json::to_vec_pretty(&newer_summary).expect("newer report"),
    )
    .expect("write newer report");
    crate::write_run_manifest(&newer_summary, &newer_dir).expect("newer manifest");

    let cfg = Config {
        base_dir: root.join(".fozzy"),
        ..Config::default()
    };
    let bundle = load_memory_bundle(&cfg, "latest").expect("bundle");
    assert_eq!(bundle.summary.leaked_bytes, 24);
    assert_eq!(bundle.leaks.len(), 1);
    assert_eq!(bundle.leaks[0].alloc_id, 17);
}

#[test]
fn latest_memory_alias_skips_newer_timeline_only_wrapper() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-memory-latest-timeline-only-{}",
        uuid::Uuid::new_v4()
    ));
    let runs_dir = root.join(".fozzy").join("runs");
    let older_dir = runs_dir.join("older");
    let newer_dir = runs_dir.join("newer");
    std::fs::create_dir_all(&older_dir).expect("older dir");
    std::fs::create_dir_all(&newer_dir).expect("newer dir");

    let older_trace_path = root.join("older.trace.fozzy");
    let older_summary = RunSummary {
        status: ExitStatus::Pass,
        mode: RunMode::Run,
        identity: RunIdentity {
            run_id: "older".to_string(),
            seed: 1,
            trace_path: Some(older_trace_path.to_string_lossy().to_string()),
            report_path: Some(older_dir.join("report.json").to_string_lossy().to_string()),
            artifacts_dir: Some(older_dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 0,
        duration_ns: 0,
        tests: None,
        memory: Some(MemorySummary {
            leaked_bytes: 24,
            leaked_allocs: 1,
            peak_bytes: 24,
            ..MemorySummary::default()
        }),
        findings: Vec::new(),
    };
    let older_trace = crate::TraceFile {
        format: crate::TRACE_FORMAT.to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Run,
        scenario_path: None,
        scenario: Some(crate::ScenarioV1Steps {
            version: 1,
            name: "older".to_string(),
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
                alloc_id: 23,
                bytes: 24,
                callsite_hash: "alloc:older-timeline".to_string(),
                tag: None,
            }],
            graph: MemoryGraph::default(),
        }),
        decisions: Vec::new(),
        events: Vec::new(),
        summary: older_summary.clone(),
        checksum: None,
    };
    older_trace
        .write_json(&older_trace_path)
        .expect("write older trace");
    std::fs::write(
        older_dir.join("report.json"),
        serde_json::to_vec_pretty(&older_summary).expect("older report"),
    )
    .expect("write older report");
    crate::write_run_manifest(&older_summary, &older_dir).expect("older manifest");

    std::thread::sleep(std::time::Duration::from_millis(1100));

    let newer_trace_path = root.join("newer.trace.fozzy");
    let newer_summary = RunSummary {
        status: ExitStatus::Pass,
        mode: RunMode::Run,
        identity: RunIdentity {
            run_id: "newer".to_string(),
            seed: 1,
            trace_path: Some(newer_trace_path.to_string_lossy().to_string()),
            report_path: Some(newer_dir.join("report.json").to_string_lossy().to_string()),
            artifacts_dir: Some(newer_dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 0,
        duration_ns: 0,
        tests: None,
        memory: None,
        findings: Vec::new(),
    };
    let newer_trace = crate::TraceFile {
        format: crate::TRACE_FORMAT.to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Run,
        scenario_path: None,
        scenario: Some(crate::ScenarioV1Steps {
            version: 1,
            name: "newer".to_string(),
            steps: Vec::new(),
        }),
        fuzz: None,
        explore: None,
        memory: None,
        decisions: Vec::new(),
        events: Vec::new(),
        summary: newer_summary.clone(),
        checksum: None,
    };
    newer_trace
        .write_json(&newer_trace_path)
        .expect("write newer trace");
    std::fs::write(
        newer_dir.join("report.json"),
        serde_json::to_vec_pretty(&newer_summary).expect("newer report"),
    )
    .expect("write newer report");
    crate::write_run_manifest(&newer_summary, &newer_dir).expect("newer manifest");
    std::fs::write(newer_dir.join("memory.timeline.json"), b"[]").expect("timeline");

    let cfg = Config {
        base_dir: root.join(".fozzy"),
        ..Config::default()
    };
    let bundle = load_memory_bundle(&cfg, "latest").expect("bundle");
    assert_eq!(bundle.summary.leaked_bytes, 24);
    assert_eq!(bundle.leaks.len(), 1);
    assert_eq!(bundle.leaks[0].alloc_id, 23);
}
