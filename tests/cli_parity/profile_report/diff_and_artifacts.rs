use super::*;

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
