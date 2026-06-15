use super::*;

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
        entry.path == external_trace.to_string_lossy() && matches!(entry.kind, ArtifactKind::Trace)
    }));
    assert!(entries.iter().any(|entry| {
        entry.path == run_dir.join("manifest.json").to_string_lossy()
            && matches!(entry.kind, ArtifactKind::Manifest)
    }));
}
