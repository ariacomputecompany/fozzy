use super::*;

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
