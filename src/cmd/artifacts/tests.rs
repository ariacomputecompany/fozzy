    use super::*;

    fn valid_manifest_json(run_id: &str) -> String {
        format!(
            r#"{{"schemaVersion":"fozzy.run_manifest.v1","runId":"{run_id}","mode":"run","status":"pass","seed":1,"startedAt":"2026-01-01T00:00:00Z","finishedAt":"2026-01-01T00:00:00Z","durationMs":0,"findingsCount":0}}"#
        )
    }

    fn valid_report_json(run_id: &str, report_path: &Path, artifacts_dir: &Path) -> String {
        format!(
            r#"{{
  "status":"pass",
  "mode":"run",
  "identity":{{
    "runId":"{run_id}",
    "seed":1,
    "reportPath":"{}",
    "artifactsDir":"{}"
  }},
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "findings":[]
}}"#,
            report_path.display(),
            artifacts_dir.display()
        )
    }

    fn valid_report_and_manifest_json(
        run_id: &str,
        report_path: &Path,
        artifacts_dir: &Path,
        trace_path: Option<&Path>,
    ) -> (String, String) {
        let trace_json = trace_path.map(|path| format!(r#","tracePath":"{}""#, path.display()));
        let report = format!(
            r#"{{
  "status":"pass",
  "mode":"run",
  "identity":{{
    "runId":"{run_id}",
    "seed":1,
    "reportPath":"{}",
    "artifactsDir":"{}"{}
  }},
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "findings":[]
}}"#,
            report_path.display(),
            artifacts_dir.display(),
            trace_json.clone().unwrap_or_default()
        );
        let manifest = format!(
            r#"{{
  "schemaVersion":"fozzy.run_manifest.v1",
  "runId":"{run_id}",
  "mode":"run",
  "status":"pass",
  "seed":1,
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "reportPath":"{}",
  "artifactsDir":"{}",
  "findingsCount":0{}
}}"#,
            report_path.display(),
            artifacts_dir.display(),
            trace_path
                .map(|path| format!(r#","tracePath":"{}""#, path.display()))
                .unwrap_or_default()
        );
        (report, manifest)
    }

    fn valid_trace_json(
        run_id: &str,
        trace_path: &Path,
        report_path: &Path,
        artifacts_dir: &Path,
    ) -> String {
        format!(
            r#"{{
  "format":"fozzy-trace",
  "version":4,
  "engine":{{"version":"0.1.0"}},
  "mode":"run",
  "scenario_path":null,
  "scenario":{{"version":1,"name":"x","steps":[]}},
  "decisions":[],
  "events":[],
  "summary":{{
    "status":"pass",
    "mode":"run",
    "identity":{{
      "runId":"{run_id}",
      "seed":1,
      "tracePath":"{}",
      "reportPath":"{}",
      "artifactsDir":"{}"
    }},
    "startedAt":"2026-01-01T00:00:00Z",
    "finishedAt":"2026-01-01T00:00:00Z",
    "durationMs":0,
    "durationNs":0
  }}
}}"#,
            trace_path.display(),
            report_path.display(),
            artifacts_dir.display()
        )
    }

    fn valid_profile_metrics_json(run_id: &str) -> String {
        format!(
            r#"{{
  "schemaVersion":"fozzy.profile_metrics.v1",
  "runId":"{run_id}",
  "timeDomains":{{
    "virtualTime":"virtual_time",
    "hostMonotonicTime":"host_monotonic_time"
  }},
  "virtualTimeMs":0,
  "hostTimeMs":0,
  "cpuTimeMs":0,
  "allocBytes":0,
  "inUseBytes":0,
  "p50LatencyMs":0,
  "p95LatencyMs":0,
  "p99LatencyMs":0,
  "maxLatencyMs":0,
  "ioOps":0,
  "schedOps":0
}}"#
        )
    }

    fn valid_profile_timeline_json(run_id: &str, seed: u64) -> String {
        format!(
            r#"{{
  "schemaVersion":"fozzy.profile_timeline.v1",
  "runId":"{run_id}",
  "timeDomains":{{
    "virtualTime":"virtual_time",
    "hostMonotonicTime":"host_monotonic_time"
  }},
  "events":[{{
    "t_virtual":0,
    "kind":"event",
    "run_id":"{run_id}",
    "seed":{seed},
    "thread":"main",
    "span_id":"root",
    "tags":{{}},
    "cost":{{}}
  }}]
}}"#
        )
    }

    fn valid_memory_leaks_json(bytes: u64) -> String {
        format!(r#"[{{"allocId":1,"bytes":{bytes},"callsiteHash":"callsite-1"}}]"#)
    }

    fn valid_memory_graph_json() -> &'static str {
        r#"{
  "nodes":[
    {"id":"alloc:1","kind":"alloc","label":"1"},
    {"id":"free:1","kind":"free","label":"1"},
    {"id":"callsite:callsite-1","kind":"callsite","label":"callsite-1"}
  ],
  "edges":[
    {"from":"callsite:callsite-1","to":"alloc:1","kind":"allocates"},
    {"from":"alloc:1","to":"free:1","kind":"freed_by"}
  ]
}"#
    }

    fn valid_memory_timeline_json() -> &'static str {
        r#"[
  {"index":0,"timeMs":0,"kind":"alloc","fields":{"allocId":1,"bytes":128}},
  {"index":1,"timeMs":1,"kind":"free","fields":{"allocId":1,"bytes":128}}
]"#
    }

    fn valid_memory_delta_json(
        after_leaked_bytes: u64,
        after_leaked_allocs: u64,
        after_alloc_count: u64,
    ) -> String {
        format!(
            r#"{{
  "schemaVersion":"fozzy.memory_delta.v1",
  "beforeLeakedBytes":0,
  "afterLeakedBytes":{after_leaked_bytes},
  "beforeLeakedAllocs":0,
  "afterLeakedAllocs":{after_leaked_allocs},
  "beforeAllocCount":0,
  "afterAllocCount":{after_alloc_count}
}}"#
        )
    }

    fn valid_events_json(name: &str) -> String {
        format!(r#"[{{"time_ms":0,"name":"{name}","fields":{{}}}}]"#)
    }

    fn valid_timeline_json(name: &str) -> String {
        format!(r#"[{{"index":0,"time_ms":0,"name":"{name}","fields":{{}}}}]"#)
    }

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
        let err =
            export_artifacts(&cfg, "does-not-exist-input.fozzy", &out).expect_err("must fail");
        assert!(err.to_string().contains("not found"));
        assert!(!out.exists(), "zip should not exist on failure");
    }

    #[test]
    fn export_empty_run_errors() {
        let root =
            std::env::temp_dir().join(format!("fozzy-artifacts-empty-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp root");
        let run_id = "empty-run";
        std::fs::create_dir_all(root.join(".fozzy").join("runs").join(run_id))
            .expect("create run dir");
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

        let root =
            std::env::temp_dir().join(format!("fozzy-pack-symlink-{}", uuid::Uuid::new_v4()));
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

        let err = export_reproducer_pack(&cfg, "r1", &out_dir)
            .expect_err("must reject symlink overwrite");
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

        let err = export_reproducer_pack(&cfg, "r1", &out_dir)
            .expect_err("must reject symlink overwrite");
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
        let root =
            std::env::temp_dir().join(format!("fozzy-pack-incomplete-{}", uuid::Uuid::new_v4()));
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

        let err_export = export_artifacts(&cfg, "r1", &out_export)
            .expect_err("export must fail for incomplete run");
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

    #[test]
    fn direct_trace_uses_declared_artifacts_dir_and_lists_detached_profile_files() {
        let root =
            std::env::temp_dir().join(format!("fozzy-trace-artifacts-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("root dir");
        let trace = root.join("record.trace.min.fozzy");
        let detached_artifacts = root.join("record.trace.min.profile-artifacts");
        std::fs::create_dir_all(&detached_artifacts).expect("artifacts dir");
        let report_path = detached_artifacts.join("report.json");
        std::fs::write(
            &trace,
            valid_trace_json("r1", &trace, &report_path, &detached_artifacts),
        )
        .expect("write trace");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &detached_artifacts, Some(&trace));
        std::fs::write(&report_path, report).expect("write report");
        std::fs::write(detached_artifacts.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            detached_artifacts.join("profile.metrics.json"),
            valid_profile_metrics_json("r1"),
        )
        .expect("write metrics");

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

        let resolved =
            resolve_artifacts_dir(&cfg, &trace.to_string_lossy()).expect("resolve artifacts dir");
        assert_eq!(resolved, detached_artifacts);

        let entries = artifacts_list(&cfg, &trace.to_string_lossy()).expect("artifacts list");
        assert!(entries.iter().any(|entry| {
            entry.path
                == detached_artifacts
                    .join("profile.metrics.json")
                    .to_string_lossy()
        }));
    }

    #[test]
    fn direct_trace_uses_manifest_only_declared_artifacts_dir() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-trace-manifest-only-artifacts-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("root dir");
        let trace = root.join("record.trace.fozzy");
        let detached_artifacts = root.join("record.trace.profile-artifacts");
        std::fs::create_dir_all(&detached_artifacts).expect("artifacts dir");
        let report_path = detached_artifacts.join("report.json");
        std::fs::write(
            &trace,
            valid_trace_json("r1", &trace, &report_path, &detached_artifacts),
        )
        .expect("write trace");
        let (_, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &detached_artifacts, Some(&trace));
        std::fs::write(detached_artifacts.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            detached_artifacts.join("profile.metrics.json"),
            valid_profile_metrics_json("r1"),
        )
        .expect("write metrics");

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

        let resolved =
            resolve_artifacts_dir(&cfg, &trace.to_string_lossy()).expect("resolve artifacts dir");
        assert_eq!(resolved, detached_artifacts);

        let entries = artifacts_list(&cfg, &trace.to_string_lossy()).expect("artifacts list");
        assert!(entries.iter().any(|entry| {
            entry.path
                == detached_artifacts
                    .join("profile.metrics.json")
                    .to_string_lossy()
        }));
    }

    #[test]
    fn direct_trace_ignores_untrusted_declared_artifacts_dir() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-trace-untrusted-declared-artifacts-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("root dir");
        let trace = root.join("record.trace.fozzy");
        let forged_artifacts = root.join("forged-artifacts");
        std::fs::create_dir_all(&forged_artifacts).expect("forged dir");
        std::fs::write(
            &trace,
            format!(
                r#"{{
  "format":"fozzy-trace",
  "version":4,
  "engine":{{"version":"0.1.0"}},
  "mode":"run",
  "scenario_path":null,
  "scenario":{{"version":1,"name":"x","steps":[]}},
  "decisions":[],
  "events":[],
  "summary":{{
    "status":"pass",
    "mode":"run",
    "identity":{{
      "runId":"r1",
      "seed":7,
      "tracePath":"{}",
      "artifactsDir":"{}"
    }},
    "startedAt":"2026-01-01T00:00:00Z",
    "finishedAt":"2026-01-01T00:00:00Z",
    "durationMs":1
  }}
}}"#,
                trace.display(),
                forged_artifacts.display()
            ),
        )
        .expect("write trace");
        std::fs::write(
            forged_artifacts.join("profile.metrics.json"),
            br#"{"schemaVersion":"forged"}"#,
        )
        .expect("write forged metrics");

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

        let resolved = resolve_artifacts_dir(&cfg, &trace.to_string_lossy()).expect("resolve");
        assert_eq!(resolved, root);

        let entries = artifacts_list(&cfg, &trace.to_string_lossy()).expect("artifacts list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, trace.to_string_lossy());
    }

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
        std::fs::write(run_dir.join("memory.graph.json"), valid_memory_graph_json())
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
        std::fs::write(root.join("timeline.json"), valid_timeline_json("forged"))
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

    #[test]
    fn artifacts_diff_marks_same_size_content_change_as_changed() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-diff-same-size-{}",
            uuid::Uuid::new_v4()
        ));
        let left_dir = root.join(".fozzy").join("runs").join("left");
        let right_dir = root.join(".fozzy").join("runs").join("right");
        std::fs::create_dir_all(&left_dir).expect("left dir");
        std::fs::create_dir_all(&right_dir).expect("right dir");

        let left_trace = left_dir.join("trace.fozzy");
        let right_trace = right_dir.join("trace.fozzy");
        let mut left: crate::TraceFile = serde_json::from_str(&valid_trace_json(
            "left",
            &left_trace,
            &left_dir.join("report.json"),
            &left_dir,
        ))
        .expect("left trace");
        let mut right: crate::TraceFile = serde_json::from_str(&valid_trace_json(
            "right",
            &right_trace,
            &right_dir.join("report.json"),
            &right_dir,
        ))
        .expect("right trace");
        left.events = vec![crate::TraceEvent {
            time_ms: 0,
            name: "aaaaaa".to_string(),
            fields: serde_json::Map::new(),
        }];
        right.events = vec![crate::TraceEvent {
            time_ms: 0,
            name: "bbbbbb".to_string(),
            fields: serde_json::Map::new(),
        }];
        left.write_json(&left_trace).expect("left trace write");
        right.write_json(&right_trace).expect("right trace write");
        std::fs::write(
            left_dir.join("report.json"),
            serde_json::to_vec_pretty(&left.summary).expect("left report"),
        )
        .expect("left report write");
        std::fs::write(
            right_dir.join("report.json"),
            serde_json::to_vec_pretty(&right.summary).expect("right report"),
        )
        .expect("right report write");
        crate::write_run_manifest(&left.summary, &left_dir).expect("left manifest");
        crate::write_run_manifest(&right.summary, &right_dir).expect("right manifest");
        std::fs::write(left_dir.join("events.json"), valid_events_json("aaaaaa"))
            .expect("left events");
        std::fs::write(right_dir.join("events.json"), valid_events_json("bbbbbb"))
            .expect("right events");

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

        let diff = artifacts_diff(&cfg, "left", "right").expect("artifacts diff");
        let events_delta = diff
            .files
            .iter()
            .find(|file| file.key == "Events:events.json")
            .expect("events delta");
        assert_eq!(events_delta.left_size_bytes, events_delta.right_size_bytes);
        assert!(
            events_delta.changed,
            "same-size content drift must still be marked changed"
        );
    }

    #[test]
    fn run_id_uses_report_declared_external_trace_path() {
        let root =
            std::env::temp_dir().join(format!("fozzy-artifacts-external-{}", uuid::Uuid::new_v4()));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let external_trace = root.join("external.trace.fozzy");
        let report_path = run_dir.join("report.json");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&external_trace));
        std::fs::write(
            &external_trace,
            valid_trace_json("r1", &external_trace, &report_path, &run_dir),
        )
        .expect("trace");
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

        let trace_path = resolve_trace_path(&cfg, "r1").expect("resolve trace path");
        assert_eq!(trace_path, external_trace);
        let entries = artifacts_list(&cfg, "r1").expect("artifacts list");
        assert!(entries.iter().any(|entry| {
            entry.path == external_trace.to_string_lossy()
                && matches!(entry.kind, ArtifactKind::Trace)
        }));
    }

    #[test]
    fn resolve_trace_path_rejects_conflicting_local_and_declared_trace_identities() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-conflict-local-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let local_trace = run_dir.join("trace.fozzy");
        let external_trace = root.join("external.trace.fozzy");
        let report_path = run_dir.join("report.json");
        let (report, manifest) =
            valid_report_and_manifest_json("r1", &report_path, &run_dir, Some(&external_trace));
        std::fs::write(&report_path, report).expect("report");
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");
        std::fs::write(
            &local_trace,
            valid_trace_json("r1", &local_trace, &report_path, &run_dir),
        )
        .expect("local trace");
        std::fs::write(
            &external_trace,
            valid_trace_json("r1", &external_trace, &report_path, &run_dir),
        )
        .expect("external trace");

        let err =
            resolve_trace_path_from_artifacts_dir(&run_dir).expect_err("must reject conflict");
        assert!(
            err.to_string()
                .contains("conflicting local and declared trace identities")
        );
    }

    #[test]
    fn resolve_trace_path_rejects_conflicting_report_and_manifest_trace_identities() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-conflict-declared-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let report_trace = root.join("report.trace.fozzy");
        let manifest_trace = root.join("manifest.trace.fozzy");
        let report_path = run_dir.join("report.json");
        std::fs::write(
            &report_path,
            valid_report_json("r1", &report_path, &run_dir).replace(
                &format!(r#""artifactsDir":"{}""#, run_dir.display()),
                &format!(
                    r#""artifactsDir":"{}","tracePath":"{}""#,
                    run_dir.display(),
                    report_trace.display()
                ),
            ),
        )
        .expect("report");
        std::fs::write(
            run_dir.join("manifest.json"),
            format!(
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
                manifest_trace.display()
            ),
        )
        .expect("manifest");
        std::fs::write(
            &report_trace,
            valid_trace_json("r1", &report_trace, &report_path, &run_dir),
        )
        .expect("report trace");
        std::fs::write(
            &manifest_trace,
            valid_trace_json("r1", &manifest_trace, &report_path, &run_dir),
        )
        .expect("manifest trace");

        let err =
            resolve_trace_path_from_artifacts_dir(&run_dir).expect_err("must reject conflict");
        assert!(
            err.to_string()
                .contains("conflicting declared trace identities")
        );
    }

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
        assert!(
            err_pack
                .to_string()
                .contains("missing required files: manifest.json")
        );
        let err_export = export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
            .expect_err("export must fail");
        assert!(
            err_export
                .to_string()
                .contains("missing required files: manifest.json")
        );
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
        assert!(
            err.to_string()
                .contains("missing required files: manifest.json")
        );
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
        assert!(
            err.to_string()
                .contains("report.json and manifest.json are required to trust sibling files")
        );
    }

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
        assert!(
            err_pack
                .to_string()
                .contains("report.json and manifest.json are required to trust sibling files")
        );
        let err_export = export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
            .expect_err("export must fail");
        assert!(
            err_export
                .to_string()
                .contains("report.json and manifest.json are required to trust sibling files")
        );
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
        let (report, manifest) = valid_report_and_manifest_json(
            "sibling-run",
            &report_path,
            &root,
            Some(&sibling_trace),
        );
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

    #[test]
    fn direct_trace_export_pack_and_bundle_allow_manifest_only_exact_sibling_wrapper() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-direct-trace-manifest-only-sibling-{}",
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
        let out_bundle = root.join("bundle.zip");

        export_reproducer_pack(&cfg, &trace_path.to_string_lossy(), &out_pack)
            .expect("pack should succeed");
        export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
            .expect("export should succeed");
        export_gate_bundle(&cfg, &trace_path.to_string_lossy(), &out_bundle)
            .expect("bundle should succeed");

        assert!(out_pack.exists());
        assert!(out_export.exists());
        assert!(out_bundle.exists());
    }

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

        let (report, manifest) = valid_report_and_manifest_json(
            "sibling-run",
            &report_path,
            &root,
            Some(&sibling_trace),
        );
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

    #[test]
    fn checked_report_loader_allows_replay_runs_to_reference_source_trace() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-replay-source-trace-{}",
            uuid::Uuid::new_v4()
        ));
        let run_dir = root.join(".fozzy").join("runs").join("r1");
        std::fs::create_dir_all(&run_dir).expect("mkdir");
        let trace_path = root.join("source.trace.fozzy");
        std::fs::write(
            &trace_path,
            valid_trace_json(
                "source-run",
                &trace_path,
                &root.join(".fozzy/runs/source-run/report.json"),
                &root.join(".fozzy/runs/source-run"),
            ),
        )
        .expect("write source trace");
        let report_path = run_dir.join("report.json");
        std::fs::write(
            &report_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "status": "pass",
                "mode": "replay",
                "identity": {
                    "runId": "r1",
                    "seed": 1,
                    "tracePath": trace_path,
                    "reportPath": report_path,
                    "artifactsDir": run_dir
                },
                "startedAt": "2026-01-01T00:00:00Z",
                "finishedAt": "2026-01-01T00:00:00Z",
                "durationMs": 0,
                "durationNs": 0,
                "findings": []
            }))
            .expect("report json"),
        )
        .expect("write report");
        std::fs::write(
            run_dir.join("manifest.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": "fozzy.run_manifest.v1",
                "runId": "r1",
                "mode": "replay",
                "status": "pass",
                "seed": 1,
                "startedAt": "2026-01-01T00:00:00Z",
                "finishedAt": "2026-01-01T00:00:00Z",
                "durationMs": 0,
                "durationNs": 0,
                "tracePath": trace_path,
                "reportPath": report_path,
                "artifactsDir": run_dir,
                "findingsCount": 0
            }))
            .expect("manifest json"),
        )
        .expect("write manifest");

        let summary = load_checked_report_summary_from_artifacts_dir(&run_dir, "r1")
            .expect("checked report load")
            .expect("summary");
        assert_eq!(summary.mode, crate::RunMode::Replay);
        assert_eq!(summary.identity.run_id, "r1");
    }

    #[test]
    fn artifacts_diff_rejects_stale_report_without_manifest() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-stale-diff-{}",
            uuid::Uuid::new_v4()
        ));
        let base_dir = root.join(".fozzy");
        let left_dir = base_dir.join("runs").join("left");
        let right_dir = base_dir.join("runs").join("right");
        std::fs::create_dir_all(&left_dir).expect("left dir");
        std::fs::create_dir_all(&right_dir).expect("right dir");

        std::fs::write(
            left_dir.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "status": "pass",
                "mode": "run",
                "identity": {
                    "runId": "left",
                    "seed": 1,
                    "tracePath": "/tmp/missing-left.trace.fozzy",
                    "reportPath": left_dir.join("report.json"),
                    "artifactsDir": left_dir
                },
                "startedAt": "2026-01-01T00:00:00Z",
                "finishedAt": "2026-01-01T00:00:00Z",
                "durationMs": 0,
                "durationNs": 0,
                "findings": []
            }))
            .expect("left report json"),
        )
        .expect("write left report");

        let trace_path = right_dir.join("trace.fozzy");
        let report_path = right_dir.join("report.json");
        let (report, manifest) =
            valid_report_and_manifest_json("right", &report_path, &right_dir, Some(&trace_path));
        std::fs::write(
            &trace_path,
            valid_trace_json("right", &trace_path, &report_path, &right_dir),
        )
        .expect("write right trace");
        std::fs::write(&report_path, report).expect("write right report");
        std::fs::write(right_dir.join("manifest.json"), manifest).expect("write right manifest");

        let cfg = crate::Config {
            base_dir,
            ..crate::Config::default()
        };
        let err = artifacts_diff(&cfg, "left", "right").expect_err("must reject stale left");
        assert!(
            err.to_string()
                .contains("missing required files: manifest.json")
        );
    }

    #[test]
    fn artifacts_list_rejects_stale_report_without_manifest() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-stale-list-{}",
            uuid::Uuid::new_v4()
        ));
        let base_dir = root.join(".fozzy");
        let run_dir = base_dir.join("runs").join("stale");
        std::fs::create_dir_all(&run_dir).expect("run dir");
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "status": "pass",
                "mode": "run",
                "identity": {
                    "runId": "stale",
                    "seed": 1,
                    "tracePath": "/tmp/missing-stale-list.trace.fozzy",
                    "reportPath": run_dir.join("report.json"),
                    "artifactsDir": run_dir
                },
                "startedAt": "2026-01-01T00:00:00Z",
                "finishedAt": "2026-01-01T00:00:00Z",
                "durationMs": 0,
                "durationNs": 0,
                "findings": []
            }))
            .expect("report json"),
        )
        .expect("write report");

        let cfg = crate::Config {
            base_dir,
            ..crate::Config::default()
        };
        let err = artifacts_list(&cfg, "stale").expect_err("must reject stale list");
        assert!(
            err.to_string()
                .contains("missing required files: manifest.json")
        );
    }

    #[test]
    fn artifacts_list_rejects_trace_only_run_wrapper() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-trace-only-list-{}",
            uuid::Uuid::new_v4()
        ));
        let base_dir = root.join(".fozzy");
        let run_dir = base_dir.join("runs").join("trace-only");
        std::fs::create_dir_all(&run_dir).expect("run dir");
        let trace_path = run_dir.join("trace.fozzy");
        std::fs::write(
            &trace_path,
            valid_trace_json(
                "trace-only",
                &trace_path,
                &run_dir.join("report.json"),
                &run_dir,
            ),
        )
        .expect("write trace");

        let cfg = crate::Config {
            base_dir,
            ..crate::Config::default()
        };
        let err = artifacts_list(&cfg, "trace-only").expect_err("must reject trace-only list");
        assert!(
            err.to_string()
                .contains("missing required files: report.json, manifest.json")
        );
    }

    #[test]
    fn artifacts_list_accepts_manifest_only_run_wrapper() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-artifacts-manifest-only-list-{}",
            uuid::Uuid::new_v4()
        ));
        let base_dir = root.join(".fozzy");
        let run_dir = base_dir.join("runs").join("manifest-only");
        std::fs::create_dir_all(&run_dir).expect("run dir");
        let external_trace = root.join("manifest-only.trace.fozzy");
        std::fs::write(
            &external_trace,
            valid_trace_json(
                "manifest-only",
                &external_trace,
                &run_dir.join("report.json"),
                &run_dir,
            ),
        )
        .expect("trace");
        let (_, manifest) = valid_report_and_manifest_json(
            "manifest-only",
            &run_dir.join("report.json"),
            &run_dir,
            Some(&external_trace),
        );
        std::fs::write(run_dir.join("manifest.json"), manifest).expect("manifest");

        let cfg = crate::Config {
            base_dir,
            ..crate::Config::default()
        };
        let entries = artifacts_list(&cfg, "manifest-only").expect("list");
        assert!(entries.iter().any(|entry| {
            entry.path == external_trace.to_string_lossy()
                && matches!(entry.kind, ArtifactKind::Trace)
        }));
        assert!(entries.iter().any(|entry| {
            entry.path == run_dir.join("manifest.json").to_string_lossy()
                && matches!(entry.kind, ArtifactKind::Manifest)
        }));
    }

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
    fn pack_dir_prunes_stale_preexisting_files() {
        let root =
            std::env::temp_dir().join(format!("fozzy-pack-stale-dir-{}", uuid::Uuid::new_v4()));
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
        let root =
            std::env::temp_dir().join(format!("fozzy-pack-bad-report-{}", uuid::Uuid::new_v4()));
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
