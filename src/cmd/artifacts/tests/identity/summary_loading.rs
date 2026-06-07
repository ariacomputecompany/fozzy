use super::*;

#[test]
fn load_summary_prefers_explicit_trace_over_sibling_bundle() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-summary-precedence-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");

    let explicit_trace = root.join("direct.trace.fozzy");
    let sibling_trace = root.join("trace.fozzy");
    let report_path = root.join("report.json");

    std::fs::write(
        &explicit_trace,
        valid_trace_json("explicit-run", &explicit_trace, &report_path, &root),
    )
    .expect("explicit trace");

    let (report, manifest) =
        valid_report_and_manifest_json("sibling-run", &report_path, &root, Some(&sibling_trace));
    std::fs::write(
        &sibling_trace,
        valid_trace_json("sibling-run", &sibling_trace, &report_path, &root),
    )
    .expect("sibling trace");
    std::fs::write(&report_path, report).expect("report");
    std::fs::write(root.join("manifest.json"), manifest).expect("manifest");

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

    let summary = load_summary(&cfg, &explicit_trace.to_string_lossy())
        .expect("load summary")
        .expect("summary");
    assert_eq!(summary.identity.run_id, "explicit-run");
}

#[test]
fn load_summary_uses_manifest_declared_external_trace_when_report_missing() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-artifacts-load-summary-{}",
        uuid::Uuid::new_v4()
    ));
    let base_dir = root.join(".fozzy");
    let run_id = "run-1";
    let run_dir = base_dir.join("runs").join(run_id);
    std::fs::create_dir_all(&run_dir).expect("run dir");

    let mut trace = crate::TraceFile::read_json(&{
        let p = root.join("seed.trace.fozzy");
        std::fs::write(
            &p,
            valid_trace_json("run-1", &p, &run_dir.join("report.json"), &run_dir),
        )
        .expect("seed trace");
        p
    })
    .expect("read seed trace");
    let external_trace = root.join("external.trace.fozzy");
    trace.summary.identity.run_id = run_id.to_string();
    trace.summary.identity.trace_path = Some(external_trace.to_string_lossy().to_string());
    trace.summary.identity.report_path =
        Some(run_dir.join("report.json").to_string_lossy().to_string());
    trace.summary.identity.artifacts_dir = Some(run_dir.to_string_lossy().to_string());
    std::fs::write(
        &external_trace,
        serde_json::to_vec_pretty(&trace).expect("trace bytes"),
    )
    .expect("write trace");
    crate::write_run_manifest(&trace.summary, &run_dir).expect("write manifest");

    let cfg = crate::Config {
        base_dir: base_dir.clone(),
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

    let summary = load_summary(&cfg, run_id).expect("load summary");
    assert_eq!(
        summary
            .as_ref()
            .and_then(|s| s.identity.trace_path.as_deref()),
        Some(external_trace.to_string_lossy().as_ref())
    );
}
