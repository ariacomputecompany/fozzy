use super::*;

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
        Some(1),
        "external recorded run id should surface the trace-backed host memory peak"
    );
    assert_eq!(
        memory_doc
            .get("entries")
            .and_then(|v| v.as_array())
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("kind"))
            .and_then(|v| v.as_str()),
        Some("peak")
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
