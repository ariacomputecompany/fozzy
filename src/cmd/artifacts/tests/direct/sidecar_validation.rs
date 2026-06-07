use super::*;

#[test]
fn artifacts_list_rejects_profile_artifacts_with_mismatched_run_identity() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-artifacts-profile-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let trace_path = run_dir.join("trace.fozzy");
    let report_path = run_dir.join("report.json");
    let (report, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &run_dir),
    )
    .expect("trace");
    std::fs::write(&report_path, report).expect("report");
    std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(
        run_dir.join("profile.metrics.json"),
        valid_profile_metrics_json("foreign-run"),
    )
    .expect("metrics");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: crate::Reporter::Pretty,
        proc_backend: crate::ProcBackend::Scripted,
        fs_backend: crate::FsBackend::Virtual,
        http_backend: crate::HttpBackend::Scripted,
        mem_track: false,
        mem_limit_mb: None,
        mem_fail_after: None,
        fail_on_leak: false,
        leak_budget: None,
        mem_artifacts: false,
        profile_heap_alloc_budget: None,
        profile_heap_in_use_budget: None,
        mem_fragmentation_seed: None,
        mem_pressure_wave: None,
    };

    let err = artifacts_list(&cfg, "r1").expect_err("list must fail");
    assert!(
        err.to_string().contains("belong to runId=foreign-run"),
        "unexpected error: {err}"
    );
}
#[test]
fn direct_trace_rejects_manifest_only_profile_timeline_with_mismatched_identity() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-profile-timeline-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    let report_path = root.join("report.json");
    let (_, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &root, Some(&trace_path));
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &root),
    )
    .expect("trace");
    std::fs::write(root.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(
        root.join("profile.timeline.json"),
        valid_profile_timeline_json("r1", 99),
    )
    .expect("timeline");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: crate::Reporter::Pretty,
        proc_backend: crate::ProcBackend::Scripted,
        fs_backend: crate::FsBackend::Virtual,
        http_backend: crate::HttpBackend::Scripted,
        mem_track: false,
        mem_limit_mb: None,
        mem_fail_after: None,
        fail_on_leak: false,
        leak_budget: None,
        mem_artifacts: false,
        profile_heap_alloc_budget: None,
        profile_heap_in_use_budget: None,
        mem_fragmentation_seed: None,
        mem_pressure_wave: None,
    };

    let err = artifacts_list(&cfg, &trace_path.to_string_lossy()).expect_err("list must fail");
    assert!(
        err.to_string()
            .contains("contains event identity runId=r1 seed=99"),
        "unexpected error: {err}"
    );
}
#[test]
fn artifacts_list_rejects_memory_leaks_with_mismatched_summary() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-artifacts-memory-leaks-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let trace_path = run_dir.join("trace.fozzy");
    let mut trace: crate::TraceFile = serde_json::from_str(&valid_trace_json(
        "r1",
        &trace_path,
        &run_dir.join("report.json"),
        &run_dir,
    ))
    .expect("trace json");
    trace.summary.memory = Some(crate::MemorySummary {
        alloc_count: 1,
        free_count: 1,
        failed_alloc_count: 0,
        in_use_bytes: 0,
        peak_bytes: 128,
        leaked_bytes: 0,
        leaked_allocs: 0,
    });
    trace.write_json(&trace_path).expect("trace");
    std::fs::write(
        run_dir.join("report.json"),
        serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
    )
    .expect("report");
    crate::write_run_manifest(&trace.summary, &run_dir).expect("manifest");
    std::fs::write(
        run_dir.join("memory.leaks.json"),
        valid_memory_leaks_json(128),
    )
    .expect("leaks");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: crate::Reporter::Pretty,
        proc_backend: crate::ProcBackend::Scripted,
        fs_backend: crate::FsBackend::Virtual,
        http_backend: crate::HttpBackend::Scripted,
        mem_track: false,
        mem_limit_mb: None,
        mem_fail_after: None,
        fail_on_leak: false,
        leak_budget: None,
        mem_artifacts: false,
        profile_heap_alloc_budget: None,
        profile_heap_in_use_budget: None,
        mem_fragmentation_seed: None,
        mem_pressure_wave: None,
    };

    let err = artifacts_list(&cfg, "r1").expect_err("list must fail");
    assert!(
        err.to_string()
            .contains("memory.leaks.json do not match summary"),
        "unexpected error: {err}"
    );
}

#[test]
fn direct_trace_rejects_manifest_only_memory_timeline_with_mismatched_summary() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-memory-timeline-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    let report_path = root.join("report.json");
    let mut trace: crate::TraceFile =
        serde_json::from_str(&valid_trace_json("r1", &trace_path, &report_path, &root))
            .expect("trace json");
    trace.summary.memory = Some(crate::MemorySummary {
        alloc_count: 1,
        free_count: 0,
        failed_alloc_count: 0,
        in_use_bytes: 128,
        peak_bytes: 128,
        leaked_bytes: 128,
        leaked_allocs: 1,
    });
    trace.write_json(&trace_path).expect("trace");
    let (_, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &root, Some(&trace_path));
    std::fs::write(root.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(
        root.join("memory.timeline.json"),
        valid_memory_timeline_json(),
    )
    .expect("timeline");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: crate::Reporter::Pretty,
        proc_backend: crate::ProcBackend::Scripted,
        fs_backend: crate::FsBackend::Virtual,
        http_backend: crate::HttpBackend::Scripted,
        mem_track: false,
        mem_limit_mb: None,
        mem_fail_after: None,
        fail_on_leak: false,
        leak_budget: None,
        mem_artifacts: false,
        profile_heap_alloc_budget: None,
        profile_heap_in_use_budget: None,
        mem_fragmentation_seed: None,
        mem_pressure_wave: None,
    };

    let err = artifacts_list(&cfg, &trace_path.to_string_lossy()).expect_err("list must fail");
    assert!(
        err.to_string()
            .contains("memory.timeline.json does not match summary"),
        "unexpected error: {err}"
    );
}

#[test]
fn artifacts_list_rejects_memory_graph_with_mismatched_summary() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-artifacts-memory-graph-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let trace_path = run_dir.join("trace.fozzy");
    let mut trace: crate::TraceFile = serde_json::from_str(&valid_trace_json(
        "r1",
        &trace_path,
        &run_dir.join("report.json"),
        &run_dir,
    ))
    .expect("trace json");
    trace.summary.memory = Some(crate::MemorySummary {
        alloc_count: 1,
        free_count: 0,
        failed_alloc_count: 0,
        in_use_bytes: 128,
        peak_bytes: 128,
        leaked_bytes: 128,
        leaked_allocs: 1,
    });
    trace.write_json(&trace_path).expect("trace");
    std::fs::write(
        run_dir.join("report.json"),
        serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
    )
    .expect("report");
    crate::write_run_manifest(&trace.summary, &run_dir).expect("manifest");
    std::fs::write(run_dir.join("memory.graph.json"), valid_memory_graph_json()).expect("graph");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: crate::Reporter::Pretty,
        proc_backend: crate::ProcBackend::Scripted,
        fs_backend: crate::FsBackend::Virtual,
        http_backend: crate::HttpBackend::Scripted,
        mem_track: false,
        mem_limit_mb: None,
        mem_fail_after: None,
        fail_on_leak: false,
        leak_budget: None,
        mem_artifacts: false,
        profile_heap_alloc_budget: None,
        profile_heap_in_use_budget: None,
        mem_fragmentation_seed: None,
        mem_pressure_wave: None,
    };

    let err = artifacts_list(&cfg, "r1").expect_err("list must fail");
    assert!(
        err.to_string()
            .contains("memory.graph.json does not match summary"),
        "unexpected error: {err}"
    );
}

#[test]
fn direct_trace_rejects_manifest_only_memory_delta_with_mismatched_summary() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-memory-delta-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    let report_path = root.join("report.json");
    let mut trace: crate::TraceFile =
        serde_json::from_str(&valid_trace_json("r1", &trace_path, &report_path, &root))
            .expect("trace json");
    trace.summary.memory = Some(crate::MemorySummary {
        alloc_count: 1,
        free_count: 0,
        failed_alloc_count: 0,
        in_use_bytes: 128,
        peak_bytes: 128,
        leaked_bytes: 128,
        leaked_allocs: 1,
    });
    trace.write_json(&trace_path).expect("trace");
    let (_, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &root, Some(&trace_path));
    std::fs::write(root.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(
        root.join("memory.delta.json"),
        valid_memory_delta_json(0, 0, 0),
    )
    .expect("delta");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: crate::Reporter::Pretty,
        proc_backend: crate::ProcBackend::Scripted,
        fs_backend: crate::FsBackend::Virtual,
        http_backend: crate::HttpBackend::Scripted,
        mem_track: false,
        mem_limit_mb: None,
        mem_fail_after: None,
        fail_on_leak: false,
        leak_budget: None,
        mem_artifacts: false,
        profile_heap_alloc_budget: None,
        profile_heap_in_use_budget: None,
        mem_fragmentation_seed: None,
        mem_pressure_wave: None,
    };

    let err = artifacts_list(&cfg, &trace_path.to_string_lossy()).expect_err("list must fail");
    assert!(
        err.to_string()
            .contains("memory.delta.json does not match summary"),
        "unexpected error: {err}"
    );
}
