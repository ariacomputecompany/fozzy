use super::*;

#[test]
fn report_show_omits_profile_diagnosis_when_only_contract_warning_is_available() {
    let ws = temp_workspace("report-profile-diagnosis-contract-warning");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");

    let run_dir = ws.join(".fozzy/runs/legacy-report");
    std::fs::create_dir_all(&run_dir).expect("legacy run dir");
    std::fs::write(
        run_dir.join("report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "pass",
            "mode": "run",
            "identity": {
                "runId": "legacy-report",
                "seed": 7,
                "reportPath": ".fozzy/runs/legacy-report/report.json",
                "artifactsDir": ".fozzy/runs/legacy-report"
            },
            "startedAt": "2026-01-01T00:00:00Z",
            "finishedAt": "2026-01-01T00:00:00Z",
            "durationMs": 1,
            "durationNs": 1000000,
            "findings": []
        }))
        .expect("report json"),
    )
    .expect("write report");
    std::fs::write(
        run_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": "fozzy.run_manifest.v1",
            "runId": "legacy-report",
            "mode": "run",
            "status": "pass",
            "seed": 7,
            "startedAt": "2026-01-01T00:00:00Z",
            "finishedAt": "2026-01-01T00:00:00Z",
            "durationMs": 1,
            "durationNs": 1000000,
            "tracePath": serde_json::Value::Null,
            "reportPath": ".fozzy/runs/legacy-report/report.json",
            "artifactsDir": ".fozzy/runs/legacy-report",
            "findingsCount": 0
        }))
        .expect("manifest json"),
    )
    .expect("write manifest");
    std::fs::write(
        run_dir.join("profile.metrics.json"),
        br#"{"schemaVersion":"fozzy.profile_metrics.v2","runId":"legacy-report","timeDomains":{"virtualTime":"deterministic","hostMonotonicTime":"host"},"virtualTimeMs":0,"hostTimeMs":0,"cpuTimeMs":0,"allocBytes":0,"inUseBytes":0,"p50LatencyMs":0,"p95LatencyMs":0,"p99LatencyMs":0,"maxLatencyMs":0,"ioOps":0,"schedOps":0}"#,
    )
    .expect("legacy metrics");

    let out = run_cli_in(
        &ws,
        &[
            "report".into(),
            "show".into(),
            "legacy-report".into(),
            "--format".into(),
            "json".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "report show stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert!(
        doc.get("profileDiagnosis").is_none(),
        "contract warning should not be injected as profile diagnosis"
    );
}

#[test]
fn report_show_omits_profile_diagnosis_for_single_run_summary_only() {
    let ws = temp_workspace("report-show-no-single-run-diagnosis");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("write scenario");
    let trace = ws.join("pass.trace.fozzy");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.to_string_lossy().to_string(),
            "--det".into(),
            "--record".into(),
            trace.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(run.status.code(), Some(0), "run should succeed");

    let report = run_cli_in(
        &ws,
        &[
            "report".into(),
            "show".into(),
            trace.to_string_lossy().to_string(),
            "--format".into(),
            "json".into(),
            "--json".into(),
        ],
    );
    assert_eq!(report.status.code(), Some(0), "report show should succeed");
    let doc = parse_json_stdout(&report);
    assert!(
        doc.get("profileDiagnosis").is_none(),
        "single-run profile summary should not be injected as a diagnosis"
    );
}
