use super::*;

#[test]
fn direct_trace_uses_declared_artifacts_dir_and_lists_detached_profile_files() {
    let root = std::env::temp_dir().join(format!("fozzy-trace-artifacts-{}", uuid::Uuid::new_v4()));
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
