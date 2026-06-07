use super::*;

#[test]
fn gate_targeted_profile_runs_scoped_strict_bundle() {
    let ws = temp_workspace("gate-targeted");
    let tests_dir = ws.join("tests");
    std::fs::create_dir_all(&tests_dir).expect("mkdir tests");
    std::fs::write(
        tests_dir.join("gateway.pass.fozzy.json"),
        br#"{
  "version": 1,
  "name": "gateway-pass",
  "steps": [
    { "type": "assert_eq_int", "a": 1, "b": 1 }
  ]
}"#,
    )
    .expect("write gateway scenario");
    std::fs::write(
        tests_dir.join("other.pass.fozzy.json"),
        br#"{
  "version": 1,
  "name": "other-pass",
  "steps": [
    { "type": "assert_eq_int", "a": 2, "b": 2 }
  ]
}"#,
    )
    .expect("write other scenario");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            tests_dir.to_str().expect("tests str"),
            "--scope",
            "gateway",
            "--seed",
            "1337",
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");
    assert_eq!(
        out.status.code(),
        Some(0),
        "gate stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        doc.get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.gate_report.v1"
    );
    assert_eq!(
        doc.get("profile")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "targeted"
    );
    assert_eq!(
        doc.get("matchedScenarios")
            .and_then(|v| v.as_array())
            .map(|v| v.len())
            .unwrap_or_default(),
        1
    );
    assert_eq!(
        full_step_status(&doc, "clean_tree").as_deref(),
        Some("skipped"),
        "clean_tree should be skipped outside a git repo"
    );
    let profile_top = full_step_detail(&doc, "profile_top").expect("profile_top detail");
    let profile_top_status = full_step_status(&doc, "profile_top");
    assert!(
        matches!(
            profile_top_status.as_deref(),
            Some("passed") | Some("skipped")
        ),
        "profile_top should either surface concrete data or skip when requested domains are empty"
    );
    assert!(
        profile_top.contains("warnings=<none>")
            && profile_top.contains("empty_domains=")
            && profile_top.contains("heap:no heap samples in trace")
            && profile_top.contains("io:no io events in trace"),
        "profile_top should report concrete profile evidence, got: {profile_top}"
    );
    let trace_verify = full_step_detail(&doc, "trace_verify").expect("trace_verify detail");
    assert!(
        trace_verify.contains("warnings=<none>"),
        "trace_verify should report concrete warning detail, got: {trace_verify}"
    );
    let profile_diff = full_step_detail(&doc, "profile_diff").expect("profile_diff detail");
    assert!(
        profile_diff.contains("verdict=")
            && profile_diff.contains("regressions=")
            && profile_diff.contains("significant_regressions="),
        "profile_diff should report concrete diff evidence, got: {profile_diff}"
    );
    let profile_explain =
        full_step_detail(&doc, "profile_explain").expect("profile_explain detail");
    let profile_explain_status = full_step_status(&doc, "profile_explain");
    if profile_explain.contains("cause_domain=unknown")
        || profile_explain.contains("shifted_path=n/a")
    {
        assert_eq!(
            profile_explain_status.as_deref(),
            Some("skipped"),
            "profile_explain should skip when no concrete diagnosis exists"
        );
    } else {
        assert_eq!(
            profile_explain_status.as_deref(),
            Some("passed"),
            "profile_explain should pass when it reports a concrete diagnosis"
        );
    }
    assert!(
        profile_explain.contains("cause_domain=") && profile_explain.contains("shifted_path="),
        "profile_explain should report concrete explain evidence, got: {profile_explain}"
    );
}

#[test]
fn gate_rejects_non_pass_primary_summaries_even_without_strict_warnings() {
    let ws = temp_workspace("gate-non-pass-primary");
    let tests_dir = ws.join("tests");
    std::fs::create_dir_all(&tests_dir).expect("mkdir tests");
    std::fs::write(
        tests_dir.join("fail.fozzy.json"),
        r#"{
          "version":1,
          "name":"fail-fast",
          "steps":[
            {"type":"assert_ok","value":false}
          ]
        }"#,
    )
    .expect("write fail scenario");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            tests_dir.to_str().expect("tests str"),
            "--scope",
            "fail.fozzy.json",
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
        "gate should fail for non-pass primary scenario"
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "test_det_strict").as_deref(),
        Some("failed"),
        "test_det_strict should fail when the suite summary is non-pass"
    );
    assert_eq!(
        full_step_status(&doc, "run_record_trace").as_deref(),
        Some("failed"),
        "run_record_trace should fail when the recorded run summary is non-pass"
    );
    assert!(
        full_step_detail(&doc, "test_det_strict")
            .unwrap_or_default()
            .contains("status=Fail"),
        "test_det_strict detail should surface the underlying fail status"
    );
    assert!(
        full_step_detail(&doc, "run_record_trace")
            .unwrap_or_default()
            .contains("status=Fail"),
        "run_record_trace detail should surface the underlying fail status"
    );
}
