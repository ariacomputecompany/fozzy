use super::*;

#[test]
fn latest_alias_skips_newer_stale_report_only_run() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-artifacts-latest-stale-{}",
        uuid::Uuid::new_v4()
    ));
    let base_dir = root.join(".fozzy");
    let runs_dir = base_dir.join("runs");
    let healthy_dir = runs_dir.join("healthy");
    std::fs::create_dir_all(&healthy_dir).expect("healthy dir");

    let healthy_trace = healthy_dir.join("trace.fozzy");
    let healthy_report = healthy_dir.join("report.json");
    let (report, manifest) = valid_report_and_manifest_json(
        "healthy",
        &healthy_report,
        &healthy_dir,
        Some(&healthy_trace),
    );
    std::fs::write(
        &healthy_trace,
        valid_trace_json("healthy", &healthy_trace, &healthy_report, &healthy_dir),
    )
    .expect("healthy trace");
    std::fs::write(&healthy_report, report).expect("healthy report");
    std::fs::write(healthy_dir.join("manifest.json"), manifest).expect("healthy manifest");

    std::thread::sleep(std::time::Duration::from_millis(1100));

    let stale_dir = runs_dir.join("stale");
    std::fs::create_dir_all(&stale_dir).expect("stale dir");
    std::fs::write(
        stale_dir.join("report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "pass",
            "mode": "run",
            "identity": {
                "runId": "stale",
                "seed": 1,
                "tracePath": "/tmp/missing-stale.trace.fozzy",
                "reportPath": stale_dir.join("report.json"),
                "artifactsDir": stale_dir
            },
            "startedAt": "2026-01-01T00:00:00Z",
            "finishedAt": "2026-01-01T00:00:00Z",
            "durationMs": 0,
            "durationNs": 0,
            "findings": []
        }))
        .expect("stale report json"),
    )
    .expect("stale report");

    let cfg = crate::Config {
        base_dir,
        ..crate::Config::default()
    };
    let resolved = resolve_artifacts_dir(&cfg, "latest").expect("resolve latest");
    assert_eq!(resolved, healthy_dir);
}
#[test]
fn export_and_bundle_allow_manifest_only_run_wrapper() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-artifacts-manifest-only-export-{}",
        uuid::Uuid::new_v4()
    ));
    let base_dir = root.join(".fozzy");
    let run_dir = base_dir.join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    let external_trace = root.join("manifest-only-export.trace.fozzy");
    std::fs::write(
        &external_trace,
        valid_trace_json(
            "r1",
            &external_trace,
            &run_dir.join("report.json"),
            &run_dir,
        ),
    )
    .expect("trace");
    let (_, manifest) = valid_report_and_manifest_json(
        "r1",
        &run_dir.join("report.json"),
        &run_dir,
        Some(&external_trace),
    );
    std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");

    let cfg = crate::Config {
        base_dir,
        ..crate::Config::default()
    };
    let out_pack = root.join("pack.zip");
    let out_export = root.join("export.zip");
    let out_bundle = root.join("bundle.zip");

    export_reproducer_pack(&cfg, "r1", &out_pack).expect("pack");
    export_artifacts(&cfg, "r1", &out_export).expect("export");
    export_gate_bundle(&cfg, "r1", &out_bundle).expect("bundle");

    assert!(out_pack.exists(), "pack zip should exist");
    assert!(out_export.exists(), "export zip should exist");
    assert!(out_bundle.exists(), "bundle zip should exist");
}
#[test]
fn latest_alias_accepts_newer_manifest_only_wrapper() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-artifacts-latest-manifest-only-{}",
        uuid::Uuid::new_v4()
    ));
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
    let runs_dir = cfg.runs_dir();
    let older_dir = runs_dir.join("older");
    let newer_dir = runs_dir.join("newer");
    std::fs::create_dir_all(&older_dir).expect("older");
    std::fs::create_dir_all(&newer_dir).expect("newer");

    let older_trace = root.join("older.trace.fozzy");
    std::fs::write(
        &older_trace,
        valid_trace_json(
            "older",
            &older_trace,
            &older_dir.join("report.json"),
            &older_dir,
        ),
    )
    .expect("older trace");
    let (older_report, older_manifest) = valid_report_and_manifest_json(
        "older",
        &older_dir.join("report.json"),
        &older_dir,
        Some(&older_trace),
    );
    std::fs::write(older_dir.join("report.json"), older_report).expect("older report");
    std::fs::write(older_dir.join("manifest.json"), older_manifest).expect("older manifest");

    let newer_trace = root.join("newer.trace.fozzy");
    std::fs::write(
        &newer_trace,
        valid_trace_json(
            "newer",
            &newer_trace,
            &newer_dir.join("report.json"),
            &newer_dir,
        ),
    )
    .expect("newer trace");
    let (_, newer_manifest) = valid_report_and_manifest_json(
        "newer",
        &newer_dir.join("report.json"),
        &newer_dir,
        Some(&newer_trace),
    );
    std::fs::write(newer_dir.join("manifest.json"), newer_manifest).expect("newer manifest");

    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(newer_dir.join("mtime.touch"), b"newer").expect("touch newer");

    let resolved = resolve_artifacts_dir(&cfg, "latest").expect("resolve latest");
    assert_eq!(resolved, newer_dir);
}

#[test]
fn latest_alias_reads_persisted_run_alias_index_without_scanning_history() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-artifacts-latest-index-{}",
        uuid::Uuid::new_v4()
    ));
    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        ..crate::Config::default()
    };
    let run_dir = cfg.runs_dir().join("indexed");
    std::fs::create_dir_all(&run_dir).expect("run dir");
    let trace = run_dir.join("trace.fozzy");
    let report = run_dir.join("report.json");
    let (report_json, manifest_json) =
        valid_report_and_manifest_json("indexed", &report, &run_dir, Some(&trace));
    std::fs::write(
        &trace,
        valid_trace_json("indexed", &trace, &report, &run_dir),
    )
    .expect("trace");
    std::fs::write(&report, report_json).expect("report");
    std::fs::write(run_dir.join("manifest.json"), manifest_json).expect("manifest");
    let summary = crate::read_cached_run_summary(&report).expect("summary");

    crate::update_run_alias_index(&summary, &run_dir).expect("write alias index");

    let resolved = resolve_artifacts_dir(&cfg, "latest").expect("resolve latest");
    assert_eq!(resolved, run_dir);
    assert!(
        cfg.run_alias_index_path().exists(),
        "expected persisted alias index to be written"
    );
}
