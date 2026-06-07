use super::super::*;
use super::common::*;

#[test]
fn export_zip_normalizes_unicode_filenames_to_ascii() {
    let root =
        std::env::temp_dir().join(format!("fozzy-artifacts-unicode-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    let src_a = root.join("résumé-😀.json");
    let src_b = root.join("résumé 👀.json");
    std::fs::write(&src_a, b"{}").expect("write source a");
    std::fs::write(&src_b, b"{}").expect("write source b");
    let out = root.join("out.zip");

    export_artifacts_zip(&[src_a, src_b], &out).expect("zip export");

    let file = std::fs::File::open(&out).expect("open zip");
    let mut archive = zip::ZipArchive::new(file).expect("parse zip");
    let a = archive.by_index(0).expect("entry 0").name().to_string();
    let b = archive.by_index(1).expect("entry 1").name().to_string();

    assert!(a.is_ascii());
    assert!(b.is_ascii());
    assert_ne!(a, b);
    assert!(a.ends_with(".json"));
    assert!(b.ends_with(".json"));
}

#[test]
fn export_missing_input_returns_error_and_does_not_create_zip() {
    let root =
        std::env::temp_dir().join(format!("fozzy-artifacts-missing-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    let out = root.join("missing-input.zip");

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
    let err = export_artifacts(&cfg, "does-not-exist-input.fozzy", &out).expect_err("must fail");
    assert!(err.to_string().contains("not found"));
    assert!(!out.exists(), "zip should not exist on failure");
}

#[test]
fn export_empty_run_errors() {
    let root = std::env::temp_dir().join(format!("fozzy-artifacts-empty-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    let run_id = "empty-run";
    std::fs::create_dir_all(root.join(".fozzy").join("runs").join(run_id)).expect("create run dir");
    let out = root.join("empty.zip");

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
    let err = export_artifacts(&cfg, run_id, &out).expect_err("must fail");
    assert!(err.to_string().contains("no artifacts found"));
    assert!(!out.exists(), "zip should not exist on failure");
}

#[test]
fn pack_includes_runtime_metadata_files() {
    let root = std::env::temp_dir().join(format!("fozzy-pack-test-{}", uuid::Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let report_path = run_dir.join("report.json");
    let trace_path = run_dir.join("trace.fozzy");
    let (report, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&trace_path));
    std::fs::write(&report_path, report).expect("report");
    std::fs::write(run_dir.join("events.json"), b"[]").expect("events");
    std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &run_dir),
    )
    .expect("trace");
    let out = root.join("pack.zip");
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
    export_reproducer_pack(&cfg, "r1", &out).expect("pack");
    let file = std::fs::File::open(&out).expect("zip");
    let mut z = zip::ZipArchive::new(file).expect("zip parse");
    let mut names = Vec::new();
    for i in 0..z.len() {
        names.push(z.by_index(i).expect("entry").name().to_string());
    }
    assert!(names.iter().any(|n| n == "env.json"));
    assert!(names.iter().any(|n| n == "version.json"));
    assert!(names.iter().any(|n| n == "commandline.json"));
}

#[cfg(unix)]
#[test]
fn pack_dir_rejects_symlink_target_overwrite() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("fozzy-pack-symlink-{}", uuid::Uuid::new_v4()));
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

    let outside = root.join("outside.json");
    std::fs::write(&outside, br#"{"victim":true}"#).expect("outside");
    let out_dir = root.join("out");
    std::fs::create_dir_all(&out_dir).expect("out");
    symlink(&outside, out_dir.join("report.json")).expect("symlink");

    let err =
        export_reproducer_pack(&cfg, "r1", &out_dir).expect_err("must reject symlink overwrite");
    assert!(err.to_string().contains("symlinked output file"));
    let victim = std::fs::read_to_string(&outside).expect("read victim");
    assert!(victim.contains("victim"));
}

#[cfg(unix)]
#[test]
fn pack_dir_failure_atomic_on_symlink_error() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!("fozzy-pack-atomic-{}", uuid::Uuid::new_v4()));
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

    let outside = root.join("outside.json");
    std::fs::write(&outside, br#"{"victim":true}"#).expect("outside");
    let out_dir = root.join("out");
    std::fs::create_dir_all(&out_dir).expect("out");
    symlink(&outside, out_dir.join("manifest.json")).expect("symlink");

    let err =
        export_reproducer_pack(&cfg, "r1", &out_dir).expect_err("must reject symlink overwrite");
    assert!(err.to_string().contains("symlinked output file"));
    assert_eq!(
        std::fs::read(&outside).expect("victim read"),
        br#"{"victim":true}"#
    );
    assert!(
        !out_dir.join("report.json").exists(),
        "partial file should not be written"
    );
    assert!(
        !out_dir.join("events.json").exists(),
        "partial file should not be written"
    );
}

#[test]
fn pack_zip_is_byte_deterministic_for_same_run() {
    let root =
        std::env::temp_dir().join(format!("fozzy-pack-deterministic-{}", uuid::Uuid::new_v4()));
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

    let out_a = root.join("a.zip");
    let out_b = root.join("b.zip");
    export_reproducer_pack(&cfg, "r1", &out_a).expect("pack a");
    export_reproducer_pack(&cfg, "r1", &out_b).expect("pack b");

    let a = std::fs::read(&out_a).expect("read a");
    let b = std::fs::read(&out_b).expect("read b");
    assert_eq!(
        a, b,
        "repeated pack exports for same run must be byte-identical"
    );
}

#[test]
fn export_and_pack_reject_incomplete_run_directory() {
    let root = std::env::temp_dir().join(format!("fozzy-pack-incomplete-{}", uuid::Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    std::fs::write(run_dir.join("manifest.json"), valid_manifest_json("r1")).expect("manifest");
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

    let err_pack = export_reproducer_pack(&cfg, "r1", &out_pack)
        .expect_err("pack must fail for incomplete run");
    assert!(err_pack.to_string().contains("incomplete artifacts"));
    assert!(
        !out_pack.exists(),
        "pack zip should not be created on incomplete run"
    );

    let err_export =
        export_artifacts(&cfg, "r1", &out_export).expect_err("export must fail for incomplete run");
    assert!(err_export.to_string().contains("incomplete artifacts"));
    assert!(
        !out_export.exists(),
        "export zip should not be created on incomplete run"
    );
}

#[test]
fn resolve_artifacts_dir_supports_latest_last_pass_last_fail_aliases() {
    let root = std::env::temp_dir().join(format!("fozzy-aliases-{}", uuid::Uuid::new_v4()));
    let runs = root.join(".fozzy").join("runs");
    std::fs::create_dir_all(&runs).expect("runs dir");
    let mk = |id: &str, status: &str, finished: &str| {
        let dir = runs.join(id);
        std::fs::create_dir_all(&dir).expect("run dir");
        let report_path = dir.join("report.json");
        let trace_path = dir.join("trace.fozzy");
        let (report, manifest) =
            valid_report_and_manifest_json(id, &report_path, &dir, Some(&trace_path));
        let report = report
            .replace(r#""status":"pass""#, &format!(r#""status":"{status}""#))
            .replace(
                r#""finishedAt":"2026-01-01T00:00:00Z""#,
                &format!(r#""finishedAt":"{finished}""#),
            );
        let manifest = manifest
            .replace(r#""status":"pass""#, &format!(r#""status":"{status}""#))
            .replace(
                r#""finishedAt":"2026-01-01T00:00:00Z""#,
                &format!(r#""finishedAt":"{finished}""#),
            );
        let trace = valid_trace_json(id, &trace_path, &report_path, &dir)
            .replace(r#""status":"pass""#, &format!(r#""status":"{status}""#));
        std::fs::write(&trace_path, trace).expect("trace");
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(dir.join("manifest.json"), manifest).expect("manifest");
    };
    mk("r1", "pass", "2026-02-19T00:00:01Z");
    mk("r2", "fail", "2026-02-19T00:00:02Z");
    mk("r3", "pass", "2026-02-19T00:00:03Z");
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

    let latest = resolve_artifacts_dir(&cfg, "latest").expect("latest");
    assert!(latest.ends_with("r3"));
    let last_pass = resolve_artifacts_dir(&cfg, "last-pass").expect("last-pass");
    assert!(last_pass.ends_with("r3"));
    let last_fail = resolve_artifacts_dir(&cfg, "last-fail").expect("last-fail");
    assert!(last_fail.ends_with("r2"));
}
