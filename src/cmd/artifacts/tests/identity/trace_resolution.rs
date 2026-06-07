use super::*;

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
    std::fs::write(left_dir.join("events.json"), valid_events_json("aaaaaa")).expect("left events");
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
        entry.path == external_trace.to_string_lossy() && matches!(entry.kind, ArtifactKind::Trace)
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

    let err = resolve_trace_path_from_artifacts_dir(&run_dir).expect_err("must reject conflict");
    assert!(err
        .to_string()
        .contains("conflicting local and declared trace identities"));
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

    let err = resolve_trace_path_from_artifacts_dir(&run_dir).expect_err("must reject conflict");
    assert!(err
        .to_string()
        .contains("conflicting declared trace identities"));
}
