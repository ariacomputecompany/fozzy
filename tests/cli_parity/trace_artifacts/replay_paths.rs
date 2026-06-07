use super::*;

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
