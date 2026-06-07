use super::*;

#[test]
fn init_honors_custom_config_path() {
    let ws = temp_workspace("init-custom-config");
    let output = run_cli_in(
        &ws,
        &[
            "--config".into(),
            "custom.toml".into(),
            "init".into(),
            "--force".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "init should succeed");
    assert!(
        ws.join("custom.toml").exists(),
        "custom config should exist"
    );
    assert!(
        !ws.join("fozzy.toml").exists(),
        "default config path should not be created when custom path was requested"
    );
}

#[test]
fn run_record_collision_defaults_to_append_for_iterative_runs() {
    let ws = temp_workspace("run-record-append");
    let scenario = ws.join("pass.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    let record = ws.join("trace.fozzy");
    let args = vec![
        "run".to_string(),
        scenario.to_string_lossy().to_string(),
        "--record".to_string(),
        record.to_string_lossy().to_string(),
        "--json".to_string(),
    ];
    let first = run_cli(&args);
    assert_eq!(first.status.code(), Some(0));
    let second = run_cli(&args);
    assert_eq!(
        second.status.code(),
        Some(0),
        "second run should append by default, stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
}

#[test]
fn fuzz_supports_scenario_target() {
    let ws = temp_workspace("fuzz-scenario-target");
    let scenario = ws.join("app.pass.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    let out = run_cli(&[
        "fuzz".into(),
        format!("scenario:{}", scenario.display()),
        "--runs".into(),
        "1".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "fuzz scenario target should run, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(doc.get("mode").and_then(|v| v.as_str()), Some("fuzz"));
}
