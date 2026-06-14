use super::*;

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
fn host_http_when_json_mismatch_emits_structured_diff_details() {
    let ws = temp_workspace("host-http-when-json-diff");
    let scenario = ws.join("host-http-when-json-diff.fozzy.json");
    let (url, stop_tx) = spawn_json_http_server();
    let raw = format!(
        r#"{{
      "version":1,
      "name":"host-http-when-json-diff",
      "steps":[
        {{"type":"http_when","method":"GET","path":"{url}","status":200,"json":{{"ok":true,"service":"expected","nested":{{"status":"ready"}}}}}},
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
    assert_eq!(run.status.code(), Some(1), "json mismatch should fail");
    let doc = parse_json_stdout(&run);
    let finding = doc
        .get("findings")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .expect("json mismatch finding");
    assert_eq!(
        finding.get("title").and_then(|v| v.as_str()),
        Some("http_when_host_json")
    );
    let details = finding
        .get("location")
        .and_then(|v| v.get("details"))
        .expect("json mismatch details");
    assert!(
        details
            .get("mismatches")
            .and_then(|v| v.as_array())
            .is_some_and(|rows| !rows.is_empty()),
        "expected structured mismatch rows, got: {details}"
    );
}
