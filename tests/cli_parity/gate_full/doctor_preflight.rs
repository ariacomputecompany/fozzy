use super::*;

#[test]
fn doctor_deep_preflights_proc_unmatched_scenarios() {
    let ws = temp_workspace("doctor-proc-preflight");
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
        "doctor".into(),
        "--deep".into(),
        "--scenario".into(),
        scenario.to_string_lossy().to_string(),
        "--runs".into(),
        "2".into(),
        "--seed".into(),
        "7".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(2), "strict doctor should fail");

    let doc = parse_first_json_stdout(&out);
    let issue = doc
        .get("issues")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .expect("doctor issue");
    assert_eq!(
        issue.get("code").and_then(|v| v.as_str()),
        Some("proc_unmatched_preflight")
    );
    let hint = issue
        .get("hint")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        hint.contains("Add a matching `proc_when` step"),
        "expected doctor hint to carry proc_when guidance, got: {hint}"
    );
    let details = issue.get("details").expect("doctor details");
    assert_eq!(
        details
            .get("suggestedProcWhen")
            .and_then(|v| v.get("cmd"))
            .and_then(|v| v.as_str()),
        Some("cargo")
    );
}

#[test]
fn gate_doctor_deep_surfaces_issue_detail() {
    let ws = temp_workspace("gate-doctor-detail");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("repo-owned.fozzy.json"),
        r#"{
          "version":1,
          "name":"repo-owned-proc",
          "steps":[
            {"type":"proc_spawn","cmd":"cargo","args":["test"]}
          ]
        }"#,
    )
    .expect("write scenario");

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
    assert_eq!(out.status.code(), Some(1), "gate should fail doctor_deep");
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "doctor_deep").as_deref(),
        Some("failed")
    );
    let detail = full_step_detail(&doc, "doctor_deep").expect("doctor detail");
    assert!(detail.contains("proc_unmatched_preflight"));
    assert!(detail.contains("Add a matching `proc_when` step"));
}
