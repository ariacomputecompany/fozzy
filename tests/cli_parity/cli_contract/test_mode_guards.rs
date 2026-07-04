use super::*;

#[test]
fn test_strict_proc_unmatched_reports_structured_scaffold_and_location() {
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
        msg.contains("Add a matching `proc_when` step"),
        "expected concrete remediation, got: {msg}"
    );
    assert_eq!(
        finding
            .get("location")
            .and_then(|v| v.get("file"))
            .and_then(|v| v.as_str()),
        Some(scenario.to_string_lossy().as_ref())
    );
    let details = finding
        .get("location")
        .and_then(|v| v.get("details"))
        .expect("proc scaffold details");
    assert_eq!(
        details
            .get("suggestedProcWhen")
            .and_then(|v| v.get("cmd"))
            .and_then(|v| v.as_str()),
        Some("cargo")
    );
    assert_eq!(
        details
            .get("suggestedProcWhen")
            .and_then(|v| v.get("args"))
            .and_then(|v| v.as_array())
            .and_then(|args| args.first())
            .and_then(|v| v.as_str()),
        Some("test")
    );
    assert_eq!(details.get("stepIndex").and_then(|v| v.as_u64()), Some(0));
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
fn test_accepts_aggregate_memory_sidecar_flag() {
    let ws = temp_workspace("test-mem-artifacts-accept");
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
        Some(0),
        "test should accept mem artifacts, stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        parse_json_stdout(&output)
            .get("status")
            .and_then(|v| v.as_str())
            == Some("pass")
    );
}

#[test]
fn run_rejects_declaration_only_proc_when_scenarios() {
    let ws = temp_workspace("proc-when-only");
    let scenario = ws.join("proc-when-only.fozzy.json");
    std::fs::write(
        &scenario,
        r#"{
          "version":1,
          "name":"proc-when-only",
          "steps":[
            {"type":"proc_when","cmd":"/bin/echo","args":["ok"],"exit_code":0,"stdout":"ok\n","stderr":""}
          ]
        }"#,
    )
    .expect("write scenario");

    let out = run_cli(&[
        "--proc-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "declaration-only scenario should fail validation, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    let msg = doc
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    assert!(
        msg.contains("`proc_when` and `http_when` are declarations"),
        "expected declaration-only authoring guidance, got: {msg}"
    );
    assert!(
        msg.contains("Add executable steps such as `proc_spawn`"),
        "expected actionable executable-step guidance, got: {msg}"
    );
}

#[test]
fn test_accepts_mem_artifacts_flag() {
    let ws = temp_workspace("test-mem-artifacts-flag");
    let scenario = ws.join("simple.fozzy.json");
    std::fs::write(
        &scenario,
        r#"{"version":1,"name":"simple","steps":[{"type":"assert_ok","value":true}]}"#,
    )
    .expect("write scenario");

    let out = run_cli(&[
        "test".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--mem-track".into(),
        "--mem-artifacts".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "mem-artifacts should parse for test mode, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}
