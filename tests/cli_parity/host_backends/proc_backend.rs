use super::*;

#[cfg(unix)]
#[test]
fn host_proc_backend_executes_real_proc_spawn_for_run() {
    let ws = temp_workspace("host-proc-run");
    let scenario = ws.join("host-proc.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-proc",
      "steps":[
        {"type":"proc_spawn","cmd":"/usr/bin/true","expect_exit":0}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");

    let out = run_cli(&[
        "--proc-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "host proc run should pass, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(doc.get("status").and_then(|v| v.as_str()), Some("pass"));
}

#[cfg(unix)]
#[test]
fn host_proc_backend_executes_in_deterministic_mode() {
    let ws = temp_workspace("host-proc-det");
    let scenario = ws.join("host-proc-det.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-proc-det",
      "steps":[
        {"type":"proc_spawn","cmd":"/usr/bin/true","expect_exit":0}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");

    let out = run_cli(&[
        "--proc-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(0), "det + host proc should pass");
    let doc = parse_json_stdout(&out);
    assert_eq!(doc.get("status").and_then(|v| v.as_str()), Some("pass"));
}

#[cfg(unix)]
#[test]
fn replay_uses_recorded_proc_decisions_from_host_backend_trace() {
    let ws = temp_workspace("host-proc-replay");
    let scenario = ws.join("host-proc-replay.fozzy.json");
    let trace = ws.join("host-proc-replay.fozzy");
    let raw = r#"{
      "version":1,
      "name":"host-proc-replay",
      "steps":[
        {"type":"proc_spawn","cmd":"/bin/echo","args":["hi"],"expect_exit":0,"expect_stdout":"hi\n"}
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
    assert_eq!(run.status.code(), Some(0), "host run should pass");

    let replay = run_cli(&[
        "replay".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        replay.status.code(),
        Some(0),
        "replay should pass from recorded proc decisions, stderr={}",
        String::from_utf8_lossy(&replay.stderr)
    );
    let doc = parse_json_stdout(&replay);
    assert_eq!(doc.get("status").and_then(|v| v.as_str()), Some("pass"));
}

#[cfg(unix)]
#[test]
fn host_proc_trace_records_real_duration() {
    let ws = temp_workspace("host-proc-duration");
    let scenario = ws.join("host-proc-duration.fozzy.json");
    let trace = ws.join("host-proc-duration.fozzy");
    let raw = r#"{
      "version":1,
      "name":"host-proc-duration",
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
    assert_eq!(run.status.code(), Some(0), "host duration run should pass");

    let trace_doc = read_trace_json(&trace);
    let summary_ms = trace_doc
        .get("summary")
        .and_then(|v| v.get("durationMs"))
        .and_then(|v| v.as_u64())
        .expect("trace summary duration");
    assert!(
        summary_ms >= 900,
        "expected recorded trace summary duration to reflect wall time, got {summary_ms}"
    );

    let events = trace_doc
        .get("events")
        .and_then(|v| v.as_array())
        .expect("events array");
    let proc_event = events
        .iter()
        .find(|event| event.get("name").and_then(|v| v.as_str()) == Some("proc_spawn"))
        .expect("proc_spawn event");
    assert_eq!(
        proc_event
            .get("fields")
            .and_then(|v| v.get("backend"))
            .and_then(|v| v.as_str()),
        Some("host")
    );
    let proc_time_ms = proc_event
        .get("time_ms")
        .and_then(|v| v.as_u64())
        .expect("proc event time");
    assert!(
        proc_time_ms >= 900,
        "expected proc event time to advance with host elapsed time, got {proc_time_ms}"
    );

    let capability_event = events
        .iter()
        .find(|event| event.get("name").and_then(|v| v.as_str()) == Some("capability_proc"))
        .expect("capability_proc event");
    let capability_duration = capability_event
        .get("fields")
        .and_then(|v| v.get("duration_ms"))
        .and_then(|v| v.as_u64())
        .expect("capability duration");
    assert!(
        capability_duration >= 900,
        "expected capability duration to reflect host elapsed time, got {capability_duration}"
    );

    let span_end = events
        .iter()
        .find(|event| event.get("name").and_then(|v| v.as_str()) == Some("span_end"))
        .expect("span_end event");
    let span_duration = span_end
        .get("fields")
        .and_then(|v| v.get("duration_ms"))
        .and_then(|v| v.as_u64())
        .expect("span duration");
    assert!(
        span_duration >= 900,
        "expected step span duration to reflect host elapsed time, got {span_duration}"
    );
}

#[cfg(unix)]
#[test]
fn host_proc_backend_executes_real_command_even_with_proc_when_contract() {
    let ws = temp_workspace("host-proc-when");
    let scenario = ws.join("host-proc-when.fozzy.json");
    let trace = ws.join("host-proc-when.fozzy");
    let marker = ws.join("invoked.txt");
    let command = format!(
        "printf 'invoked\\n' >> {}; sleep 1; echo real-ok",
        marker.display()
    );
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-proc-when",
      "steps":[
        {{"type":"proc_when","cmd":"/bin/sh","args":["-lc","{command}"],"exit_code":0,"stdout":"real-ok\n","stderr":"","times":1}},
        {{"type":"proc_spawn","cmd":"/bin/sh","args":["-lc","{command}"],"expect_exit":0,"expect_stdout":"real-ok\n"}}
      ]
    }}"#
    );
    std::fs::write(&scenario, raw).expect("write scenario");

    let run = run_cli_in(
        &ws,
        &[
            "--proc-backend".into(),
            "host".into(),
            "run".into(),
            scenario.to_string_lossy().to_string(),
            "--det".into(),
            "--record".into(),
            trace.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(run.status.code(), Some(0), "host proc_when run should pass");

    let invocations = std::fs::read_to_string(&marker).expect("marker file should exist");
    assert_eq!(
        invocations.lines().count(),
        1,
        "host proc should run exactly once"
    );

    let trace_doc = read_trace_json(&trace);
    let proc_event = trace_doc
        .get("events")
        .and_then(|v| v.as_array())
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.get("name").and_then(|v| v.as_str()) == Some("proc_spawn"))
        })
        .expect("proc_spawn event");
    assert_eq!(
        proc_event
            .get("fields")
            .and_then(|v| v.get("backend"))
            .and_then(|v| v.as_str()),
        Some("host")
    );
}

#[cfg(unix)]
#[test]
fn host_proc_timeout_is_recorded_and_replayed_as_timeout() {
    let ws = temp_workspace("host-proc-timeout");
    let scenario = ws.join("host-proc-timeout.fozzy.json");
    let trace = ws.join("host-proc-timeout.fozzy");
    let raw = r#"{
      "version":1,
      "name":"host-proc-timeout",
      "steps":[
        {"type":"proc_spawn","cmd":"/bin/sh","args":["-c","sleep 2"]}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");

    let run = run_cli(&[
        "--proc-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--timeout".into(),
        "50ms".into(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        run.status.code(),
        Some(3),
        "host proc timeout should exit 3, stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );
    let run_doc = parse_json_stdout(&run);
    assert_eq!(
        run_doc.get("status").and_then(|v| v.as_str()),
        Some("timeout")
    );

    let replay = run_cli(&[
        "replay".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        replay.status.code(),
        Some(3),
        "replay should preserve proc timeout, stderr={}",
        String::from_utf8_lossy(&replay.stderr)
    );
    let replay_doc = parse_json_stdout(&replay);
    assert_eq!(
        replay_doc.get("status").and_then(|v| v.as_str()),
        Some("timeout")
    );
}

#[cfg(unix)]
#[test]
fn host_proc_timeout_emits_lifecycle_specific_diagnostics() {
    let ws = temp_workspace("host-proc-timeout-details");
    let scenario = ws.join("host-proc-timeout-details.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-proc-timeout-details",
      "steps":[
        {"type":"proc_spawn","cmd":"/bin/sh","args":["-c","sleep 2"]}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");

    let run = run_cli(&[
        "--proc-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--timeout".into(),
        "50ms".into(),
        "--json".into(),
    ]);
    assert_eq!(
        run.status.code(),
        Some(3),
        "host proc timeout should exit 3, stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );

    let doc = parse_json_stdout(&run);
    let finding = doc
        .get("findings")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())
        .expect("timeout finding");
    assert_eq!(
        finding.get("title").and_then(|v| v.as_str()),
        Some("timeout")
    );
    assert!(
        finding
            .get("message")
            .and_then(|v| v.as_str())
            .is_some_and(|msg| msg.contains("terminal process-exit boundary")),
        "expected process lifecycle timeout guidance, got: {finding:?}"
    );
    let details = finding
        .get("location")
        .and_then(|v| v.get("details"))
        .expect("location details");
    assert_eq!(
        details.get("requestKind").and_then(|v| v.as_str()),
        Some("process_spawn")
    );
    assert_eq!(
        details.get("command").and_then(|v| v.as_str()),
        Some("/bin/sh")
    );
}

#[cfg(unix)]
#[test]
fn host_proc_unmatched_rule_reports_contract_guidance_with_details() {
    let ws = temp_workspace("host-proc-unmatched");
    let scenario = ws.join("host-proc-unmatched.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-proc-unmatched",
      "steps":[
        {"type":"proc_when","cmd":"/bin/echo","args":["wrong"],"exit_code":0,"stdout":"wrong\n","stderr":"","times":1},
        {"type":"proc_spawn","cmd":"/bin/echo","args":["right"],"expect_exit":0,"expect_stdout":"right\n"}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");

    let run = run_cli(&[
        "--proc-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--json".into(),
    ]);
    assert_eq!(
        run.status.code(),
        Some(1),
        "unmatched proc_when should fail, stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );

    let doc = parse_json_stdout(&run);
    let finding = doc
        .get("findings")
        .and_then(|v| v.as_array())
        .and_then(|v| v.first())
        .expect("unmatched finding");
    assert_eq!(
        finding.get("title").and_then(|v| v.as_str()),
        Some("proc_when_host_unmatched")
    );
    let details = finding
        .get("location")
        .and_then(|v| v.get("details"))
        .expect("location details");
    assert_eq!(
        details.get("requestKind").and_then(|v| v.as_str()),
        Some("process_spawn")
    );
    assert_eq!(
        details.get("args").and_then(|v| v.as_array()).map(Vec::len),
        Some(1)
    );
}

#[cfg(unix)]
#[test]
fn mixed_host_http_and_proc_lifecycle_completes_with_terminal_boundaries() {
    let (url, stop_tx) = spawn_websocket_upgrade_http_server();
    let ws = temp_workspace("mixed-host-lifecycle");
    let scenario = ws.join("mixed-host-lifecycle.fozzy.json");
    let trace = ws.join("mixed-host-lifecycle.fozzy");
    let marker = ws.join("proc.txt");
    let command = format!("printf 'ok\\n' >> {}; echo proc-ok", marker.display());
    let raw = format!(
        r#"{{
      "version":1,
      "name":"mixed-host-lifecycle",
      "steps":[
        {{"type":"proc_when","cmd":"/bin/sh","args":["-lc","{command}"],"exit_code":0,"stdout":"proc-ok\n","stderr":"","times":1}},
        {{"type":"http_when","method":"GET","path":"{url}","status":101,"times":1}},
        {{"type":"proc_spawn","cmd":"/bin/sh","args":["-lc","{command}"],"expect_exit":0,"expect_stdout":"proc-ok\n"}},
        {{"type":"http_request","method":"GET","path":"{url}","headers":{{"authorization":"Bearer mn_bootstrap","connection":"Upgrade","upgrade":"websocket","sec-websocket-version":"13","sec-websocket-key":"dGhlIHNhbXBsZSBub25jZQ=="}},"expect_status":101}}
      ]
    }}"#
    );
    std::fs::write(&scenario, raw).expect("write scenario");

    let run = run_cli_in(
        &ws,
        &[
            "--proc-backend".into(),
            "host".into(),
            "--http-backend".into(),
            "host".into(),
            "run".into(),
            scenario.to_string_lossy().to_string(),
            "--det".into(),
            "--record".into(),
            trace.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(0),
        "mixed host-backed lifecycle should complete cleanly: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&marker).expect("marker"),
        "ok\n",
        "host proc should run exactly once"
    );

    let trace_doc = read_trace_json(&trace);
    let events = trace_doc
        .get("events")
        .and_then(|v| v.as_array())
        .expect("events");
    let proc_event = events
        .iter()
        .find(|event| event.get("name").and_then(|v| v.as_str()) == Some("proc_spawn"))
        .expect("proc event");
    assert_eq!(
        proc_event
            .get("fields")
            .and_then(|v| v.get("completion_boundary"))
            .and_then(|v| v.as_str()),
        Some("process_exit")
    );
    let http_event = events
        .iter()
        .find(|event| {
            event.get("name").and_then(|v| v.as_str()) == Some("http_request")
                && event
                    .get("fields")
                    .and_then(|v| v.get("path"))
                    .and_then(|v| v.as_str())
                    .is_some_and(|path| path.contains("/ws/status"))
        })
        .expect("http event");
    assert_eq!(
        http_event
            .get("fields")
            .and_then(|v| v.get("completion_boundary"))
            .and_then(|v| v.as_str()),
        Some("upgrade_headers")
    );
}

#[cfg(unix)]
#[test]
fn host_proc_stdout_limit_is_enforced_during_streaming() {
    let ws = temp_workspace("host-proc-limit");
    let scenario = ws.join("host-proc-limit.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-proc-limit",
      "steps":[
        {"type":"proc_spawn","cmd":"/bin/sh","args":["-c","yes x | head -c 9000000"]}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");

    let run = run_cli(&[
        "--proc-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        run.status.code(),
        Some(1),
        "oversized host proc stdout should fail, stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );
    let doc = parse_json_stdout(&run);
    let findings = doc
        .get("findings")
        .and_then(|v| v.as_array())
        .expect("findings array");
    assert!(findings.iter().any(|finding| {
        finding.get("title").and_then(|v| v.as_str()) == Some("proc_spawn_host")
            && finding
                .get("message")
                .and_then(|v| v.as_str())
                .is_some_and(|msg| msg.contains("stdout exceeded limit"))
    }));
}

#[test]
fn recorded_proc_spawn_events_include_stdout_and_stderr() {
    let ws = temp_workspace("proc-spawn-event-io");
    let scenario = ws.join("proc.fozzy.json");
    std::fs::write(&scenario, fixture("proc.fozzy.json")).expect("write scenario");
    let trace = ws.join("trace.fozzy");

    let out = run_cli(&[
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--record-collision".into(),
        "overwrite".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "proc scenario should pass, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let trace_doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&trace).expect("read trace")).expect("trace json");
    let proc_event = trace_doc
        .get("events")
        .and_then(|v| v.as_array())
        .and_then(|events| {
            events
                .iter()
                .find(|e| e.get("name").and_then(|n| n.as_str()) == Some("proc_spawn"))
        })
        .expect("proc_spawn event");
    let fields = proc_event
        .get("fields")
        .and_then(|v| v.as_object())
        .expect("proc_spawn fields");
    assert_eq!(
        fields.get("stdout").and_then(|v| v.as_str()),
        Some("abc123")
    );
    assert_eq!(fields.get("stderr").and_then(|v| v.as_str()), Some(""));
}
