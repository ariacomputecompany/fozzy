use super::*;

#[test]
fn common_global_and_mode_flags_parse_across_run_like_commands() {
    let ws = temp_workspace("parity");
    std::fs::write(ws.join("fozzy.toml"), "base_dir = \".fozzy\"\n").expect("write config");
    std::fs::write(ws.join("example.fozzy.json"), fixture("example.fozzy.json"))
        .expect("write example");
    std::fs::write(
        ws.join("kv.explore.fozzy.json"),
        fixture("kv.explore.fozzy.json"),
    )
    .expect("write explore");

    let cfg = ws.join("fozzy.toml").to_string_lossy().to_string();
    let cwd = ws.to_string_lossy().to_string();
    let run_scenario = ws.join("example.fozzy.json").to_string_lossy().to_string();
    let explore_scenario = ws
        .join("kv.explore.fozzy.json")
        .to_string_lossy()
        .to_string();

    let run = run_cli(&[
        "run".into(),
        run_scenario.clone(),
        "--det".into(),
        "--seed".into(),
        "7".into(),
        "--reporter".into(),
        "pretty".into(),
        "--json".into(),
        "--cwd".into(),
        cwd.clone(),
        "--config".into(),
        cfg.clone(),
    ]);
    assert_eq!(
        run.status.code(),
        Some(0),
        "run stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );

    let test = run_cli(&[
        "test".into(),
        "example.fozzy.json".into(),
        "--det".into(),
        "--seed".into(),
        "7".into(),
        "--reporter".into(),
        "pretty".into(),
        "--json".into(),
        "--cwd".into(),
        cwd.clone(),
        "--config".into(),
        cfg.clone(),
    ]);
    assert_eq!(
        test.status.code(),
        Some(0),
        "test stderr={}",
        String::from_utf8_lossy(&test.stderr)
    );

    let fuzz = run_cli(&[
        "fuzz".into(),
        "scenario:example.fozzy.json".into(),
        "--seed".into(),
        "7".into(),
        "--runs".into(),
        "1".into(),
        "--reporter".into(),
        "pretty".into(),
        "--json".into(),
        "--cwd".into(),
        cwd.clone(),
        "--config".into(),
        cfg.clone(),
    ]);
    assert_ne!(
        fuzz.status.code(),
        Some(2),
        "fuzz should parse/execute; stderr={}",
        String::from_utf8_lossy(&fuzz.stderr)
    );

    let explore = run_cli(&[
        "explore".into(),
        explore_scenario,
        "--seed".into(),
        "7".into(),
        "--steps".into(),
        "10".into(),
        "--reporter".into(),
        "pretty".into(),
        "--json".into(),
        "--cwd".into(),
        cwd,
        "--config".into(),
        cfg,
    ]);
    assert_eq!(
        explore.status.code(),
        Some(0),
        "explore stderr={}",
        String::from_utf8_lossy(&explore.stderr)
    );
}

#[test]
fn strict_verify_alias_maps_to_canonical_strict_flag() {
    let ws = temp_workspace("strict-verify-alias");
    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--strict-verify".into(),
            "--json".into(),
        ],
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "strict-verify alias should map to strict: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn non_finite_flake_budget_is_rejected() {
    let ws = temp_workspace("flake-budget");
    std::fs::write(ws.join("fozzy.toml"), "base_dir = \".fozzy\"\n").expect("write config");
    let cfg = ws.join("fozzy.toml").to_string_lossy().to_string();
    let cwd = ws.to_string_lossy().to_string();

    let report_nan = run_cli(&[
        "report".into(),
        "flaky".into(),
        "r1".into(),
        "r2".into(),
        "--flake-budget".into(),
        "NaN".into(),
        "--cwd".into(),
        cwd.clone(),
        "--config".into(),
        cfg.clone(),
    ]);
    assert_eq!(report_nan.status.code(), Some(2), "NaN should be rejected");

    let report_inf = run_cli(&[
        "report".into(),
        "flaky".into(),
        "r1".into(),
        "r2".into(),
        "--flake-budget".into(),
        "inf".into(),
        "--cwd".into(),
        cwd.clone(),
        "--config".into(),
        cfg.clone(),
    ]);
    assert_eq!(report_inf.status.code(), Some(2), "inf should be rejected");

    let ci_nan = run_cli(&[
        "ci".into(),
        "trace.fozzy".into(),
        "--flake-budget".into(),
        "NaN".into(),
        "--cwd".into(),
        cwd,
        "--config".into(),
        cfg,
    ]);
    assert_eq!(ci_nan.status.code(), Some(2), "ci NaN should be rejected");
}

#[test]
fn json_mode_argument_errors_emit_json_for_parse_failures() {
    for args in [
        vec!["artifacts".into(), "export".into(), "--json".into()],
        vec!["ci".into(), "--json".into()],
        vec!["replay".into(), "--json".into()],
    ] {
        let out = run_cli(&args);
        assert_eq!(out.status.code(), Some(2), "parse error should exit 2");
        let doc = parse_json_stdout(&out);
        assert_eq!(doc.get("code").and_then(|v| v.as_str()), Some("error"));
        assert!(
            !doc.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .is_empty(),
            "error message should be present"
        );
    }
}

#[test]
fn exit_code_matrix_core_contract() {
    let ws = temp_workspace("exit-matrix");
    let pass = ws.join("pass.fozzy.json");
    let fail = ws.join("fail.fozzy.json");
    std::fs::write(&pass, fixture("example.fozzy.json")).expect("write pass");
    std::fs::write(&fail, fixture("fail.fozzy.json")).expect("write fail");

    let pass_out = run_cli(&[
        "run".into(),
        pass.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(pass_out.status.code(), Some(0), "pass run must exit 0");

    let fail_out = run_cli(&[
        "run".into(),
        fail.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(fail_out.status.code(), Some(1), "failing run must exit 1");

    let parse_err = run_cli(&["run".into(), "--json".into()]);
    assert_eq!(
        parse_err.status.code(),
        Some(2),
        "usage/parse errors must exit 2"
    );
}

#[test]
fn concurrent_same_root_runs_are_stable() {
    let ws = temp_workspace("concurrent-root");
    let scenario = ws.join("scenario.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    std::fs::write(ws.join("fozzy.toml"), "base_dir = \".fozzy\"\n").expect("write config");

    let mut handles = Vec::new();
    for _ in 0..8 {
        let scenario = scenario.clone();
        let ws = ws.clone();
        handles.push(thread::spawn(move || {
            run_cli(&[
                "run".into(),
                scenario.to_string_lossy().to_string(),
                "--cwd".into(),
                ws.to_string_lossy().to_string(),
                "--json".into(),
            ])
        }));
    }

    for h in handles {
        let out = h.join().expect("thread join");
        assert_eq!(
            out.status.code(),
            Some(0),
            "concurrent run failed: stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}
