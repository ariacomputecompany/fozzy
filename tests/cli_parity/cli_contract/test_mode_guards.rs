use super::*;

#[test]
fn test_strict_proc_unmatched_reports_actionable_stub_and_location() {
    let ws = temp_workspace("proc-unmatched-guidance");
    let scenario = ws.join("repo-owned.fozzy.json");
    std::fs::write(
        &scenario,
        r#"{
          "version":1,
          "name":"repo-owned-proc",
          "steps":[
            {"type":"proc_spawn","cmd":"cargo","args":["test"]}
          ]
        }"#,
    )
    .expect("write scenario");

    let out = run_cli(&[
        "test".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(1), "strict proc test should fail");

    let doc = parse_json_stdout(&out);
    let finding = doc
        .get("findings")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .expect("first finding");
    assert_eq!(
        finding.get("title").and_then(|v| v.as_str()),
        Some("proc_unmatched")
    );
    let msg = finding
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        msg.contains("Strict proc backend blocked an undeclared subprocess"),
        "expected higher-context headline, got: {msg}"
    );
    assert!(
        msg.contains("Add a `proc_when` step"),
        "expected concrete remediation, got: {msg}"
    );
    assert!(
        msg.contains("\"cmd\": \"cargo\""),
        "expected stub example for cargo, got: {msg}"
    );
    assert!(
        msg.contains("\"args\": [\"test\"]"),
        "expected args example, got: {msg}"
    );
    assert_eq!(
        finding
            .get("location")
            .and_then(|v| v.get("file"))
            .and_then(|v| v.as_str()),
        Some(scenario.to_string_lossy().as_ref())
    );
}

#[test]
fn test_rejects_aggregate_profile_capture_flag() {
    let ws = temp_workspace("test-profile-capture-reject");
    let scenario = ws.join("ok.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");

    let output = run_cli_in(
        &ws,
        &[
            "test".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--profile-capture".into(),
            "full".into(),
            "--json".into(),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "test should reject profile capture"
    );
    let out = parse_json_stdout(&output);
    let msg = out
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(
        msg.contains("unexpected argument '--profile-capture'"),
        "expected clap-level profile capture rejection, got: {msg}"
    );
}

#[test]
fn test_rejects_aggregate_memory_sidecar_flag() {
    let ws = temp_workspace("test-mem-artifacts-reject");
    let scenario = ws.join("ok.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");

    let output = run_cli_in(
        &ws,
        &[
            "test".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--mem-artifacts".into(),
            "--json".into(),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(2),
        "test should reject mem artifacts"
    );
    let out = parse_json_stdout(&output);
    let msg = out
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(
        msg.contains("unexpected argument '--mem-artifacts'"),
        "expected clap-level memory artifact rejection, got: {msg}"
    );
}
