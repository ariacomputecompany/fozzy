use super::*;

#[test]
fn artifacts_list_rejects_events_artifact_with_mismatched_trace() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-artifacts-events-mismatch-{}",
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
    trace.events = vec![crate::TraceEvent {
        time_ms: 0,
        name: "real".to_string(),
        fields: serde_json::Map::new(),
    }];
    trace.write_json(&trace_path).expect("trace");
    std::fs::write(
        run_dir.join("report.json"),
        serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
    )
    .expect("report");
    crate::write_run_manifest(&trace.summary, &run_dir).expect("manifest");
    std::fs::write(run_dir.join("events.json"), valid_events_json("forged")).expect("events");

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
            .contains("events.json does not match trace events"),
        "unexpected error: {err}"
    );
}

#[test]
fn direct_trace_rejects_manifest_only_timeline_with_mismatched_trace() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-timeline-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    let report_path = root.join("report.json");
    let mut trace: crate::TraceFile =
        serde_json::from_str(&valid_trace_json("r1", &trace_path, &report_path, &root))
            .expect("trace json");
    trace.events = vec![crate::TraceEvent {
        time_ms: 0,
        name: "real".to_string(),
        fields: serde_json::Map::new(),
    }];
    trace.write_json(&trace_path).expect("trace");
    let (_, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &root, Some(&trace_path));
    std::fs::write(root.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(root.join("timeline.json"), valid_timeline_json("forged")).expect("timeline");

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
            .contains("timeline.json does not match trace events"),
        "unexpected error: {err}"
    );
}

#[test]
fn artifacts_list_rejects_report_html_with_mismatched_summary_rendering() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-report-html-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let trace_path = run_dir.join("trace.fozzy");
    let trace: crate::TraceFile = serde_json::from_str(&valid_trace_json(
        "r1",
        &trace_path,
        &run_dir.join("report.json"),
        &run_dir,
    ))
    .expect("trace json");
    trace.write_json(&trace_path).expect("trace");
    std::fs::write(
        run_dir.join("report.json"),
        serde_json::to_vec_pretty(&trace.summary).expect("report bytes"),
    )
    .expect("report");
    crate::write_run_manifest(&trace.summary, &run_dir).expect("manifest");
    std::fs::write(run_dir.join("report.html"), b"<html>forged</html>\n").expect("html");

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
            .contains("report.html does not match summary rendering"),
        "unexpected error: {err}"
    );
}

#[test]
fn direct_trace_rejects_manifest_only_junit_with_mismatched_summary_rendering() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-junit-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    let report_path = root.join("report.json");
    let trace: crate::TraceFile =
        serde_json::from_str(&valid_trace_json("r1", &trace_path, &report_path, &root))
            .expect("trace json");
    trace.write_json(&trace_path).expect("trace");
    let (_, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &root, Some(&trace_path));
    std::fs::write(root.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(root.join("junit.xml"), b"<testsuite forged=\"true\"/>\n").expect("junit");

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
            .contains("junit.xml does not match summary rendering"),
        "unexpected error: {err}"
    );
}
