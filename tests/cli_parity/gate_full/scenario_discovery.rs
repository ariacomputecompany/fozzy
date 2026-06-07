use super::*;

#[test]
fn full_fails_when_no_scenarios_are_discovered() {
    let ws = temp_workspace("full-no-scenarios");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");

    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        scenario_root.to_string_lossy().to_string(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "full should fail when no scenarios are discovered: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "discover_scenarios"),
        Some("failed".to_string())
    );
    assert!(
        full_step_detail(&doc, "discover_scenarios")
            .unwrap_or_default()
            .contains("step_scenarios=0"),
        "discover_scenarios should report empty discovery detail"
    );
}

#[test]
fn full_fails_discover_when_only_distributed_scenarios_exist() {
    let ws = temp_workspace("full-distributed-only");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("distributed.fozzy.json"),
        r#"{
          "version":1,
          "name":"distributed-only",
          "distributed":{
            "node_count":2,
            "steps":[{"type":"tick","duration":"1ms"}],
            "invariants":[]
          }
        }"#,
    )
    .expect("write distributed scenario");

    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        scenario_root.to_string_lossy().to_string(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "full should fail when only distributed scenarios are discovered: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "discover_scenarios").as_deref(),
        Some("failed"),
        "discover_scenarios should fail when there are no executable step scenarios"
    );
    assert!(
        full_step_detail(&doc, "discover_scenarios")
            .unwrap_or_default()
            .contains("step_scenarios=0 distributed_scenarios=1"),
        "discover_scenarios should report the distributed-only shape"
    );
    assert!(
        doc.get("guidance")
            .and_then(|v| v.as_array())
            .is_some_and(|items| items
                .iter()
                .filter_map(|v| v.as_str())
                .any(|s| s.contains("distributed-only roots cannot exercise"))),
        "full guidance should explain why distributed-only roots are insufficient"
    );
}

#[test]
fn gate_fails_when_no_scenarios_are_discovered() {
    let ws = temp_workspace("gate-no-scenarios");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--seed",
            "7",
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");
    assert_eq!(
        out.status.code(),
        Some(1),
        "gate should fail when no scenarios are discovered: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "discover").as_deref(),
        Some("failed"),
        "discover should fail on an empty scenario root"
    );
    assert_eq!(
        full_step_status(&doc, "scope_match").as_deref(),
        Some("failed"),
        "scope_match should also fail when no step scenarios are available"
    );
}

#[test]
fn gate_fails_discover_when_only_distributed_scenarios_exist() {
    let ws = temp_workspace("gate-distributed-only");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("distributed.fozzy.json"),
        r#"{
          "version":1,
          "name":"distributed-only",
          "distributed":{
            "node_count":2,
            "steps":[{"type":"tick","duration":"1ms"}],
            "invariants":[]
          }
        }"#,
    )
    .expect("write distributed scenario");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--seed",
            "7",
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");
    assert_eq!(
        out.status.code(),
        Some(1),
        "gate should fail when only distributed scenarios are discovered: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "discover").as_deref(),
        Some("failed"),
        "discover should fail when gate has no executable step scenarios"
    );
    assert!(
        full_step_detail(&doc, "discover")
            .unwrap_or_default()
            .contains("step_scenarios=0 distributed_scenarios=1"),
        "discover should report that only distributed scenarios were found"
    );
    assert_eq!(
        full_step_status(&doc, "scope_match").as_deref(),
        Some("failed"),
        "scope_match should still fail when no step scenarios are available"
    );
}
