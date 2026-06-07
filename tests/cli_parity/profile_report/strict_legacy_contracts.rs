use super::*;

#[test]
fn profile_strict_and_unsafe_legacy_behavior_and_capture_mode_budgets() {
    let ws = temp_workspace("profile-strict-unsafe");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let missing = ws.join("missing.trace.fozzy");

    let strict = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "top".into(),
            missing.to_string_lossy().to_string(),
            "--heap".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_ne!(strict.status.code(), Some(0), "strict should fail");

    let unsafe_out = run_cli_in(
        &ws,
        &[
            "--unsafe".into(),
            "profile".into(),
            "top".into(),
            missing.to_string_lossy().to_string(),
            "--heap".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(unsafe_out.status.code(), Some(0), "unsafe should warn");
    let unsafe_doc = parse_json_stdout(&unsafe_out);
    assert_eq!(
        unsafe_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_contract_warning.v1"
    );

    let run_dir = ws.join(".fozzy/runs/legacy-partial");
    std::fs::create_dir_all(&run_dir).expect("legacy run dir");
    std::fs::write(
        run_dir.join("profile.metrics.json"),
        br#"{"schemaVersion":"fozzy.profile_metrics.v2","runId":"legacy","timeDomains":{"virtualTime":"deterministic","hostMonotonicTime":"host"},"virtualTimeMs":0,"hostTimeMs":0,"cpuTimeMs":0,"allocBytes":0,"inUseBytes":0,"p50LatencyMs":0,"p95LatencyMs":0,"p99LatencyMs":0,"maxLatencyMs":0,"ioOps":0,"schedOps":0}"#,
    )
    .expect("legacy metrics");

    let legacy_strict = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "top".into(),
            "legacy-partial".into(),
            "--heap".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_ne!(
        legacy_strict.status.code(),
        Some(0),
        "strict should fail for legacy partial artifacts"
    );
    let legacy_unsafe = run_cli_in(
        &ws,
        &[
            "--unsafe".into(),
            "profile".into(),
            "top".into(),
            "legacy-partial".into(),
            "--heap".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        legacy_unsafe.status.code(),
        Some(0),
        "unsafe should downgrade legacy contract error"
    );

    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    for (level, expect_profile) in [("baseline", false), ("full", true)] {
        let out = run_cli_in(
            &ws,
            &[
                "run".into(),
                scenario.to_string_lossy().to_string(),
                "--det".into(),
                "--seed".into(),
                "7".into(),
                "--profile-capture".into(),
                level.into(),
                "--config".into(),
                cfg.to_string_lossy().to_string(),
                "--json".into(),
            ],
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "run ({level}) stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
        let doc = parse_json_stdout(&out);
        let run_id = json_run_id(&doc);
        let metrics_path = ws
            .join(".fozzy")
            .join("runs")
            .join(run_id)
            .join("profile.metrics.json");
        assert_eq!(
            metrics_path.exists(),
            expect_profile,
            "profile artifact policy mismatch for {level}"
        );
        if expect_profile {
            let size = std::fs::metadata(&metrics_path)
                .expect("metrics metadata")
                .len();
            assert!(size > 0, "metrics artifact should be non-empty");
            assert!(
                size < 2 * 1024 * 1024,
                "metrics artifact should stay bounded"
            );
        }
    }
}
