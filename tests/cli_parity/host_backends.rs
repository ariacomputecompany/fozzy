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
fn host_fs_backend_executes_real_filesystem_steps() {
    let ws = temp_workspace("host-fs");
    let scenario = ws.join("host-fs.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-fs",
      "steps":[
        {"type":"fs_write","path":"tmp/host-fs.txt","data":"hello"},
        {"type":"fs_read_assert","path":"tmp/host-fs.txt","equals":"hello"}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");
    let out = run_cli(&[
        "--fs-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--cwd".into(),
        ws.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(0), "host fs run should pass");
    let written =
        std::fs::read_to_string(ws.join("tmp").join("host-fs.txt")).expect("read host fs output");
    assert_eq!(written, "hello");
}

#[test]
fn host_fs_backend_rejects_path_escape() {
    let ws = temp_workspace("host-fs-escape");
    let scenario = ws.join("host-fs-escape.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-fs-escape",
      "steps":[
        {"type":"fs_write","path":"../escape.txt","data":"bad"}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");
    let out = run_cli(&[
        "--fs-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--cwd".into(),
        ws.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "path escape must fail as assertion"
    );
}

#[test]
fn host_fs_backend_executes_in_deterministic_mode() {
    let ws = temp_workspace("host-fs-det");
    let scenario = ws.join("host-fs-det.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-fs-det",
      "steps":[
        {"type":"fs_write","path":"x.txt","data":"x"},
        {"type":"fs_read_assert","path":"x.txt","equals":"x"}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");
    let out = run_cli(&[
        "--fs-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(0), "det + host fs should pass");
}

#[test]
fn host_fs_backend_replays_from_recorded_deterministic_trace() {
    let ws = temp_workspace("host-fs-replay-det");
    let scenario = ws.join("host-fs-replay-det.fozzy.json");
    let trace = ws.join("host-fs-replay-det.fozzy");
    let raw = r#"{
      "version":1,
      "name":"host-fs-replay-det",
      "steps":[
        {"type":"fs_write","path":"tmp/host-fs.txt","data":"hello"},
        {"type":"fs_snapshot","name":"before"},
        {"type":"fs_read_assert","path":"tmp/host-fs.txt","equals":"hello"},
        {"type":"fs_write","path":"tmp/host-fs.txt","data":"changed"},
        {"type":"fs_restore","name":"before"},
        {"type":"fs_read_assert","path":"tmp/host-fs.txt","equals":"hello"}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");

    let run = run_cli(&[
        "--fs-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--cwd".into(),
        ws.to_string_lossy().to_string(),
        "--det".into(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        run.status.code(),
        Some(0),
        "det + host fs record should pass: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let verify = run_cli(&[
        "trace".into(),
        "verify".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(verify.status.code(), Some(0), "trace verify should pass");
    let verify_doc = parse_json_stdout(&verify);
    assert!(
        verify_doc
            .get("warnings")
            .and_then(|v| v.as_array())
            .is_none_or(|warnings| warnings.is_empty()),
        "host fs trace should include replay decisions"
    );

    let replay = run_cli(&[
        "replay".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        replay.status.code(),
        Some(0),
        "replay should pass from recorded fs decisions: {}",
        String::from_utf8_lossy(&replay.stderr)
    );
}

#[test]
fn host_http_backend_executes_and_replays_from_decisions() {
    let (url, stop_tx) = spawn_one_shot_http_server();
    let ws = temp_workspace("host-http");
    let scenario = ws.join("host-http.fozzy.json");
    let trace = ws.join("host-http.fozzy");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http",
      "steps":[
        {{"type":"http_request","method":"GET","path":"{url}","expect_status":200,"expect_body":"ok"}}
      ]
    }}"#
    );
    std::fs::write(&scenario, raw).expect("write scenario");
    let run = run_cli(&[
        "--http-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(0),
        "host http run should pass: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let replay = run_cli(&[
        "replay".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(replay.status.code(), Some(0), "replay must pass");
}

#[test]
fn host_http_timeout_is_recorded_and_replayed_as_timeout() {
    let (url, stop_tx) = spawn_slow_http_server(Duration::from_millis(200));
    let ws = temp_workspace("host-http-timeout");
    let scenario = ws.join("host-http-timeout.fozzy.json");
    let trace = ws.join("host-http-timeout.fozzy");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-timeout",
      "steps":[
        {{"type":"http_request","method":"GET","path":"{url}","expect_status":200}}
      ]
    }}"#
    );
    std::fs::write(&scenario, raw).expect("write scenario");
    let run = run_cli(&[
        "--http-backend".into(),
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
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(3),
        "host http timeout should exit 3, stderr={}",
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
        "replay should preserve http timeout, stderr={}",
        String::from_utf8_lossy(&replay.stderr)
    );
    let replay_doc = parse_json_stdout(&replay);
    assert_eq!(
        replay_doc.get("status").and_then(|v| v.as_str()),
        Some("timeout")
    );
}

#[test]
fn host_http_body_limit_is_enforced_during_streaming() {
    let (url, stop_tx) = spawn_large_body_http_server(8 * 1024 * 1024 + 1024);
    let ws = temp_workspace("host-http-limit");
    let scenario = ws.join("host-http-limit.fozzy.json");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-limit",
      "steps":[
        {{"type":"http_request","method":"GET","path":"{url}","expect_status":200}}
      ]
    }}"#
    );
    std::fs::write(&scenario, raw).expect("write scenario");
    let run = run_cli(&[
        "--http-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(1),
        "oversized host http body should fail, stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );
    let doc = parse_json_stdout(&run);
    let findings = doc
        .get("findings")
        .and_then(|v| v.as_array())
        .expect("findings array");
    assert!(findings.iter().any(|finding| {
        let title = finding.get("title").and_then(|v| v.as_str());
        let message = finding.get("message").and_then(|v| v.as_str());
        title == Some("http_host_request")
            && message.is_some_and(|msg| {
                msg.contains("host http body exceeded limit")
                    || msg.contains("host http body read failed")
            })
    }));
}

#[test]
fn http_request_supports_headers_and_response_header_assertions() {
    let (url, stop_tx) = spawn_header_http_server();
    let ws = temp_workspace("host-http-headers");
    let scenario = ws.join("host-http-headers.fozzy.json");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-headers",
      "steps":[
        {{
          "type":"http_request",
          "method":"GET",
          "path":"{url}",
          "headers":{{"Authorization":"Bearer demo-token"}},
          "expect_status":200,
          "expect_headers":{{"x-trace-id":"abc-123","x-service":"fozzy-test"}},
          "expect_body":"ok"
        }}
      ]
    }}"#
    );
    std::fs::write(&scenario, raw).expect("write scenario");
    let run = run_cli(&[
        "--http-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(0),
        "header request/assertions should pass: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn host_http_backend_executes_in_deterministic_mode() {
    let ws = temp_workspace("host-http-det");
    let scenario = ws.join("host-http-det.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-http-det",
      "steps":[{"type":"http_request","method":"GET","path":"http://127.0.0.1:1/x","expect_status":200}]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");
    let out = run_cli(&[
        "--http-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "det + host http should reach live backend"
    );
}

#[test]
fn host_http_backend_accepts_https_scheme() {
    let ws = temp_workspace("host-http-https");
    let scenario = ws.join("host-http-https.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"host-http-https",
      "steps":[{"type":"http_request","method":"GET","path":"https://127.0.0.1:1/x","expect_status":200}]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");
    let out = run_cli(&[
        "--http-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "request should fail at network/tls layer"
    );
    let doc = parse_json_stdout(&out);
    let msg = doc
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        !msg.contains("https is not supported"),
        "https must be supported by host backend, got: {msg}"
    );
}

#[test]
fn scripted_http_when_supports_response_headers_assertions() {
    let ws = temp_workspace("scripted-http-headers");
    let scenario = ws.join("scripted-http-headers.fozzy.json");
    let raw = r#"{
      "version":1,
      "name":"scripted-http-headers",
      "steps":[
        {"type":"http_when","method":"GET","path":"/ping","status":200,"headers":{"x-test":"yes","content-type":"text/plain"},"body":"ok"},
        {"type":"http_request","method":"GET","path":"/ping","expect_status":200,"expect_headers":{"x-test":"yes","content-type":"text/plain"},"expect_body":"ok"}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");
    let out = run_cli(&[
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "scripted response headers should assert: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn host_http_when_supports_absolute_url_rules() {
    let (url, stop_tx) = spawn_one_shot_http_server();
    let ws = temp_workspace("host-http-when-absolute");
    let scenario = ws.join("host-http-when-absolute.fozzy.json");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-when-absolute",
      "steps":[
        {{"type":"http_when","method":"GET","path":"{url}","status":200,"body":"ok"}},
        {{"type":"http_request","method":"GET","path":"{url}"}}
      ]
    }}"#
    );
    std::fs::write(&scenario, raw).expect("write scenario");
    let run = run_cli(&[
        "--http-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(0),
        "host http_when absolute rule should pass: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn host_http_when_supports_relative_path_rules() {
    let (url, stop_tx) = spawn_one_shot_http_server();
    let ws = temp_workspace("host-http-when-relative");
    let scenario = ws.join("host-http-when-relative.fozzy.json");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-when-relative",
      "steps":[
        {{"type":"http_when","method":"GET","path":"/ping","status":200,"body":"ok"}},
        {{"type":"http_request","method":"GET","path":"{url}"}}
      ]
    }}"#
    );
    std::fs::write(&scenario, raw).expect("write scenario");
    let run = run_cli(&[
        "--http-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(0),
        "host http_when relative rule should pass: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn host_http_when_unmatched_includes_remediation_guidance() {
    let (url, stop_tx) = spawn_one_shot_http_server();
    let ws = temp_workspace("host-http-when-unmatched");
    let scenario = ws.join("host-http-when-unmatched.fozzy.json");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-when-unmatched",
      "steps":[
        {{"type":"http_when","method":"GET","path":"/wrong","status":200,"body":"ok"}},
        {{"type":"http_request","method":"GET","path":"{url}"}}
      ]
    }}"#
    );
    std::fs::write(&scenario, raw).expect("write scenario");
    let run = run_cli(&[
        "--http-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(run.status.code(), Some(1), "host rule mismatch should fail");
    let doc = parse_json_stdout(&run);
    let msg = doc
        .get("findings")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|finding| finding.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        msg.contains("--http-backend scripted"),
        "expected remediation guidance in message, got: {msg}"
    );
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

