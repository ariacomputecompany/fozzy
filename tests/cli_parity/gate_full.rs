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
        hint.contains("Add a `proc_when` step"),
        "expected doctor hint to carry proc_when guidance, got: {hint}"
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
    assert!(detail.contains("Add a `proc_when` step"));
}

#[test]
fn full_allow_expected_failures_controls_shrink_status_for_fail_class_runs() {
    let ws = temp_workspace("full-allow-expected-failures");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("intentional-fail.fozzy.json"),
        r#"{
          "version":1,
          "name":"intentional-fail",
          "steps":[
            {"type":"trace_event","name":"start"},
            {"type":"fail","message":"expected failure"}
          ]
        }"#,
    )
    .expect("write fail scenario");

    let mut common = vec![
        "full".to_string(),
        "--scenario-root".to_string(),
        scenario_root.to_string_lossy().to_string(),
        "--seed".to_string(),
        "7".to_string(),
        "--doctor-runs".to_string(),
        "2".to_string(),
        "--fuzz-time".to_string(),
        "10ms".to_string(),
        "--required-steps".to_string(),
        "run_record_trace,replay,ci,shrink".to_string(),
        "--json".to_string(),
    ];

    let no_allow = run_cli(&common);
    assert_eq!(
        no_allow.status.code(),
        Some(1),
        "full should fail without --allow-expected-failures: {}",
        String::from_utf8_lossy(&no_allow.stderr)
    );
    let no_allow_doc = parse_json_stdout(&no_allow);
    assert_eq!(
        full_step_status(&no_allow_doc, "run_record_trace"),
        Some("failed".to_string())
    );
    assert_eq!(
        full_step_status(&no_allow_doc, "replay"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&no_allow_doc, "ci"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&no_allow_doc, "shrink"),
        Some("failed".to_string())
    );
    assert_eq!(
        full_step_status(&no_allow_doc, "replay_shrunk"),
        Some("skipped".to_string())
    );
    assert_eq!(
        no_allow_doc
            .get("shrinkClassification")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        Some("policy_rejected_non_pass".to_string())
    );

    common.insert(1, "--allow-expected-failures".to_string());
    let allow = run_cli(&common);
    assert_eq!(
        allow.status.code(),
        Some(1),
        "full should still fail when the primary scenario itself is non-pass: {}",
        String::from_utf8_lossy(&allow.stderr)
    );
    let allow_doc = parse_json_stdout(&allow);
    assert_eq!(
        full_step_status(&allow_doc, "run_record_trace"),
        Some("failed".to_string())
    );
    assert_eq!(
        full_step_status(&allow_doc, "replay"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&allow_doc, "ci"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&allow_doc, "shrink"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&allow_doc, "replay_shrunk"),
        Some("skipped".to_string())
    );
    assert_eq!(
        allow_doc
            .get("shrinkClassification")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        Some("expected_fail_class_preserved".to_string())
    );
}

#[test]
fn full_rejects_non_pass_primary_summaries_even_without_strict_warnings() {
    let ws = temp_workspace("full-non-pass-primary");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("fail.fozzy.json"),
        r#"{
          "version":1,
          "name":"fail-fast",
          "steps":[
            {"type":"assert_ok","value":false}
          ]
        }"#,
    )
    .expect("write fail scenario");

    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        scenario_root.to_string_lossy().to_string(),
        "--scenario-filter".into(),
        "fail.fozzy.json".into(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--required-steps".into(),
        "test_det,run_record_trace".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "full should fail for non-pass primary scenario"
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "test_det").as_deref(),
        Some("failed"),
        "test_det should fail when the test summary is non-pass"
    );
    assert_eq!(
        full_step_status(&doc, "run_record_trace").as_deref(),
        Some("failed"),
        "run_record_trace should fail when the recorded run summary is non-pass"
    );
    assert!(
        full_step_detail(&doc, "test_det")
            .unwrap_or_default()
            .contains("status=Fail"),
        "test_det detail should surface the underlying fail status"
    );
    assert!(
        full_step_detail(&doc, "run_record_trace")
            .unwrap_or_default()
            .contains("status=Fail"),
        "run_record_trace detail should surface the underlying fail status"
    );
}

#[test]
fn full_report_query_rejects_non_pass_primary_status() {
    let ws = temp_workspace("full-report-query-fail-status");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("fail.fozzy.json"),
        r#"{
          "version":1,
          "name":"fail-fast",
          "steps":[
            {"type":"assert_ok","value":false}
          ]
        }"#,
    )
    .expect("write fail scenario");

    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        scenario_root.to_string_lossy().to_string(),
        "--scenario-filter".into(),
        "fail.fozzy.json".into(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--required-steps".into(),
        "run_record_trace,report_query".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "full should fail for non-pass primary scenario"
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "report_query").as_deref(),
        Some("failed"),
        "report_query should fail when the queried run status is non-pass"
    );
    assert_eq!(
        full_step_detail(&doc, "report_query").as_deref(),
        Some(".status=fail"),
        "report_query should surface the queried fail status verbatim"
    );
}

#[test]
fn full_memory_graph_skips_empty_graph_evidence() {
    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        "tests".into(),
        "--scenario-filter".into(),
        "example.fozzy.json".into(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--required-steps".into(),
        "run_record_trace,memory_graph".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "full example flow should complete"
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "memory_graph").as_deref(),
        Some("skipped"),
        "memory_graph should skip when the graph payload has no nodes or edges"
    );
    assert_eq!(
        full_step_detail(&doc, "memory_graph").as_deref(),
        Some("nodes=0 edges=0"),
        "memory_graph should still report the empty graph evidence explicitly"
    );
}

#[test]
fn full_ci_surfaces_concrete_detail() {
    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        "tests".into(),
        "--scenario-filter".into(),
        "memory.pass".into(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--required-steps".into(),
        "run_record_trace,ci".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "full should complete for CI detail proof: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "ci").as_deref(),
        Some("passed"),
        "ci should pass on clean trace"
    );
    let detail = full_step_detail(&doc, "ci").expect("ci detail");
    assert!(
        detail.contains("checks=") && detail.contains("failed=<none>"),
        "ci detail should surface concrete check detail instead of a lossy count, got: {detail}"
    );
}

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
fn full_flags_conflict_with_required_steps_surfaces_policy_conflict() {
    let ws = temp_workspace("full-policy-conflict");
    let tests_dir = ws.join("tests");
    std::fs::create_dir_all(&tests_dir).expect("mkdir tests");
    std::fs::write(
        tests_dir.join("app.pass.fozzy.json"),
        fixture("example.fozzy.json"),
    )
    .expect("write scenario");
    let out = run_cli(&[
        "--cwd".into(),
        ws.to_string_lossy().to_string(),
        "full".into(),
        "--scenario-root".into(),
        "tests".into(),
        "--required-steps".into(),
        "usage,version,test_det".into(),
        "--require-topology-coverage".into(),
        ".".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "policy_conflict"),
        Some("failed".to_string())
    );
}

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
        matches!(profile_top_status.as_deref(), Some("passed") | Some("skipped")),
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

