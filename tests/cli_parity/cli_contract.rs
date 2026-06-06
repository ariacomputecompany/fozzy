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
fn artifacts_help_uses_run_or_trace_value_name() {
    for sub in ["pack", "export"] {
        let out = run_cli(&["artifacts".into(), sub.to_string(), "--help".into()]);
        assert_eq!(out.status.code(), Some(0), "help should exit 0");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("RUN_OR_TRACE"),
            "help should show RUN_OR_TRACE for artifacts {sub}; got: {stdout}"
        );
    }
}

#[test]
fn corpus_import_rejects_raw_duplicate_entries_in_strict_and_non_strict() {
    let ws = temp_workspace("corpus-dup-raw");
    let zip = ws.join("dup.zip");
    let out = ws.join("out");
    std::fs::create_dir_all(&out).expect("out");
    std::fs::write(
        &zip,
        build_zip_with_raw_entries(&[(b"same.txt", b"A"), (b"same.txt", b"B")]),
    )
    .expect("zip");

    for strict in [false, true] {
        let mut args = vec![
            "corpus".into(),
            "import".into(),
            zip.to_string_lossy().to_string(),
            "--out".into(),
            out.to_string_lossy().to_string(),
            "--json".into(),
        ];
        if strict {
            args.insert(0, "--strict".into());
        }
        let outp = run_cli(&args);
        assert_eq!(outp.status.code(), Some(2), "duplicate import must fail");
        let doc = parse_json_stdout(&outp);
        assert_eq!(doc.get("code").and_then(|v| v.as_str()), Some("error"));
        assert!(
            doc.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .contains("duplicate output file in archive is not allowed")
        );
    }
}

#[test]
fn corpus_import_rejects_raw_nul_collision_in_strict_and_non_strict() {
    let ws = temp_workspace("corpus-nul-raw");
    let zip = ws.join("nuldup.zip");
    let out = ws.join("out");
    std::fs::create_dir_all(&out).expect("out");
    std::fs::write(
        &zip,
        build_zip_with_raw_entries(&[(b"bad\0a.txt", b"A"), (b"bad", b"B")]),
    )
    .expect("zip");

    for strict in [false, true] {
        let mut args = vec![
            "corpus".into(),
            "import".into(),
            zip.to_string_lossy().to_string(),
            "--out".into(),
            out.to_string_lossy().to_string(),
            "--json".into(),
        ];
        if strict {
            args.insert(0, "--strict".into());
        }
        let outp = run_cli(&args);
        assert_eq!(
            outp.status.code(),
            Some(2),
            "nul collision import must fail"
        );
        let doc = parse_json_stdout(&outp);
        assert_eq!(doc.get("code").and_then(|v| v.as_str()), Some("error"));
        assert!(
            doc.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .contains("unsafe archive entry path rejected")
        );
    }
}

#[test]
fn ci_rejects_flake_budget_without_flake_runs() {
    let ws = temp_workspace("ci-budget");
    let trace = ws.join("trace.fozzy");
    let raw = r#"{
      "format":"fozzy-trace",
      "version":2,
      "engine":{"version":"0.1.0"},
      "mode":"run",
      "scenario_path":null,
      "scenario":{"version":1,"name":"x","steps":[]},
      "decisions":[],
      "events":[],
      "summary":{
        "status":"pass",
        "mode":"run",
        "identity":{"runId":"r1","seed":1},
        "startedAt":"2026-01-01T00:00:00Z",
        "finishedAt":"2026-01-01T00:00:00Z",
        "durationMs":0
      }
    }"#;
    std::fs::write(&trace, raw).expect("write trace");
    let trace_arg = trace.to_string_lossy().to_string();

    let normal = run_cli(&[
        "ci".into(),
        trace_arg.clone(),
        "--flake-budget".into(),
        "5".into(),
        "--json".into(),
    ]);
    assert_eq!(
        normal.status.code(),
        Some(2),
        "normal mode should reject misconfig"
    );

    let strict = run_cli(&[
        "--strict".into(),
        "ci".into(),
        trace_arg,
        "--flake-budget".into(),
        "5".into(),
        "--json".into(),
    ]);
    assert_eq!(
        strict.status.code(),
        Some(2),
        "strict mode should reject misconfig"
    );
}

#[test]
fn report_flaky_rejects_duplicate_inputs() {
    let ws = temp_workspace("flake-dup");
    let runs = ws.join(".fozzy").join("runs");
    std::fs::create_dir_all(&runs).expect("mkdir");

    let mk_report = |id: &str, status: &str| {
        let dir = runs.join(id);
        std::fs::create_dir_all(&dir).expect("run dir");
        let body = format!(
            r#"{{
  "status":"{status}",
  "mode":"run",
  "identity":{{"runId":"{id}","seed":1}},
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0
}}"#
        );
        std::fs::write(dir.join("report.json"), body).expect("write report");
    };
    mk_report("r1", "pass");
    mk_report("r2", "fail");

    std::fs::write(ws.join("fozzy.toml"), "base_dir = \".fozzy\"\n").expect("write config");
    let cfg = ws.join("fozzy.toml").to_string_lossy().to_string();
    let cwd = ws.to_string_lossy().to_string();

    let out = run_cli(&[
        "report".into(),
        "flaky".into(),
        "r1".into(),
        "r1".into(),
        "r2".into(),
        "--flake-budget".into(),
        "10".into(),
        "--cwd".into(),
        cwd,
        "--config".into(),
        cfg,
    ]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "duplicate runs should be rejected"
    );
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
fn steps_alias_matches_schema_output() {
    let schema = run_cli(&["schema".into(), "--json".into()]);
    assert_eq!(
        schema.status.code(),
        Some(0),
        "schema stderr={}",
        String::from_utf8_lossy(&schema.stderr)
    );
    let steps = run_cli(&["steps".into(), "--json".into()]);
    assert_eq!(
        steps.status.code(),
        Some(0),
        "steps alias stderr={}",
        String::from_utf8_lossy(&steps.stderr)
    );
    assert_eq!(parse_json_stdout(&schema), parse_json_stdout(&steps));
}

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

#[test]
fn map_hotspots_services_and_suites_emit_expected_schema() {
    let ws = temp_workspace("map-schema");
    let services_dir = ws.join("services").join("payments");
    let tests_dir = ws.join("tests");
    std::fs::create_dir_all(&services_dir).expect("services dir");
    std::fs::create_dir_all(&tests_dir).expect("tests dir");
    std::fs::write(
        services_dir.join("handler.rs"),
        r#"
        async fn handle_payment() {
            if retry { tokio::spawn(async move {}); }
            let _ = std::fs::read("config.toml");
            if timeout { panic!("failed"); }
        }
        "#,
    )
    .expect("write source");
    std::fs::write(
        tests_dir.join("handler.fozzy.json"),
        r#"{"version":1,"name":"handler","steps":[{"type":"trace_event","name":"x"}]}"#,
    )
    .expect("write scenario");

    let root = ws.to_string_lossy().to_string();
    let scenario_root = tests_dir.to_string_lossy().to_string();

    let hotspots = run_cli(&[
        "map".into(),
        "hotspots".into(),
        "--root".into(),
        root.clone(),
        "--min-risk".into(),
        "1".into(),
        "--limit".into(),
        "20".into(),
        "--json".into(),
    ]);
    assert_eq!(
        hotspots.status.code(),
        Some(0),
        "map hotspots stderr={}",
        String::from_utf8_lossy(&hotspots.stderr)
    );
    let hotspots_doc = parse_json_stdout(&hotspots);
    assert_eq!(
        hotspots_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.map_hotspots.v2"
    );
    assert!(
        hotspots_doc
            .get("hotspots")
            .and_then(|v| v.as_array())
            .is_some_and(|v| !v.is_empty())
    );

    let services = run_cli(&[
        "map".into(),
        "services".into(),
        "--root".into(),
        root.clone(),
        "--json".into(),
    ]);
    assert_eq!(
        services.status.code(),
        Some(0),
        "map services stderr={}",
        String::from_utf8_lossy(&services.stderr)
    );
    let services_doc = parse_json_stdout(&services);
    assert_eq!(
        services_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.map_services.v2"
    );

    let suites = run_cli(&[
        "map".into(),
        "suites".into(),
        "--root".into(),
        root,
        "--scenario-root".into(),
        scenario_root,
        "--min-risk".into(),
        "1".into(),
        "--json".into(),
    ]);
    assert_eq!(
        suites.status.code(),
        Some(0),
        "map suites stderr={}",
        String::from_utf8_lossy(&suites.stderr)
    );
    let suites_doc = parse_json_stdout(&suites);
    assert_eq!(
        suites_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.map_suites.v5"
    );
    assert!(
        suites_doc
            .get("suites")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|s| s.get("coverageEvidence"))
            .is_some(),
        "map suites should emit explainable coverage evidence"
    );
    assert_eq!(
        suites_doc
            .get("profile")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "pedantic"
    );
    assert_eq!(
        suites_doc
            .get("shrinkPolicy")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "no_known_failures"
    );
    assert!(
        suites_doc
            .get("requiredHotspotCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= suites_doc
                .get("coveredHotspotCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
    );
}

