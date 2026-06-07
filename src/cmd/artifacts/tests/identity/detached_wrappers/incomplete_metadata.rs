#[allow(unused_imports)]
use super::*;

#[test]
fn export_and_pack_allow_run_dirs_without_trace_or_events() {
    let root =
        std::env::temp_dir().join(format!("fozzy-pack-minimal-run-{}", uuid::Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let report_path = run_dir.join("report.json");
    let (report, manifest) = valid_report_and_manifest_json("r1", &report_path, &run_dir, None);
    std::fs::write(&report_path, report).expect("report");
    std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
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

    export_reproducer_pack(&cfg, "r1", &out_pack).expect("pack should succeed");
    export_artifacts(&cfg, "r1", &out_export).expect("export should succeed");
    assert!(out_pack.exists(), "pack zip should exist");
    assert!(out_export.exists(), "export zip should exist");
}
#[test]
fn direct_trace_export_and_pack_reject_incomplete_declared_detached_metadata() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-partial-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    let artifacts_dir = root.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
    std::fs::write(
        &trace_path,
        valid_trace_json(
            "r1",
            &trace_path,
            &artifacts_dir.join("report.json"),
            &artifacts_dir,
        ),
    )
    .expect("trace");
    std::fs::write(
        artifacts_dir.join("report.json"),
        valid_report_json("r1", &artifacts_dir.join("report.json"), &artifacts_dir),
    )
    .expect("report");
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
        .contains("missing required files: manifest.json"));
    let err_export = export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
        .expect_err("export must fail");
    assert!(err_export
        .to_string()
        .contains("missing required files: manifest.json"));
}
#[test]
fn direct_trace_list_rejects_incomplete_declared_detached_metadata() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-list-partial-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    let artifacts_dir = root.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
    std::fs::write(
        &trace_path,
        valid_trace_json(
            "r1",
            &trace_path,
            &artifacts_dir.join("report.json"),
            &artifacts_dir,
        ),
    )
    .expect("trace");
    std::fs::write(
        artifacts_dir.join("report.json"),
        valid_report_json("r1", &artifacts_dir.join("report.json"), &artifacts_dir),
    )
    .expect("report");
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
    assert!(err
        .to_string()
        .contains("missing required files: manifest.json"));
}

#[test]
fn direct_trace_list_rejects_unchecked_sibling_artifacts() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-list-unchecked-{}",
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
    std::fs::write(
        root.join("memory.graph.json"),
        br#"{"nodes":[],"edges":[]}"#,
    )
    .expect("graph");
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
    assert!(err
        .to_string()
        .contains("report.json and manifest.json are required to trust sibling files"));
}
