use super::*;

#[test]
fn direct_trace_list_ignores_standalone_sibling_traces() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-sibling-traces-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let explicit_trace = root.join("direct.trace.fozzy");
    let sibling_trace = root.join("other.trace.fozzy");
    for (path, run_id) in [(&explicit_trace, "explicit"), (&sibling_trace, "sibling")] {
        crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: crate::RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: None,
            decisions: Vec::new(),
            events: Vec::new(),
            summary: crate::RunSummary {
                status: crate::ExitStatus::Pass,
                mode: crate::RunMode::Run,
                identity: crate::RunIdentity {
                    run_id: run_id.to_string(),
                    seed: 1,
                    trace_path: Some(path.to_string_lossy().to_string()),
                    report_path: None,
                    artifacts_dir: None,
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: None,
                findings: Vec::new(),
            },
            checksum: None,
        }
        .write_json(path)
        .expect("trace");
    }
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

    let list =
        artifacts_list(&cfg, &explicit_trace.to_string_lossy()).expect("list should succeed");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].path, explicit_trace.to_string_lossy());
}

#[test]
fn direct_trace_export_and_pack_reject_unchecked_sibling_artifacts() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-export-unchecked-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    crate::TraceFile {
        format: crate::TRACE_FORMAT.to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: crate::RunMode::Run,
        scenario_path: None,
        scenario: Some(crate::ScenarioV1Steps {
            version: 1,
            name: "x".to_string(),
            steps: Vec::new(),
        }),
        fuzz: None,
        explore: None,
        memory: None,
        decisions: Vec::new(),
        events: Vec::new(),
        summary: crate::RunSummary {
            status: crate::ExitStatus::Pass,
            mode: crate::RunMode::Run,
            identity: crate::RunIdentity {
                run_id: "r1".to_string(),
                seed: 1,
                trace_path: Some(trace_path.to_string_lossy().to_string()),
                report_path: None,
                artifacts_dir: None,
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: None,
            findings: Vec::new(),
        },
        checksum: None,
    }
    .write_json(&trace_path)
    .expect("trace");
    std::fs::write(root.join("profile.metrics.json"), br#"{"domains":[]}"#).expect("profile");
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
    let out_pack = root.join("pack.zip");
    let out_export = root.join("export.zip");

    let err_pack = export_reproducer_pack(&cfg, &trace_path.to_string_lossy(), &out_pack)
        .expect_err("pack must fail");
    assert!(err_pack
        .to_string()
        .contains("report.json and manifest.json are required to trust sibling files"));
    let err_export = export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
        .expect_err("export must fail");
    assert!(err_export
        .to_string()
        .contains("report.json and manifest.json are required to trust sibling files"));
}

#[test]
fn direct_trace_export_and_pack_allow_valid_sibling_metadata() {
    let root =
        std::env::temp_dir().join(format!("fozzy-direct-trace-valid-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    let artifacts_dir = root.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
    let report_path = artifacts_dir.join("report.json");
    let (report, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &artifacts_dir, Some(&trace_path));
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &artifacts_dir),
    )
    .expect("trace");
    std::fs::write(&report_path, report).expect("report");
    std::fs::write(artifacts_dir.join("manifest.json"), manifest).expect("manifest");
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
    let out_pack = root.join("pack.zip");
    let out_export = root.join("export.zip");

    export_reproducer_pack(&cfg, &trace_path.to_string_lossy(), &out_pack)
        .expect("pack should succeed");
    export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
        .expect("export should succeed");
    assert!(out_pack.exists());
    assert!(out_export.exists());
}

#[test]
fn direct_trace_ignores_coherent_foreign_sibling_wrapper() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-foreign-wrapper-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let explicit_trace = root.join("direct.trace.fozzy");
    let sibling_trace = root.join("trace.fozzy");
    let report_path = root.join("report.json");
    let (report, manifest) =
        valid_report_and_manifest_json("sibling-run", &report_path, &root, Some(&sibling_trace));
    std::fs::write(
        &explicit_trace,
        valid_trace_json("explicit-run", &explicit_trace, &report_path, &root),
    )
    .expect("explicit trace");
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
    let out_pack = root.join("pack.zip");
    let out_export = root.join("export.zip");

    let entries = artifacts_list(&cfg, &explicit_trace.to_string_lossy()).expect("list");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, explicit_trace.to_string_lossy());

    export_reproducer_pack(&cfg, &explicit_trace.to_string_lossy(), &out_pack)
        .expect("pack should succeed");
    export_artifacts(&cfg, &explicit_trace.to_string_lossy(), &out_export)
        .expect("export should succeed");
    export_gate_bundle(
        &cfg,
        &explicit_trace.to_string_lossy(),
        &root.join("bundle.zip"),
    )
    .expect("bundle should succeed");
    assert!(out_pack.exists());
    assert!(out_export.exists());
}
