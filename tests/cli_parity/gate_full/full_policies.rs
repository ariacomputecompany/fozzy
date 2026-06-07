use super::*;

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
