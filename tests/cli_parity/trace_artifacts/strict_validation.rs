use super::*;

#[test]
fn strict_mode_fails_on_stale_trace_verify_warnings() {
    let ws = temp_workspace("strict");
    let trace = ws.join("stale.fozzy");
    let raw = r#"{
      "format":"fozzy-trace",
      "version":1,
      "engine":{"version":"0.1.0"},
      "mode":"run",
      "scenario_path":"tests/example.fozzy.json",
      "scenario":{"version":1,"name":"example","steps":[]},
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

    let ok = run_cli(&[
        "trace".into(),
        "verify".into(),
        trace_arg.clone(),
        "--json".into(),
        "--unsafe".into(),
    ]);
    assert_eq!(ok.status.code(), Some(0), "non-strict should pass");

    let strict = run_cli(&[
        "trace".into(),
        "verify".into(),
        trace_arg,
        "--json".into(),
        "--strict".into(),
    ]);
    assert_eq!(strict.status.code(), Some(2), "strict should fail");
}

#[test]
fn strict_rejects_checksumless_trace_in_verify_and_ci() {
    let ws = temp_workspace("strict-checksum");
    let trace = ws.join("no-checksum.fozzy");
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

    let strict_verify = run_cli(&[
        "--strict".into(),
        "trace".into(),
        "verify".into(),
        trace_arg.clone(),
        "--json".into(),
    ]);
    assert_eq!(
        strict_verify.status.code(),
        Some(2),
        "strict trace verify should fail"
    );

    let strict_ci = run_cli(&["--strict".into(), "ci".into(), trace_arg, "--json".into()]);
    assert_eq!(strict_ci.status.code(), Some(1), "strict ci should fail");
}

#[test]
fn strict_verify_accepts_trace_recorded_by_passing_deterministic_run() {
    let ws = temp_workspace("strict-verify-recorded-trace");
    let scenario = ws.join("example.fozzy.json");
    let trace = ws.join("recorded.trace.fozzy");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--record".into(),
            trace.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "deterministic run should pass: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let verify = run_cli_in(
        &ws,
        &[
            "trace".into(),
            "verify".into(),
            trace.display().to_string(),
            "--strict-verify".into(),
            "--json".into(),
        ],
    );
    assert_eq!(
        verify.status.code(),
        Some(0),
        "strict trace verify should pass for a freshly recorded deterministic trace: {}",
        String::from_utf8_lossy(&verify.stderr)
    );
    let doc = parse_json_stdout(&verify);
    assert_eq!(doc.get("ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        doc.get("checksumPresent").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        doc.get("checksumValid").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(
        doc.get("warnings")
            .and_then(|v| v.as_array())
            .is_none_or(|warnings| warnings.is_empty()),
        "fresh deterministic trace should not emit replay drift warnings"
    );
}

#[test]
fn strict_trace_verify_json_emits_single_error_document() {
    let ws = temp_workspace("strict-json-contract");
    let trace = ws.join("stale.fozzy");
    let raw = r#"{
      "format":"fozzy-trace",
      "version":1,
      "engine":{"version":"0.1.0"},
      "mode":"run",
      "scenario_path":"tests/example.fozzy.json",
      "scenario":{"version":1,"name":"example","steps":[]},
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

    let strict = run_cli(&[
        "--strict".into(),
        "trace".into(),
        "verify".into(),
        trace_arg,
        "--json".into(),
    ]);
    assert_eq!(strict.status.code(), Some(2), "strict should fail");

    let stdout = String::from_utf8_lossy(&strict.stdout);
    let doc: serde_json::Value = serde_json::from_str(stdout.trim()).expect("stdout json");
    assert_eq!(doc.get("code").and_then(|v| v.as_str()), Some("error"));
}

#[test]
fn invalid_trace_header_is_rejected_in_non_strict_verify_replay_and_ci() {
    let ws = temp_workspace("trace-header");
    let bad_format = ws.join("bad-format.fozzy");
    let bad_version = ws.join("bad-version.fozzy");

    let base = |format: &str, version: u32| -> String {
        format!(
            r#"{{
      "format":"{format}",
      "version":{version},
      "engine":{{"version":"0.1.0"}},
      "mode":"run",
      "scenario_path":null,
      "scenario":{{"version":1,"name":"x","steps":[]}},
      "decisions":[],
      "events":[],
      "summary":{{
        "status":"pass",
        "mode":"run",
        "identity":{{"runId":"r1","seed":1}},
        "startedAt":"2026-01-01T00:00:00Z",
        "finishedAt":"2026-01-01T00:00:00Z",
        "durationMs":0
      }}
    }}"#
        )
    };

    std::fs::write(&bad_format, base("fozzy-trace-vX", 2)).expect("write bad format");
    std::fs::write(&bad_version, base("fozzy-trace", 999)).expect("write bad version");

    let bad_format_arg = bad_format.to_string_lossy().to_string();
    let bad_version_arg = bad_version.to_string_lossy().to_string();

    let verify_bad_format = run_cli(&[
        "trace".into(),
        "verify".into(),
        bad_format_arg.clone(),
        "--json".into(),
    ]);
    assert_eq!(
        verify_bad_format.status.code(),
        Some(2),
        "trace verify must reject bad format in non-strict mode"
    );

    let replay_bad_version = run_cli(&["replay".into(), bad_version_arg.clone(), "--json".into()]);
    assert_eq!(
        replay_bad_version.status.code(),
        Some(2),
        "replay must reject bad version in non-strict mode"
    );

    let ci_bad_version = run_cli(&["ci".into(), bad_version_arg, "--json".into()]);
    assert_eq!(
        ci_bad_version.status.code(),
        Some(2),
        "ci must reject bad version in non-strict mode"
    );
}
