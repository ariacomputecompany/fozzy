use super::*;

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
fn host_http_websocket_upgrade_status_completes_without_hanging() {
    let (url, stop_tx) = spawn_websocket_upgrade_http_server();
    let ws = temp_workspace("host-http-ws-upgrade");
    let scenario = ws.join("host-http-ws-upgrade.fozzy.json");
    let trace = ws.join("host-http-ws-upgrade.fozzy");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-ws-upgrade",
      "steps":[
        {{
          "type":"http_request",
          "method":"GET",
          "path":"{url}",
          "headers":{{
            "connection":"Upgrade",
            "upgrade":"websocket",
            "sec-websocket-version":"13",
            "sec-websocket-key":"dGhlIHNhbXBsZSBub25jZQ=="
          }},
          "expect_status":101
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
        "--det".into(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(0),
        "websocket upgrade should complete cleanly: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let replay = run_cli(&[
        "replay".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        replay.status.code(),
        Some(0),
        "replay must preserve upgraded host http decision"
    );
}

#[test]
fn host_http_websocket_upgrade_with_http_when_and_auth_completes_cleanly() {
    let (url, stop_tx) = spawn_websocket_upgrade_http_server();
    let ws = temp_workspace("host-http-ws-afw-contract");
    let scenario = ws.join("host-http-ws-afw-contract.fozzy.json");
    let trace = ws.join("host-http-ws-afw-contract.fozzy");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-ws-afw-contract",
      "steps":[
        {{
          "type":"http_when",
          "method":"GET",
          "path":"{url}",
          "status":101,
          "times":1
        }},
        {{
          "type":"http_request",
          "method":"GET",
          "path":"{url}",
          "headers":{{
            "authorization":"Bearer mn_bootstrap",
            "connection":"Upgrade",
            "upgrade":"websocket",
            "sec-websocket-version":"13",
            "sec-websocket-key":"dGhlIHNhbXBsZSBub25jZQ=="
          }},
          "expect_status":101
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
        "--det".into(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(0),
        "afw-style websocket upgrade should complete cleanly: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let trace_doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&trace).expect("read trace")).expect("trace json");
    let http_event = trace_doc
        .get("events")
        .and_then(|v| v.as_array())
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.get("name").and_then(|v| v.as_str()) == Some("http_request"))
        })
        .expect("http event");
    let fields = http_event.get("fields").expect("event fields");
    assert_eq!(
        fields.get("request_kind").and_then(|v| v.as_str()),
        Some("websocket_upgrade")
    );
    assert_eq!(
        fields.get("completion_boundary").and_then(|v| v.as_str()),
        Some("upgrade_headers")
    );
    assert_eq!(
        fields.get("upgrade_requested").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        fields.get("upgrade_accepted").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[test]
fn host_http_websocket_upgrade_timeout_emits_lifecycle_specific_diagnostics() {
    let (url, stop_tx) = spawn_slow_http_server(Duration::from_millis(200));
    let ws = temp_workspace("host-http-ws-timeout");
    let scenario = ws.join("host-http-ws-timeout.fozzy.json");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-ws-timeout",
      "steps":[
        {{
          "type":"http_request",
          "method":"GET",
          "path":"{url}",
          "headers":{{
            "connection":"Upgrade",
            "upgrade":"websocket",
            "sec-websocket-version":"13",
            "sec-websocket-key":"dGhlIHNhbXBsZSBub25jZQ=="
          }},
          "expect_status":101
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
        "--det".into(),
        "--timeout".into(),
        "50ms".into(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(3),
        "websocket upgrade timeout should exit 3: {}",
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
            .is_some_and(|msg| msg.contains("host websocket upgrade timed out")),
        "expected websocket-specific timeout message, got: {finding:?}"
    );
    let details = finding
        .get("location")
        .and_then(|v| v.get("details"))
        .expect("location details");
    assert_eq!(
        details.get("requestKind").and_then(|v| v.as_str()),
        Some("websocket_upgrade")
    );
    assert_eq!(
        details.get("upgradeRequested").and_then(|v| v.as_bool()),
        Some(true)
    );
}

#[test]
fn host_http_websocket_upgrade_unmatched_rule_reports_contract_guidance() {
    let (url, stop_tx) = spawn_websocket_upgrade_http_server();
    let ws = temp_workspace("host-http-ws-unmatched");
    let scenario = ws.join("host-http-ws-unmatched.fozzy.json");
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-ws-unmatched",
      "steps":[
        {{
          "type":"http_when",
          "method":"GET",
          "path":"{url}&lane=wrong",
          "status":101,
          "times":1
        }},
        {{
          "type":"http_request",
          "method":"GET",
          "path":"{url}",
          "headers":{{
            "authorization":"Bearer mn_bootstrap",
            "connection":"Upgrade",
            "upgrade":"websocket",
            "sec-websocket-version":"13",
            "sec-websocket-key":"dGhlIHNhbXBsZSBub25jZQ=="
          }},
          "expect_status":101
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
        "--det".into(),
        "--json".into(),
    ]);
    let _ = stop_tx.send(());
    assert_eq!(
        run.status.code(),
        Some(1),
        "unmatched websocket upgrade rule should fail: {}",
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
        Some("http_when_host_unmatched")
    );
    assert!(
        finding
            .get("message")
            .and_then(|v| v.as_str())
            .is_some_and(|msg| {
                msg.contains("host websocket upgrade") && msg.contains("auth/upgrade headers")
            }),
        "expected websocket-specific contract guidance, got: {finding:?}"
    );
    let details = finding
        .get("location")
        .and_then(|v| v.get("details"))
        .expect("location details");
    assert_eq!(
        details.get("requestKind").and_then(|v| v.as_str()),
        Some("websocket_upgrade")
    );
    assert_eq!(
        details.get("upgradeRequested").and_then(|v| v.as_bool()),
        Some(true)
    );
}
