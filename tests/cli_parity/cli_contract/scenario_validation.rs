use super::*;

#[test]
fn validate_returns_non_zero_with_actionable_parse_diagnostics() {
    let ws = temp_workspace("validate-parse-error");
    let scenario = ws.join("broken.fozzy.json");
    std::fs::write(
        &scenario,
        r#"{
          "version":1,
          "name":"broken",
          "steps":[
            {"type":"memory_alloc","bytes":"not-a-number"}
          ]
        }"#,
    )
    .expect("write broken scenario");

    let out = run_cli(&[
        "validate".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "validate should fail for malformed step payload: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(doc.get("ok").and_then(|v| v.as_bool()), Some(false));
    let msg = doc
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        msg.contains("failed to parse scenario"),
        "expected parse context in validate error, got: {msg}"
    );
    assert!(
        msg.contains("fozzy schema --json"),
        "expected schema guidance in validate error, got: {msg}"
    );
}

#[test]
fn validate_accepts_distributed_scenarios() {
    let ws = temp_workspace("validate-distributed");
    let scenario = ws.join("distributed.fozzy.json");
    std::fs::write(
        &scenario,
        r#"{
          "version":1,
          "name":"dist-ok",
          "distributed":{
            "node_count":3,
            "steps":[
              {"type":"client_put","node":"n0","key":"k","value":"v"},
              {"type":"tick","duration":"10ms"}
            ],
            "invariants":[{"type":"kv_present_on_all","key":"k"}]
          }
        }"#,
    )
    .expect("write distributed scenario");
    let out = run_cli(&[
        "validate".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(0));
    let doc = parse_json_stdout(&out);
    assert_eq!(doc.get("ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        doc.get("variant").and_then(|v| v.as_str()),
        Some("distributed")
    );
}

#[test]
fn validate_rejects_invalid_nested_steps() {
    let ws = temp_workspace("validate-nested-invalid");
    let scenario = ws.join("nested-invalid.fozzy.json");
    std::fs::write(
        &scenario,
        r#"{
          "version": 1,
          "name": "nested-invalid",
          "steps": [
            {
              "type": "assert_throws",
              "steps": [
                { "type": "sleep", "duration": "not-a-duration" }
              ]
            }
          ]
        }"#,
    )
    .expect("write scenario");

    let output = run_cli(&[
        "validate".into(),
        scenario.display().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "validate should fail nested invalid step"
    );
    let msg = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        msg.contains("not-a-duration"),
        "expected nested validation error, got: {msg}"
    );
}

#[test]
fn explore_rejects_invalid_distributed_scenario_missing_topology() {
    let ws = temp_workspace("explore-invalid-distributed");
    let scenario = ws.join("distributed-invalid.fozzy.json");
    std::fs::write(
        &scenario,
        r#"{
          "version": 1,
          "name": "distributed-invalid",
          "distributed": {
            "steps": [
              { "type": "tick", "duration": "1ms" }
            ],
            "invariants": []
          }
        }"#,
    )
    .expect("write scenario");

    let output = run_cli(&[
        "explore".into(),
        scenario.display().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "explore should reject invalid distributed scenario"
    );
    let out = parse_json_stdout(&output);
    let msg = out
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(
        msg.contains("distributed requires either nodes:[...] or node_count"),
        "expected distributed validation error, got: {msg}"
    );
}

#[test]
fn test_rejects_explicit_missing_scenario_path_even_if_other_inputs_exist() {
    let ws = temp_workspace("test-missing-explicit");
    let scenario = ws.join("ok.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");

    let output = run_cli_in(
        &ws,
        &[
            "test".into(),
            scenario.display().to_string(),
            ws.join("missing.fozzy.json").display().to_string(),
            "--det".into(),
            "--json".into(),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "test should reject missing explicit path"
    );
    let out = parse_json_stdout(&output);
    let msg = out
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(
        msg.contains("explicit scenario path(s) not found"),
        "expected missing explicit path error, got: {msg}"
    );
}

#[test]
fn test_rejects_distributed_scenarios_in_default_test_mode() {
    let ws = temp_workspace("test-distributed-reject");
    std::fs::write(ws.join("example.fozzy.json"), fixture("example.fozzy.json"))
        .expect("write example");
    std::fs::write(
        ws.join("distributed.fozzy.json"),
        r#"{
          "version": 1,
          "name": "distributed",
          "distributed": {
            "node_count": 2,
            "steps": [
              { "type": "tick", "duration": "1ms" }
            ],
            "invariants": []
          }
        }"#,
    )
    .expect("write distributed");

    let output = run_cli_in(
        &ws,
        &[
            "test".into(),
            "*.fozzy.json".into(),
            "--det".into(),
            "--json".into(),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "test should reject distributed scenarios"
    );
    let out = parse_json_stdout(&output);
    let msg = out
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(
        msg.contains("must be run with `fozzy explore`"),
        "expected distributed-scenario rejection, got: {msg}"
    );
}
