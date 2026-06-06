use super::*;

#[test]
fn profile_help_uses_run_or_trace_value_name() {
    for sub in ["top", "flame", "timeline", "export", "shrink", "doctor"] {
        let out = run_cli(&["profile".into(), sub.to_string(), "--help".into()]);
        assert_eq!(out.status.code(), Some(0), "help should exit 0");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("RUN_OR_TRACE"),
            "help should show RUN_OR_TRACE for profile {sub}; got: {stdout}"
        );
        assert!(
            stdout.contains("latest|last-pass|last-fail")
                || stdout.contains("latest, last-pass, last-fail"),
            "help should describe aliases for profile {sub}; got: {stdout}"
        );
    }
}

#[test]
fn profile_golden_run_top_flame_timeline_export_flow() {
    let ws = temp_workspace("profile-golden-flow");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("write scenario");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let trace = ws.join("golden.trace.fozzy");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.to_string_lossy().to_string(),
            "--det".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--record".into(),
            trace.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "run stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(trace.exists(), "expected recorded trace");

    let top = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "top".into(),
            trace.to_string_lossy().to_string(),
            "--heap".into(),
            "--latency".into(),
            "--io".into(),
            "--sched".into(),
            "--limit".into(),
            "10".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        top.status.code(),
        Some(0),
        "profile top stderr={}",
        String::from_utf8_lossy(&top.stderr)
    );
    let top_doc = parse_json_stdout(&top);
    assert_eq!(
        top_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_top.v1"
    );
    assert!(top_doc.get("heap").is_some());
    assert!(top_doc.get("latency").is_some());

    let folded_out = ws.join("heap.folded.txt");
    let flame = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "flame".into(),
            trace.to_string_lossy().to_string(),
            "--heap".into(),
            "--format".into(),
            "folded".into(),
            "--out".into(),
            folded_out.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        flame.status.code(),
        Some(0),
        "profile flame stderr={}",
        String::from_utf8_lossy(&flame.stderr)
    );
    assert!(folded_out.exists(), "folded output must exist");
    assert!(
        std::fs::metadata(&folded_out)
            .map(|m| m.len() > 0)
            .unwrap_or(false),
        "folded output should be non-empty"
    );

    let timeline_out = ws.join("timeline.json");
    let timeline = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "timeline".into(),
            trace.to_string_lossy().to_string(),
            "--format".into(),
            "json".into(),
            "--out".into(),
            timeline_out.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        timeline.status.code(),
        Some(0),
        "profile timeline stderr={}",
        String::from_utf8_lossy(&timeline.stderr)
    );
    assert!(timeline_out.exists(), "timeline output must exist");

    let export_out = ws.join("profile.otlp.json");
    let export = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "export".into(),
            trace.to_string_lossy().to_string(),
            "--format".into(),
            "otlp".into(),
            "--out".into(),
            export_out.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        export.status.code(),
        Some(0),
        "profile export stderr={}",
        String::from_utf8_lossy(&export.stderr)
    );
    assert!(export_out.exists(), "profile export output must exist");
}

#[test]
fn profile_record_replay_diff_explain_and_artifact_parity_flow() {
    let ws = temp_workspace("profile-diff-flow");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let left_scenario = ws.join("left.fozzy.json");
    let right_scenario = ws.join("right.fozzy.json");
    std::fs::write(&left_scenario, fixture("example.fozzy.json")).expect("left scenario");
    std::fs::write(&right_scenario, fixture("memory.pass.fozzy.json")).expect("right scenario");
    let left_trace = ws.join("left.trace.fozzy");
    let right_trace = ws.join("right.trace.fozzy");

    let mut left_run_id = String::new();
    let mut right_run_id = String::new();
    for (idx, (scenario, trace, seed)) in [
        (left_scenario.as_path(), left_trace.as_path(), "7"),
        (right_scenario.as_path(), right_trace.as_path(), "13"),
    ]
    .into_iter()
    .enumerate()
    {
        let run = run_cli_in(
            &ws,
            &[
                "run".into(),
                scenario.to_string_lossy().to_string(),
                "--det".into(),
                "--seed".into(),
                seed.into(),
                "--profile-capture".into(),
                "full".into(),
                "--record".into(),
                trace.to_string_lossy().to_string(),
                "--config".into(),
                cfg.to_string_lossy().to_string(),
                "--json".into(),
            ],
        );
        assert_eq!(
            run.status.code(),
            Some(0),
            "run stderr={}",
            String::from_utf8_lossy(&run.stderr)
        );
        let run_doc = parse_json_stdout(&run);
        if idx == 0 {
            left_run_id = json_run_id(&run_doc);
        } else {
            right_run_id = json_run_id(&run_doc);
        }
        let replay = run_cli_in(
            &ws,
            &[
                "replay".into(),
                trace.to_string_lossy().to_string(),
                "--profile-capture".into(),
                "full".into(),
                "--config".into(),
                cfg.to_string_lossy().to_string(),
                "--json".into(),
            ],
        );
        assert_eq!(
            replay.status.code(),
            Some(0),
            "replay stderr={}",
            String::from_utf8_lossy(&replay.stderr)
        );
    }

    let diff = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "diff".into(),
            left_trace.to_string_lossy().to_string(),
            right_trace.to_string_lossy().to_string(),
            "--heap".into(),
            "--latency".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        diff.status.code(),
        Some(0),
        "profile diff stderr={}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let diff_doc = parse_json_stdout(&diff);
    assert_eq!(
        diff_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_diff.v2"
    );
    assert!(diff_doc.get("regressions").is_some());

    let explain = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "explain".into(),
            left_trace.to_string_lossy().to_string(),
            "--diff-with".into(),
            right_trace.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        explain.status.code(),
        Some(0),
        "profile explain stderr={}",
        String::from_utf8_lossy(&explain.stderr)
    );
    let explain_doc = parse_json_stdout(&explain);
    assert_eq!(
        explain_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_explain.v1"
    );

    let ls = run_cli_in(
        &ws,
        &[
            "artifacts".into(),
            "ls".into(),
            left_run_id.clone(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        ls.status.code(),
        Some(0),
        "artifacts ls stderr={}",
        String::from_utf8_lossy(&ls.stderr)
    );
    let ls_stdout = String::from_utf8_lossy(&ls.stdout);
    assert!(ls_stdout.contains("profile.timeline.json"));
    assert!(ls_stdout.contains("profile.cpu.json"));
    assert!(ls_stdout.contains("profile.heap.json"));
    assert!(ls_stdout.contains("profile.latency.json"));
    assert!(ls_stdout.contains("profile.metrics.json"));

    let adiff = run_cli_in(
        &ws,
        &[
            "artifacts".into(),
            "diff".into(),
            left_run_id.clone(),
            right_run_id.clone(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        adiff.status.code(),
        Some(0),
        "artifacts diff stderr={}",
        String::from_utf8_lossy(&adiff.stderr)
    );
    let adiff_doc = parse_json_stdout(&adiff);
    assert_eq!(
        adiff_doc
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "diff"
    );

    let export_zip = ws.join("artifacts.export.zip");
    let pack_zip = ws.join("artifacts.pack.zip");
    let bundle_zip = ws.join("artifacts.bundle.zip");
    for (sub, out, target) in [
        ("export", &export_zip, left_run_id.clone()),
        ("pack", &pack_zip, left_run_id.clone()),
        (
            "bundle",
            &bundle_zip,
            left_trace.to_string_lossy().to_string(),
        ),
    ] {
        let out_cmd = run_cli_in(
            &ws,
            &[
                "artifacts".into(),
                sub.into(),
                target,
                "--out".into(),
                out.to_string_lossy().to_string(),
                "--config".into(),
                cfg.to_string_lossy().to_string(),
                "--json".into(),
            ],
        );
        assert_eq!(
            out_cmd.status.code(),
            Some(0),
            "artifacts {sub} stderr={}",
            String::from_utf8_lossy(&out_cmd.stderr)
        );
        let size = std::fs::metadata(out).expect("zip metadata").len();
        assert!(size > 0, "zip should be non-empty");
        assert!(size <= 8 * 1024 * 1024, "zip should stay within budget");
    }
}

#[test]
fn profile_shrink_trace_resolves_detached_artifact_directory() {
    let ws = temp_workspace("profile-shrink-artifacts");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("scenario");
    let trace = ws.join("record.trace.fozzy");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.to_string_lossy().to_string(),
            "--det".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--record".into(),
            trace.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "run stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );

    let shrink = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "shrink".into(),
            trace.to_string_lossy().to_string(),
            "--metric".into(),
            "alloc_bytes".into(),
            "--direction".into(),
            "increase".into(),
            "--minimize".into(),
            "all".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        shrink.status.code(),
        Some(0),
        "profile shrink stderr={}",
        String::from_utf8_lossy(&shrink.stderr)
    );
    let shrink_doc = parse_json_stdout(&shrink);
    assert_eq!(
        shrink_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_shrink.v2"
    );
    let out_trace = shrink_doc
        .get("outTrace")
        .and_then(|v| v.as_str())
        .expect("out trace");
    let artifacts_dir = shrink_doc
        .get("artifactsDir")
        .and_then(|v| v.as_str())
        .expect("artifacts dir");
    assert_ne!(
        Path::new(out_trace).parent().expect("trace parent"),
        Path::new(artifacts_dir)
    );

    let ls = run_cli_in(
        &ws,
        &[
            "artifacts".into(),
            "ls".into(),
            out_trace.into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        ls.status.code(),
        Some(0),
        "artifacts ls stderr={}",
        String::from_utf8_lossy(&ls.stderr)
    );
    let ls_doc = parse_json_stdout(&ls);
    let entries = ls_doc
        .get("entries")
        .and_then(|v| v.as_array())
        .expect("entries");
    assert!(entries.iter().any(|entry| {
        entry.get("path").and_then(|v| v.as_str())
            == Some(&format!("{artifacts_dir}/profile.metrics.json"))
    }));
}

#[test]
fn profile_direct_trace_prefers_declared_artifacts_dir_over_profile_cache() {
    let ws = temp_workspace("profile-direct-artifacts-dir");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("scenario");
    let trace = ws.join("record.trace.fozzy");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.to_string_lossy().to_string(),
            "--det".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--record".into(),
            trace.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "run stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );

    let shrink = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "shrink".into(),
            trace.to_string_lossy().to_string(),
            "--metric".into(),
            "alloc_bytes".into(),
            "--direction".into(),
            "increase".into(),
            "--minimize".into(),
            "all".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        shrink.status.code(),
        Some(0),
        "profile shrink stderr={}",
        String::from_utf8_lossy(&shrink.stderr)
    );
    let shrink_doc = parse_json_stdout(&shrink);
    let out_trace = shrink_doc
        .get("outTrace")
        .and_then(|v| v.as_str())
        .expect("out trace");

    let top = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "top".into(),
            out_trace.into(),
            "--heap".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        top.status.code(),
        Some(0),
        "profile top stderr={}",
        String::from_utf8_lossy(&top.stderr)
    );

    let cache_dir = ws.join(".fozzy").join("profile-cache");
    let cache_entries = if cache_dir.exists() {
        std::fs::read_dir(&cache_dir)
            .expect("read cache dir")
            .filter_map(Result::ok)
            .count()
    } else {
        0
    };
    assert_eq!(
        cache_entries, 0,
        "direct trace with declared artifacts dir should not synthesize duplicate profile-cache entries"
    );
}

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

