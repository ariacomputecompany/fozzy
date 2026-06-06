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

#[cfg(unix)]
#[test]
fn shrink_preserves_real_duration_in_output_trace() {
    let ws = temp_workspace("host-proc-shrink-duration");
    let scenario = ws.join("host-proc-shrink-duration.fozzy.json");
    let trace = ws.join("host-proc-shrink-duration.fozzy");
    let shrunk = ws.join("host-proc-shrink-duration.min.fozzy");
    let raw = r#"{
      "version":1,
      "name":"host-proc-shrink-duration",
      "steps":[
        {"type":"proc_spawn","cmd":"/bin/sh","args":["-lc","sleep 1; echo done"],"expect_exit":0,"expect_stdout":"done\n"}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");

    let run = run_cli(&[
        "--proc-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        run.status.code(),
        Some(0),
        "host shrink source run should pass"
    );

    let shrink = run_cli(&[
        "shrink".into(),
        trace.to_string_lossy().to_string(),
        "--minimize".into(),
        "all".into(),
        "--out".into(),
        shrunk.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(shrink.status.code(), Some(0), "shrink should pass");

    let shrunk_doc = read_trace_json(&shrunk);
    let summary_ms = shrunk_doc
        .get("summary")
        .and_then(|v| v.get("durationMs"))
        .and_then(|v| v.as_u64())
        .expect("shrunk trace summary duration");
    assert!(
        summary_ms >= 900,
        "expected shrunk trace summary duration to preserve real runtime evidence, got {summary_ms}"
    );
}

#[test]
fn replay_fuzz_report_references_actual_trace_path() {
    let ws = temp_workspace("replay-fuzz-trace-path");
    let trace = ws.join("example-fuzz.fozzy");

    let fuzz = run_cli(&[
        "fuzz".into(),
        "scenario:tests/example.fozzy.json".into(),
        "--det".into(),
        "--runs".into(),
        "1".into(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(fuzz.status.code(), Some(0), "fuzz should pass");

    let replay = run_cli(&[
        "replay".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(replay.status.code(), Some(0), "replay should pass");
    let doc = parse_json_stdout(&replay);
    assert_eq!(
        doc.get("identity")
            .and_then(|v| v.get("tracePath"))
            .and_then(|v| v.as_str()),
        Some(trace.to_string_lossy().as_ref())
    );
}

#[test]
fn replay_explore_report_references_actual_trace_path() {
    let ws = temp_workspace("replay-explore-trace-path");
    let trace = ws.join("kv-explore.fozzy");

    let explore = run_cli(&[
        "explore".into(),
        "tests/kv.explore.fozzy.json".into(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(explore.status.code(), Some(0), "explore should pass");

    let replay = run_cli(&[
        "replay".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(replay.status.code(), Some(0), "replay should pass");
    let doc = parse_json_stdout(&replay);
    assert_eq!(
        doc.get("identity")
            .and_then(|v| v.get("tracePath"))
            .and_then(|v| v.as_str()),
        Some(trace.to_string_lossy().as_ref())
    );
}

#[test]
fn run_recorded_trace_embeds_actual_written_trace_path() {
    let ws = temp_workspace("run-trace-metadata");
    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    let requested = ws.join("trace.fozzy");
    std::fs::write(&requested, b"old").expect("seed collision");

    let output = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "run should succeed");
    let out = parse_json_stdout(&output);
    let trace_path = out
        .get("identity")
        .and_then(|v| v.get("tracePath"))
        .and_then(|v| v.as_str())
        .expect("trace path")
        .to_string();
    let trace_doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&trace_path).expect("read trace"))
            .expect("trace json");
    let embedded = trace_doc
        .get("summary")
        .and_then(|v| v.get("identity"))
        .and_then(|v| v.get("tracePath"))
        .and_then(|v| v.as_str())
        .expect("embedded trace path");
    assert_eq!(embedded, trace_path);
}

#[test]
fn run_recorded_trace_shares_report_identity() {
    let ws = temp_workspace("run-trace-report-identity");
    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    let requested = ws.join("trace.fozzy");

    let output = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "run should succeed");
    let out = parse_json_stdout(&output);
    let trace_path = out
        .get("identity")
        .and_then(|v| v.get("tracePath"))
        .and_then(|v| v.as_str())
        .expect("trace path")
        .to_string();
    let trace_doc = read_trace_json(Path::new(&trace_path));
    let trace_identity = trace_doc
        .get("summary")
        .and_then(|v| v.get("identity"))
        .expect("trace identity");
    assert_eq!(
        trace_identity.get("runId").and_then(|v| v.as_str()),
        out.get("identity")
            .and_then(|v| v.get("runId"))
            .and_then(|v| v.as_str())
    );
    assert_eq!(
        trace_identity.get("reportPath").and_then(|v| v.as_str()),
        out.get("identity")
            .and_then(|v| v.get("reportPath"))
            .and_then(|v| v.as_str())
    );
    assert_eq!(
        trace_identity.get("artifactsDir").and_then(|v| v.as_str()),
        out.get("identity")
            .and_then(|v| v.get("artifactsDir"))
            .and_then(|v| v.as_str())
    );
    let report_path = resolve_output_path(
        &ws,
        out.get("identity")
            .and_then(|v| v.get("reportPath"))
            .and_then(|v| v.as_str())
            .expect("report path"),
    );
    let report_doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report_path).expect("read report"))
            .expect("report json");
    assert_eq!(
        report_doc
            .get("identity")
            .and_then(|v| v.get("tracePath"))
            .and_then(|v| v.as_str()),
        Some(trace_path.as_str())
    );
}

#[test]
fn run_recorded_trace_emits_profile_source_provenance() {
    let ws = temp_workspace("run-profile-source-provenance");
    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    let requested = ws.join("trace.fozzy");

    let output = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--profile-capture".into(),
            "full".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "run should succeed");
    let out = parse_json_stdout(&output);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    let source: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts_dir.join("profile.source.json")).expect("read source"),
    )
    .expect("source json");
    assert_eq!(
        source.get("tracePath").and_then(|v| v.as_str()),
        Some(
            std::fs::canonicalize(&requested)
                .expect("canonicalize trace")
                .to_string_lossy()
                .as_ref()
        )
    );
    assert_eq!(
        source.get("runId").and_then(|v| v.as_str()),
        out.get("identity")
            .and_then(|v| v.get("runId"))
            .and_then(|v| v.as_str())
    );
}

#[test]
fn test_recorded_traces_are_standalone_and_do_not_reuse_aggregate_identity() {
    let ws = temp_workspace("test-recorded-trace-identity");
    let first = ws.join("first.fozzy.json");
    let second = ws.join("second.fozzy.json");
    std::fs::write(&first, fixture("example.fozzy.json")).expect("write first scenario");
    std::fs::write(&second, fixture("example.fozzy.json")).expect("write second scenario");
    let requested = ws.join("test.fozzy");

    let output = run_cli_in(
        &ws,
        &[
            "test".into(),
            "--det".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
            first.display().to_string(),
            second.display().to_string(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "test should succeed");

    let out = parse_json_stdout(&output);
    let aggregate_run_id = out
        .get("identity")
        .and_then(|v| v.get("runId"))
        .and_then(|v| v.as_str())
        .expect("aggregate run id");

    let first_trace: serde_json::Value =
        serde_json::from_slice(&std::fs::read(ws.join("test.1.fozzy")).expect("read first trace"))
            .expect("first trace json");
    let second_trace: serde_json::Value =
        serde_json::from_slice(&std::fs::read(ws.join("test.2.fozzy")).expect("read second trace"))
            .expect("second trace json");

    let first_identity = first_trace
        .get("summary")
        .and_then(|v| v.get("identity"))
        .expect("first identity");
    let second_identity = second_trace
        .get("summary")
        .and_then(|v| v.get("identity"))
        .expect("second identity");

    let first_run_id = first_identity
        .get("runId")
        .and_then(|v| v.as_str())
        .expect("first run id");
    let second_run_id = second_identity
        .get("runId")
        .and_then(|v| v.as_str())
        .expect("second run id");

    assert_ne!(first_run_id, aggregate_run_id);
    assert_ne!(second_run_id, aggregate_run_id);
    assert_ne!(first_run_id, second_run_id);
    assert!(first_identity.get("reportPath").is_none());
    assert!(first_identity.get("artifactsDir").is_none());
    assert!(second_identity.get("reportPath").is_none());
    assert!(second_identity.get("artifactsDir").is_none());
}

#[test]
fn ci_accepts_one_trace_from_a_multi_trace_recording_directory() {
    let ws = temp_workspace("test-recorded-trace-ci");
    let first = ws.join("first.fozzy.json");
    let second = ws.join("second.fozzy.json");
    std::fs::write(&first, fixture("example.fozzy.json")).expect("write first scenario");
    std::fs::write(&second, fixture("example.fozzy.json")).expect("write second scenario");
    let requested = ws.join("test.fozzy");

    let test_output = run_cli_in(
        &ws,
        &[
            "test".into(),
            "--det".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
            first.display().to_string(),
            second.display().to_string(),
        ],
    );
    assert_eq!(test_output.status.code(), Some(0), "test should succeed");

    let ci_output = run_cli_in(
        &ws,
        &[
            "ci".into(),
            ws.join("test.1.fozzy").display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(ci_output.status.code(), Some(0), "ci should succeed");
    let ci = parse_json_stdout(&ci_output);
    assert_eq!(ci.get("ok").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn artifacts_run_id_uses_external_recorded_trace_identity() {
    let ws = temp_workspace("artifacts-external-trace");
    let requested = ws.join("external.trace.fozzy");

    let output = run_cli_in(
        &ws,
        &[
            "run".into(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/memory.pass.fozzy.json")
                .display()
                .to_string(),
            "--det".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "run should succeed");
    let out = parse_json_stdout(&output);
    let run_id = out
        .get("identity")
        .and_then(|v| v.get("runId"))
        .and_then(|v| v.as_str())
        .expect("run id")
        .to_string();

    let ls = run_cli_in(
        &ws,
        &[
            "artifacts".into(),
            "ls".into(),
            run_id.clone(),
            "--json".into(),
        ],
    );
    assert_eq!(
        ls.status.code(),
        Some(0),
        "artifacts ls stderr={}",
        String::from_utf8_lossy(&ls.stderr)
    );
    let ls_doc = parse_json_stdout(&ls);
    let trace_entry = ls_doc
        .get("entries")
        .and_then(|v| v.as_array())
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.get("kind").and_then(|v| v.as_str()) == Some("trace"))
        })
        .and_then(|entry| entry.get("path"))
        .and_then(|v| v.as_str())
        .expect("trace entry path");
    assert_eq!(
        std::fs::canonicalize(trace_entry).expect("canonicalize listed trace"),
        std::fs::canonicalize(&requested).expect("canonicalize requested trace")
    );

    let memory = run_cli_in(
        &ws,
        &["memory".into(), "top".into(), run_id, "--json".into()],
    );
    assert_eq!(
        memory.status.code(),
        Some(0),
        "memory top stderr={}",
        String::from_utf8_lossy(&memory.stderr)
    );
    let memory_doc = parse_json_stdout(&memory);
    assert_eq!(
        memory_doc.get("total").and_then(|v| v.as_u64()),
        Some(0),
        "external recorded run id should resolve trace-backed memory summary"
    );
}

#[test]
fn fuzz_recorded_trace_shares_report_identity() {
    let ws = temp_workspace("fuzz-trace-report-identity");
    let requested = ws.join("fuzz.trace.fozzy");

    let output = run_cli_in(
        &ws,
        &[
            "fuzz".into(),
            format!(
                "scenario:{}",
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/memory.pass.fozzy.json")
                    .display()
            ),
            "--det".into(),
            "--runs".into(),
            "1".into(),
            "--seed".into(),
            "7".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "fuzz should succeed");
    let out = parse_json_stdout(&output);
    let trace_path = out
        .get("identity")
        .and_then(|v| v.get("tracePath"))
        .and_then(|v| v.as_str())
        .expect("trace path")
        .to_string();
    let report_identity = out
        .get("identity")
        .and_then(|v| v.get("reportPath"))
        .and_then(|v| v.as_str())
        .expect("report path")
        .to_string();
    let report_path = resolve_output_path(
        &ws,
        out.get("identity")
            .and_then(|v| v.get("reportPath"))
            .and_then(|v| v.as_str())
            .expect("report path"),
    );
    let trace_doc = read_trace_json(Path::new(&trace_path));
    let trace_identity = trace_doc
        .get("summary")
        .and_then(|v| v.get("identity"))
        .expect("trace identity");
    let report_doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report_path).expect("read report"))
            .expect("report json");
    assert_eq!(
        trace_identity.get("tracePath").and_then(|v| v.as_str()),
        Some(trace_path.as_str())
    );
    assert_eq!(
        trace_identity.get("reportPath").and_then(|v| v.as_str()),
        Some(report_identity.as_str())
    );
    assert_eq!(
        report_doc
            .get("identity")
            .and_then(|v| v.get("tracePath"))
            .and_then(|v| v.as_str()),
        Some(trace_path.as_str())
    );
}

#[test]
fn fuzz_recorded_trace_emits_profile_source_provenance() {
    let ws = temp_workspace("fuzz-profile-source-provenance");
    let requested = ws.join("fuzz.trace.fozzy");

    let output = run_cli_in(
        &ws,
        &[
            "fuzz".into(),
            format!(
                "scenario:{}",
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/memory.pass.fozzy.json")
                    .display()
            ),
            "--det".into(),
            "--runs".into(),
            "1".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "fuzz should succeed");
    let out = parse_json_stdout(&output);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    let source: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts_dir.join("profile.source.json")).expect("read source"),
    )
    .expect("source json");
    assert_eq!(
        source.get("tracePath").and_then(|v| v.as_str()),
        Some(
            std::fs::canonicalize(&requested)
                .expect("canonicalize trace")
                .to_string_lossy()
                .as_ref()
        )
    );
    assert_eq!(
        source.get("runId").and_then(|v| v.as_str()),
        out.get("identity")
            .and_then(|v| v.get("runId"))
            .and_then(|v| v.as_str())
    );
}

#[test]
fn explore_recorded_trace_shares_report_identity() {
    let ws = temp_workspace("explore-trace-report-identity");
    let requested = ws.join("explore.trace.fozzy");

    let output = run_cli_in(
        &ws,
        &[
            "explore".into(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/kv.explore.fozzy.json")
                .display()
                .to_string(),
            "--steps".into(),
            "10".into(),
            "--seed".into(),
            "7".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "explore should succeed");
    let out = parse_json_stdout(&output);
    let trace_path = out
        .get("identity")
        .and_then(|v| v.get("tracePath"))
        .and_then(|v| v.as_str())
        .expect("trace path")
        .to_string();
    let report_identity = out
        .get("identity")
        .and_then(|v| v.get("reportPath"))
        .and_then(|v| v.as_str())
        .expect("report path")
        .to_string();
    let report_path = resolve_output_path(
        &ws,
        out.get("identity")
            .and_then(|v| v.get("reportPath"))
            .and_then(|v| v.as_str())
            .expect("report path"),
    );
    let trace_doc = read_trace_json(Path::new(&trace_path));
    let trace_identity = trace_doc
        .get("summary")
        .and_then(|v| v.get("identity"))
        .expect("trace identity");
    let report_doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&report_path).expect("read report"))
            .expect("report json");
    assert_eq!(
        trace_identity.get("tracePath").and_then(|v| v.as_str()),
        Some(trace_path.as_str())
    );
    assert_eq!(
        trace_identity.get("reportPath").and_then(|v| v.as_str()),
        Some(report_identity.as_str())
    );
    assert_eq!(
        report_doc
            .get("identity")
            .and_then(|v| v.get("tracePath"))
            .and_then(|v| v.as_str()),
        Some(trace_path.as_str())
    );
}

#[test]
fn explore_recorded_trace_emits_profile_source_provenance() {
    let ws = temp_workspace("explore-profile-source-provenance");
    let requested = ws.join("explore.trace.fozzy");

    let output = run_cli_in(
        &ws,
        &[
            "explore".into(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/kv.explore.fozzy.json")
                .display()
                .to_string(),
            "--steps".into(),
            "10".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--record".into(),
            requested.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "explore should succeed");
    let out = parse_json_stdout(&output);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    let source: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts_dir.join("profile.source.json")).expect("read source"),
    )
    .expect("source json");
    assert_eq!(
        source.get("tracePath").and_then(|v| v.as_str()),
        Some(
            std::fs::canonicalize(&requested)
                .expect("canonicalize trace")
                .to_string_lossy()
                .as_ref()
        )
    );
    assert_eq!(
        source.get("runId").and_then(|v| v.as_str()),
        out.get("identity")
            .and_then(|v| v.get("runId"))
            .and_then(|v| v.as_str())
    );
}

#[test]
fn run_manifest_refreshes_profile_capabilities_after_artifact_emit() {
    let ws = temp_workspace("run-manifest-profile-capabilities");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("write scenario");

    let output = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "run should succeed");
    let out = parse_json_stdout(&output);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts_dir.join("manifest.json")).expect("read manifest"),
    )
    .expect("manifest json");
    let caps = manifest
        .get("profileCapabilities")
        .and_then(|v| v.as_array())
        .expect("profile capabilities");
    assert!(
        caps.iter().any(|v| v.as_str() == Some("metrics")),
        "manifest should include metrics capability after profile artifact emit"
    );
}

#[test]
fn fuzz_manifest_refreshes_profile_capabilities_after_artifact_emit() {
    let ws = temp_workspace("fuzz-manifest-profile-capabilities");
    let output = run_cli_in(
        &ws,
        &[
            "fuzz".into(),
            format!(
                "scenario:{}",
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/memory.pass.fozzy.json")
                    .display()
            ),
            "--det".into(),
            "--runs".into(),
            "1".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "fuzz should succeed");
    let out = parse_json_stdout(&output);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts_dir.join("manifest.json")).expect("read manifest"),
    )
    .expect("manifest json");
    let caps = manifest
        .get("profileCapabilities")
        .and_then(|v| v.as_array())
        .expect("profile capabilities");
    assert!(
        caps.iter().any(|v| v.as_str() == Some("metrics")),
        "manifest should include metrics capability after profile artifact emit"
    );
}

#[test]
fn explore_manifest_refreshes_profile_capabilities_after_artifact_emit() {
    let ws = temp_workspace("explore-manifest-profile-capabilities");
    let output = run_cli_in(
        &ws,
        &[
            "explore".into(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/kv.explore.fozzy.json")
                .display()
                .to_string(),
            "--steps".into(),
            "10".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "explore should succeed");
    let out = parse_json_stdout(&output);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts_dir.join("manifest.json")).expect("read manifest"),
    )
    .expect("manifest json");
    let caps = manifest
        .get("profileCapabilities")
        .and_then(|v| v.as_array())
        .expect("profile capabilities");
    assert!(
        caps.iter().any(|v| v.as_str() == Some("metrics")),
        "manifest should include metrics capability after profile artifact emit"
    );
}

#[test]
fn replay_embedded_trace_without_scenario_path_reports_real_trace_file_location() {
    let ws = temp_workspace("replay-embedded-trace-location");
    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    let trace = ws.join("embedded-trace.fozzy");

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
    assert_eq!(run.status.code(), Some(0), "run should succeed");

    let mut trace_doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&trace).expect("read trace")).expect("trace json");
    trace_doc["scenario_path"] = serde_json::Value::Null;
    trace_doc["scenario"]["steps"][3] = serde_json::json!({
        "type": "proc_spawn",
        "cmd": "echo",
        "args": ["drift"],
        "expect_exit": 0
    });
    trace_doc["checksum"] = serde_json::Value::Null;
    std::fs::write(
        &trace,
        serde_json::to_vec_pretty(&trace_doc).expect("rewrite trace"),
    )
    .expect("write trace");

    let replay = run_cli_in(
        &ws,
        &[
            "replay".into(),
            trace.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(replay.status.code(), Some(1), "replay should fail");
    let doc = parse_json_stdout(&replay);
    assert_eq!(
        doc.get("findings")
            .and_then(|v| v.as_array())
            .and_then(|v| v.first())
            .and_then(|v| v.get("location"))
            .and_then(|v| v.get("file"))
            .and_then(|v| v.as_str()),
        Some(trace.to_string_lossy().as_ref())
    );
}

#[test]
fn replay_emits_requested_html_report_artifact() {
    let ws = temp_workspace("replay-reporter-html");
    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    let trace = ws.join("trace.fozzy");

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
    assert_eq!(run.status.code(), Some(0), "run should succeed");

    let replay = run_cli_in(
        &ws,
        &[
            "replay".into(),
            trace.display().to_string(),
            "--reporter".into(),
            "html".into(),
            "--json".into(),
        ],
    );
    assert_eq!(replay.status.code(), Some(0), "replay should succeed");
    let out = parse_json_stdout(&replay);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    assert!(
        artifacts_dir.join("report.html").exists(),
        "replay should emit report.html when reporter=html"
    );
}

#[test]
fn replay_fuzz_emits_requested_html_report_artifact() {
    let ws = temp_workspace("replay-fuzz-reporter-html");
    let trace = ws.join("fuzz.trace.fozzy");

    let fuzz = run_cli_in(
        &ws,
        &[
            "fuzz".into(),
            format!(
                "scenario:{}",
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/memory.pass.fozzy.json")
                    .display()
            ),
            "--det".into(),
            "--runs".into(),
            "1".into(),
            "--seed".into(),
            "7".into(),
            "--record".into(),
            trace.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(fuzz.status.code(), Some(0), "fuzz should succeed");

    let replay = run_cli_in(
        &ws,
        &[
            "replay".into(),
            trace.display().to_string(),
            "--reporter".into(),
            "html".into(),
            "--json".into(),
        ],
    );
    assert_eq!(replay.status.code(), Some(0), "replay should succeed");
    let out = parse_json_stdout(&replay);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    assert!(
        artifacts_dir.join("report.html").exists(),
        "fuzz replay should emit report.html when reporter=html"
    );
}

#[test]
fn replay_explore_emits_requested_html_report_artifact() {
    let ws = temp_workspace("replay-explore-reporter-html");
    let trace = ws.join("explore.trace.fozzy");

    let explore = run_cli_in(
        &ws,
        &[
            "explore".into(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/kv.explore.fozzy.json")
                .display()
                .to_string(),
            "--steps".into(),
            "10".into(),
            "--seed".into(),
            "7".into(),
            "--record".into(),
            trace.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(explore.status.code(), Some(0), "explore should succeed");

    let replay = run_cli_in(
        &ws,
        &[
            "replay".into(),
            trace.display().to_string(),
            "--reporter".into(),
            "html".into(),
            "--json".into(),
        ],
    );
    assert_eq!(replay.status.code(), Some(0), "replay should succeed");
    let out = parse_json_stdout(&replay);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    assert!(
        artifacts_dir.join("report.html").exists(),
        "explore replay should emit report.html when reporter=html"
    );
}

#[test]
fn shrink_rejects_unsupported_reporter_flag() {
    let ws = temp_workspace("shrink-reporter-reject");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("write scenario");
    let trace = ws.join("trace.fozzy");
    let shrunk = ws.join("trace.min.fozzy");

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
    assert_eq!(run.status.code(), Some(0), "run should succeed");

    let shrink = run_cli_in(
        &ws,
        &[
            "shrink".into(),
            trace.display().to_string(),
            "--out".into(),
            shrunk.display().to_string(),
            "--reporter".into(),
            "html".into(),
            "--json".into(),
        ],
    );
    assert_ne!(
        shrink.status.code(),
        Some(0),
        "shrink with unsupported reporter must fail"
    );
    let stdout = String::from_utf8_lossy(&shrink.stdout);
    assert!(
        stdout.contains("invalid value 'html' for '--reporter <REPORTER>'")
            || stdout.contains("possible values: pretty"),
        "stdout: {stdout}"
    );
}

#[test]
fn trace_followup_commands_accept_bare_and_dot_relative_paths() {
    let ws = temp_workspace("trace-relative-followup");
    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            "example.fozzy.json".into(),
            "--det".into(),
            "--mem-track".into(),
            "--record".into(),
            "artifacts/repro.trace.fozzy".into(),
            "--json".into(),
        ],
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "run should succeed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    let out = parse_json_stdout(&run);
    let trace_path = out
        .get("identity")
        .and_then(|v| v.get("tracePath"))
        .and_then(|v| v.as_str())
        .expect("trace path");
    assert_eq!(
        std::fs::canonicalize(trace_path).expect("canonicalize recorded trace"),
        std::fs::canonicalize(ws.join("artifacts/repro.trace.fozzy"))
            .expect("canonicalize expected trace"),
        "recorded trace path should normalize to the created trace location"
    );

    for trace_arg in [
        "artifacts/repro.trace.fozzy",
        "./artifacts/repro.trace.fozzy",
    ] {
        let verify = run_cli_in(
            &ws,
            &[
                "trace".into(),
                "verify".into(),
                trace_arg.into(),
                "--strict".into(),
                "--json".into(),
            ],
        );
        assert_eq!(
            verify.status.code(),
            Some(0),
            "trace verify should pass for {trace_arg}: {}",
            String::from_utf8_lossy(&verify.stderr)
        );

        let replay = run_cli_in(&ws, &["replay".into(), trace_arg.into(), "--json".into()]);
        assert_eq!(
            replay.status.code(),
            Some(0),
            "replay should pass for {trace_arg}: {}",
            String::from_utf8_lossy(&replay.stderr)
        );

        let ci = run_cli_in(&ws, &["ci".into(), trace_arg.into(), "--json".into()]);
        assert_eq!(
            ci.status.code(),
            Some(0),
            "ci should pass for {trace_arg}: {}",
            String::from_utf8_lossy(&ci.stderr)
        );

        let artifacts = run_cli_in(
            &ws,
            &[
                "artifacts".into(),
                "ls".into(),
                trace_arg.into(),
                "--json".into(),
            ],
        );
        assert_eq!(
            artifacts.status.code(),
            Some(0),
            "artifacts ls should pass for {trace_arg}: {}",
            String::from_utf8_lossy(&artifacts.stderr)
        );
        let artifacts_doc = parse_json_stdout(&artifacts);
        let listed_path = artifacts_doc
            .get("entries")
            .and_then(|v| v.as_array())
            .and_then(|v| v.first())
            .and_then(|v| v.get("path"))
            .and_then(|v| v.as_str())
            .expect("listed trace path");
        assert_eq!(
            std::fs::canonicalize(listed_path).expect("canonicalize listed path"),
            std::fs::canonicalize(ws.join("artifacts/repro.trace.fozzy"))
                .expect("canonicalize expected trace"),
            "artifacts ls should normalize direct trace path for {trace_arg}"
        );

        let report = run_cli_in(
            &ws,
            &[
                "report".into(),
                "show".into(),
                trace_arg.into(),
                "--format".into(),
                "json".into(),
                "--json".into(),
            ],
        );
        assert_eq!(
            report.status.code(),
            Some(0),
            "report show should pass for {trace_arg}: {}",
            String::from_utf8_lossy(&report.stderr)
        );
        let report_doc = parse_json_stdout(&report);
        assert!(
            report_doc.get("profileDiagnosis").is_none(),
            "report show should not inject a non-diagnostic single-run profile summary for {trace_arg}"
        );

        let memory = run_cli_in(
            &ws,
            &[
                "memory".into(),
                "top".into(),
                trace_arg.into(),
                "--json".into(),
            ],
        );
        assert_eq!(
            memory.status.code(),
            Some(0),
            "memory top should pass for {trace_arg}: {}",
            String::from_utf8_lossy(&memory.stderr)
        );
        let memory_doc = parse_json_stdout(&memory);
        assert_eq!(
            memory_doc
                .get("run")
                .and_then(|v| v.as_str())
                .expect("memory run"),
            std::fs::canonicalize(ws.join("artifacts/repro.trace.fozzy"))
                .expect("canonicalize expected trace")
                .to_string_lossy(),
            "memory top should normalize run selector for {trace_arg}"
        );

        let profile = run_cli_in(
            &ws,
            &[
                "profile".into(),
                "doctor".into(),
                trace_arg.into(),
                "--json".into(),
            ],
        );
        assert_eq!(
            profile.status.code(),
            Some(0),
            "profile doctor should pass for {trace_arg}: {}",
            String::from_utf8_lossy(&profile.stderr)
        );
        let profile_doc = parse_json_stdout(&profile);
        assert_eq!(
            profile_doc
                .get("run")
                .and_then(|v| v.as_str())
                .expect("profile doctor run"),
            std::fs::canonicalize(ws.join("artifacts/repro.trace.fozzy"))
                .expect("canonicalize expected trace")
                .to_string_lossy(),
            "profile doctor should normalize run selector for {trace_arg}"
        );
    }
}

