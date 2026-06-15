use super::*;

#[test]
fn pack_dir_prunes_stale_preexisting_files() {
    let root = std::env::temp_dir().join(format!("fozzy-pack-stale-dir-{}", uuid::Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let report_path = run_dir.join("report.json");
    let trace_path = run_dir.join("trace.fozzy");
    let (report, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
    std::fs::write(&report_path, report).expect("report");
    std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
    std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &run_dir),
    )
    .expect("trace");
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

    let out_dir = root.join("out");
    std::fs::create_dir_all(&out_dir).expect("out");
    std::fs::write(out_dir.join("stale.txt"), b"old").expect("stale");

    export_reproducer_pack(&cfg, "r1", &out_dir).expect("pack should prune stale files");
    assert!(
        !out_dir.join("stale.txt").exists(),
        "stale entry should be removed"
    );
    assert!(
        out_dir.join("manifest.json").exists(),
        "expected artifact should exist"
    );
}

#[test]
fn export_dir_prunes_stale_preexisting_files() {
    let root =
        std::env::temp_dir().join(format!("fozzy-export-stale-dir-{}", uuid::Uuid::new_v4()));
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

    let out_dir = root.join("out");
    std::fs::create_dir_all(&out_dir).expect("out");
    std::fs::write(out_dir.join("stale.txt"), b"old").expect("stale");

    export_artifacts(&cfg, "r1", &out_dir).expect("export should prune stale files");
    assert!(
        !out_dir.join("stale.txt").exists(),
        "stale entry should be removed"
    );
    assert!(
        out_dir.join("manifest.json").exists(),
        "expected artifact should exist"
    );
}

#[test]
fn pack_and_export_reject_invalid_manifest_bytes() {
    let root =
        std::env::temp_dir().join(format!("fozzy-pack-bad-manifest-{}", uuid::Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let report_path = run_dir.join("report.json");
    let trace_path = run_dir.join("trace.fozzy");
    std::fs::write(
        &report_path,
        valid_report_json("r1", &report_path, &run_dir),
    )
    .expect("report");
    std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
    std::fs::write(run_dir.join("manifest.json"), br#"not-json"#).expect("manifest");
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &run_dir),
    )
    .expect("trace");
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

    let err_pack = export_reproducer_pack(&cfg, "r1", &out_pack).expect_err("pack must fail");
    assert!(err_pack.to_string().contains("invalid manifest"));
    assert!(!out_pack.exists(), "pack zip should not be created");

    let err_export = export_artifacts(&cfg, "r1", &out_export).expect_err("export must fail");
    assert!(err_export.to_string().contains("invalid manifest"));
    assert!(!out_export.exists(), "export zip should not be created");
}

#[test]
fn pack_and_export_reject_invalid_report_bytes() {
    let root = std::env::temp_dir().join(format!("fozzy-pack-bad-report-{}", uuid::Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let report_path = run_dir.join("report.json");
    let trace_path = run_dir.join("trace.fozzy");
    let (_, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
    std::fs::write(&report_path, br#"not-json"#).expect("report");
    std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
    std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &run_dir),
    )
    .expect("trace");
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

    let err_pack = export_reproducer_pack(&cfg, "r1", &out_pack).expect_err("pack must fail");
    assert!(err_pack.to_string().contains("invalid report"));
    assert!(!out_pack.exists(), "pack zip should not be created");

    let err_export = export_artifacts(&cfg, "r1", &out_export).expect_err("export must fail");
    assert!(err_export.to_string().contains("invalid report"));
    assert!(!out_export.exists(), "export zip should not be created");
}

#[test]
fn pack_and_export_reject_manifest_report_identity_mismatch() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-pack-identity-mismatch-{}",
        uuid::Uuid::new_v4()
    ));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let report_path = run_dir.join("report.json");
    let trace_path = run_dir.join("trace.fozzy");
    let (report, _) =
        valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
    let mismatched_manifest = format!(
        r#"{{
  "schemaVersion":"fozzy.run_manifest.v1",
  "runId":"r1",
  "mode":"run",
  "status":"pass",
  "seed":1,
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "reportPath":"{}",
  "artifactsDir":"{}",
  "tracePath":"{}",
  "findingsCount":0
}}"#,
        report_path.display(),
        run_dir.display(),
        root.join("wrong.trace.fozzy").display()
    );
    std::fs::write(&report_path, report).expect("report");
    std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
    std::fs::write(run_dir.join("manifest.json"), mismatched_manifest).expect("manifest");
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &run_dir),
    )
    .expect("trace");
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

    let err_pack = export_reproducer_pack(&cfg, "r1", &out_pack).expect_err("pack must fail");
    assert!(err_pack.to_string().contains("identity mismatch"));
    assert!(!out_pack.exists(), "pack zip should not be created");

    let err_export = export_artifacts(&cfg, "r1", &out_export).expect_err("export must fail");
    assert!(err_export.to_string().contains("identity mismatch"));
    assert!(!out_export.exists(), "export zip should not be created");
}

#[cfg(unix)]
#[test]
fn zip_output_rejects_symlinked_parent_components() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "fozzy-pack-symlink-parent-{}",
        uuid::Uuid::new_v4()
    ));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let report_path = run_dir.join("report.json");
    let trace_path = run_dir.join("trace.fozzy");
    let (report, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
    std::fs::write(&report_path, report).expect("report");
    std::fs::write(run_dir.join("events.json"), br#"[]"#).expect("events");
    std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &run_dir),
    )
    .expect("trace");
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

    let real_out_dir = root.join("real-out");
    std::fs::create_dir_all(&real_out_dir).expect("real out");
    let linked_parent = root.join("linked");
    symlink(&real_out_dir, &linked_parent).expect("symlink parent");
    let out_pack = linked_parent.join("pack.zip");
    let out_export = linked_parent.join("export.zip");

    let err_pack =
        export_reproducer_pack(&cfg, "r1", &out_pack).expect_err("must reject symlink parent");
    assert!(err_pack.to_string().contains("symlinked output path"));
    let err_export =
        export_artifacts(&cfg, "r1", &out_export).expect_err("must reject symlink parent");
    assert!(err_export.to_string().contains("symlinked output path"));
}

#[test]
fn bundle_includes_replay_ci_and_env_reports() {
    let root = std::env::temp_dir().join(format!("fozzy-bundle-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace = root.join("trace.fozzy");
    let raw = r#"{
          "format":"fozzy-trace",
          "version":2,
          "engine":{"version":"0.1.0"},
          "mode":"run",
          "scenario_path":null,
          "scenario":{"version":1,"name":"x","steps":[]},
          "decisions":[],
          "events":[],
          "summary":{
            "status":"pass",
            "mode":"run",
            "identity":{"runId":"r1","seed":1},
            "startedAt":"2026-01-01T00:00:00Z",
            "finishedAt":"2026-01-01T00:00:00Z",
            "durationMs":0
          }
        }"#;
    std::fs::write(&trace, raw).expect("write trace");
    let out = root.join("bundle.zip");
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

    export_gate_bundle(&cfg, &trace.display().to_string(), &out).expect("bundle");
    let file = std::fs::File::open(&out).expect("zip");
    let mut z = zip::ZipArchive::new(file).expect("zip parse");
    let mut names = Vec::new();
    for i in 0..z.len() {
        names.push(z.by_index(i).expect("entry").name().to_string());
    }
    assert!(names.iter().any(|n| n == "trace.fozzy"));
    assert!(names.iter().any(|n| n == "replay.report.json"));
    assert!(names.iter().any(|n| n == "ci.report.json"));
    assert!(names.iter().any(|n| n == "env.json"));
}

#[test]
fn bundle_rejects_stale_report_without_manifest() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-bundle-stale-report-{}",
        uuid::Uuid::new_v4()
    ));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let trace_path = run_dir.join("trace.fozzy");
    let report_path = run_dir.join("report.json");

    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &run_dir),
    )
    .expect("trace");
    std::fs::write(
        &report_path,
        valid_report_json("r1", &report_path, &run_dir),
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
    let out = root.join("bundle.zip");

    let err = export_gate_bundle(&cfg, "r1", &out).expect_err("bundle must fail");
    assert!(
        err.to_string()
            .contains("missing required files: manifest.json")
    );
}
