use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn temp_workspace(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("fozzy-cli-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp workspace");
    root
}

fn fixture(name: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tests_path = root.join("tests").join(name);
    if tests_path.exists() {
        return std::fs::read_to_string(tests_path).expect("read fixture");
    }
    let fixtures_path = root.join("fixtures").join(name);
    std::fs::read_to_string(fixtures_path).expect("read fixture")
}

fn run_cli(args: &[String]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args(args)
        .output()
        .expect("run cli")
}

fn run_cli_in(cwd: &Path, args: &[String]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run cli in cwd")
}

fn spawn_one_shot_http_server() -> (String, mpsc::Sender<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind http listener");
    listener
        .set_nonblocking(true)
        .expect("set nonblocking listener");
    let addr = listener.local_addr().expect("local addr");
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let start = std::time::Instant::now();
        loop {
            if stop_rx.try_recv().is_ok() || start.elapsed() > Duration::from_secs(10) {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 1024];
                    let _ = std::io::Read::read(&mut stream, &mut buf);
                    let response =
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
                    let _ = std::io::Write::write_all(&mut stream, response);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (format!("http://{addr}/ping"), stop_tx)
}

fn spawn_header_http_server() -> (String, mpsc::Sender<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind http listener");
    listener
        .set_nonblocking(true)
        .expect("set nonblocking listener");
    let addr = listener.local_addr().expect("local addr");
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let start = std::time::Instant::now();
        loop {
            if stop_rx.try_recv().is_ok() || start.elapsed() > Duration::from_secs(10) {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 4096];
                    let n = std::io::Read::read(&mut stream, &mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let has_auth = req
                        .lines()
                        .any(|line| line.eq_ignore_ascii_case("authorization: bearer demo-token"));
                    let (status, body) = if has_auth {
                        ("200 OK", "ok")
                    } else {
                        ("401 Unauthorized", "missing-auth")
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\nX-Trace-Id: abc-123\r\nX-Service: fozzy-test\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (format!("http://{addr}/headers"), stop_tx)
}

fn spawn_slow_http_server(delay: Duration) -> (String, mpsc::Sender<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind http listener");
    listener
        .set_nonblocking(true)
        .expect("set nonblocking listener");
    let addr = listener.local_addr().expect("local addr");
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let start = std::time::Instant::now();
        loop {
            if stop_rx.try_recv().is_ok() || start.elapsed() > Duration::from_secs(10) {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 1024];
                    let _ = std::io::Read::read(&mut stream, &mut buf);
                    thread::sleep(delay);
                    let response =
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
                    let _ = std::io::Write::write_all(&mut stream, response);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (format!("http://{addr}/slow"), stop_tx)
}

fn spawn_large_body_http_server(body_len: usize) -> (String, mpsc::Sender<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind http listener");
    listener
        .set_nonblocking(true)
        .expect("set nonblocking listener");
    let addr = listener.local_addr().expect("local addr");
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    thread::spawn(move || {
        let start = std::time::Instant::now();
        loop {
            if stop_rx.try_recv().is_ok() || start.elapsed() > Duration::from_secs(10) {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 1024];
                    let _ = std::io::Read::read(&mut stream, &mut buf);
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {body_len}\r\nConnection: close\r\n\r\n"
                    );
                    let _ = std::io::Write::write_all(&mut stream, header.as_bytes());
                    let chunk = vec![b'x'; 8192];
                    let mut remaining = body_len;
                    while remaining > 0 {
                        let write_len = remaining.min(chunk.len());
                        if std::io::Write::write_all(&mut stream, &chunk[..write_len]).is_err() {
                            break;
                        }
                        remaining -= write_len;
                    }
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });
    (format!("http://{addr}/large"), stop_tx)
}

fn parse_json_stdout(output: &std::process::Output) -> serde_json::Value {
    let s = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(s.trim()).expect("stdout json")
}

fn read_trace_json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).expect("read trace")).expect("trace json")
}

fn parse_first_json_stdout(output: &std::process::Output) -> serde_json::Value {
    let mut docs = serde_json::Deserializer::from_slice(&output.stdout).into_iter();
    docs.next()
        .expect("stdout contains json document")
        .expect("first stdout json")
}

fn json_run_id(doc: &serde_json::Value) -> String {
    doc.get("identity")
        .and_then(|v| v.get("runId").or_else(|| v.get("run_id")))
        .and_then(|v| v.as_str())
        .expect("run id")
        .to_string()
}

fn full_step_status(doc: &serde_json::Value, name: &str) -> Option<String> {
    doc.get("steps")
        .and_then(|v| v.as_array())
        .and_then(|steps| {
            steps.iter().find_map(|step| {
                if step.get("name").and_then(|v| v.as_str()) == Some(name) {
                    step.get("status")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
}

fn full_step_detail(doc: &serde_json::Value, name: &str) -> Option<String> {
    doc.get("steps")
        .and_then(|v| v.as_array())
        .and_then(|steps| {
            steps.iter().find_map(|step| {
                if step.get("name").and_then(|v| v.as_str()) == Some(name) {
                    step.get("detail")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let lsb = crc & 1;
            crc >>= 1;
            if lsb != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    !crc
}

fn build_zip_with_raw_entries(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut out = Vec::<u8>::new();
    let mut central = Vec::<u8>::new();
    let mut offsets = Vec::<u32>::new();

    for (name, payload) in entries {
        let offset = out.len() as u32;
        offsets.push(offset);
        let crc = crc32(payload);
        let name_len = name.len() as u16;
        let size = payload.len() as u32;

        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name);
        out.extend_from_slice(payload);
    }

    let cd_offset = out.len() as u32;
    for ((name, payload), offset) in entries.iter().zip(offsets.iter().copied()) {
        let crc = crc32(payload);
        let name_len = name.len() as u16;
        let size = payload.len() as u32;
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name);
    }
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);

    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

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
        "json".into(),
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
        "json".into(),
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
        "json".into(),
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
        "json".into(),
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
    assert_eq!(strict_ci.status.code(), Some(2), "strict ci should fail");
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
fn profile_help_uses_run_or_trace_value_name() {
    for sub in ["top", "flame", "timeline", "export", "shrink", "doctor"] {
        let out = run_cli(&["profile".into(), sub.to_string(), "--help".into()]);
        assert_eq!(out.status.code(), Some(0), "help should exit 0");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("RUN_OR_TRACE"),
            "help should show RUN_OR_TRACE for profile {sub}; got: {stdout}"
        );
        assert!(
            stdout.contains("latest|last-pass|last-fail")
                || stdout.contains("latest, last-pass, last-fail"),
            "help should describe aliases for profile {sub}; got: {stdout}"
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
    assert_eq!(run.status.code(), Some(0), "host shrink source run should pass");

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
    assert_eq!(invocations.lines().count(), 1, "host proc should run exactly once");

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
fn doctor_deep_preflights_proc_unmatched_scenarios() {
    let ws = temp_workspace("doctor-proc-preflight");
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
        "doctor".into(),
        "--deep".into(),
        "--scenario".into(),
        scenario.to_string_lossy().to_string(),
        "--runs".into(),
        "2".into(),
        "--seed".into(),
        "7".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(2), "strict doctor should fail");

    let doc = parse_first_json_stdout(&out);
    let issue = doc
        .get("issues")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .expect("doctor issue");
    assert_eq!(
        issue.get("code").and_then(|v| v.as_str()),
        Some("proc_unmatched_preflight")
    );
    let hint = issue
        .get("hint")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        hint.contains("Add a `proc_when` step"),
        "expected doctor hint to carry proc_when guidance, got: {hint}"
    );
}

#[test]
fn gate_doctor_deep_surfaces_issue_detail() {
    let ws = temp_workspace("gate-doctor-detail");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("repo-owned.fozzy.json"),
        r#"{
          "version":1,
          "name":"repo-owned-proc",
          "steps":[
            {"type":"proc_spawn","cmd":"cargo","args":["test"]}
          ]
        }"#,
    )
    .expect("write scenario");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--seed",
            "7",
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");
    assert_eq!(out.status.code(), Some(1), "gate should fail doctor_deep");
    let doc = parse_json_stdout(&out);
    assert_eq!(full_step_status(&doc, "doctor_deep").as_deref(), Some("failed"));
    let detail = full_step_detail(&doc, "doctor_deep").expect("doctor detail");
    assert!(detail.contains("proc_unmatched_preflight"));
    assert!(detail.contains("Add a `proc_when` step"));
}

#[test]
fn full_allow_expected_failures_controls_shrink_status_for_fail_class_runs() {
    let ws = temp_workspace("full-allow-expected-failures");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("intentional-fail.fozzy.json"),
        r#"{
          "version":1,
          "name":"intentional-fail",
          "steps":[
            {"type":"trace_event","name":"start"},
            {"type":"fail","message":"expected failure"}
          ]
        }"#,
    )
    .expect("write fail scenario");

    let mut common = vec![
        "full".to_string(),
        "--scenario-root".to_string(),
        scenario_root.to_string_lossy().to_string(),
        "--seed".to_string(),
        "7".to_string(),
        "--doctor-runs".to_string(),
        "2".to_string(),
        "--fuzz-time".to_string(),
        "10ms".to_string(),
        "--required-steps".to_string(),
        "run_record_trace,replay,ci,shrink".to_string(),
        "--json".to_string(),
    ];

    let no_allow = run_cli(&common);
    assert_eq!(
        no_allow.status.code(),
        Some(1),
        "full should fail without --allow-expected-failures: {}",
        String::from_utf8_lossy(&no_allow.stderr)
    );
    let no_allow_doc = parse_json_stdout(&no_allow);
    assert_eq!(
        full_step_status(&no_allow_doc, "run_record_trace"),
        Some("failed".to_string())
    );
    assert_eq!(
        full_step_status(&no_allow_doc, "replay"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&no_allow_doc, "ci"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&no_allow_doc, "shrink"),
        Some("failed".to_string())
    );
    assert_eq!(
        full_step_status(&no_allow_doc, "replay_shrunk"),
        Some("skipped".to_string())
    );
    assert_eq!(
        no_allow_doc
            .get("shrinkClassification")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        Some("policy_rejected_non_pass".to_string())
    );

    common.insert(1, "--allow-expected-failures".to_string());
    let allow = run_cli(&common);
    assert_eq!(
        allow.status.code(),
        Some(1),
        "full should still fail when the primary scenario itself is non-pass: {}",
        String::from_utf8_lossy(&allow.stderr)
    );
    let allow_doc = parse_json_stdout(&allow);
    assert_eq!(
        full_step_status(&allow_doc, "run_record_trace"),
        Some("failed".to_string())
    );
    assert_eq!(
        full_step_status(&allow_doc, "replay"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&allow_doc, "ci"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&allow_doc, "shrink"),
        Some("passed".to_string())
    );
    assert_eq!(
        full_step_status(&allow_doc, "replay_shrunk"),
        Some("skipped".to_string())
    );
    assert_eq!(
        allow_doc
            .get("shrinkClassification")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        Some("expected_fail_class_preserved".to_string())
    );
}

#[test]
fn full_rejects_non_pass_primary_summaries_even_without_strict_warnings() {
    let ws = temp_workspace("full-non-pass-primary");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("fail.fozzy.json"),
        r#"{
          "version":1,
          "name":"fail-fast",
          "steps":[
            {"type":"assert_ok","value":false}
          ]
        }"#,
    )
    .expect("write fail scenario");

    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        scenario_root.to_string_lossy().to_string(),
        "--scenario-filter".into(),
        "fail.fozzy.json".into(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--required-steps".into(),
        "test_det,run_record_trace".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(1), "full should fail for non-pass primary scenario");
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "test_det").as_deref(),
        Some("failed"),
        "test_det should fail when the test summary is non-pass"
    );
    assert_eq!(
        full_step_status(&doc, "run_record_trace").as_deref(),
        Some("failed"),
        "run_record_trace should fail when the recorded run summary is non-pass"
    );
    assert!(
        full_step_detail(&doc, "test_det")
            .unwrap_or_default()
            .contains("status=Fail"),
        "test_det detail should surface the underlying fail status"
    );
    assert!(
        full_step_detail(&doc, "run_record_trace")
            .unwrap_or_default()
            .contains("status=Fail"),
        "run_record_trace detail should surface the underlying fail status"
    );
}

#[test]
fn full_report_query_rejects_non_pass_primary_status() {
    let ws = temp_workspace("full-report-query-fail-status");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("fail.fozzy.json"),
        r#"{
          "version":1,
          "name":"fail-fast",
          "steps":[
            {"type":"assert_ok","value":false}
          ]
        }"#,
    )
    .expect("write fail scenario");

    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        scenario_root.to_string_lossy().to_string(),
        "--scenario-filter".into(),
        "fail.fozzy.json".into(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--required-steps".into(),
        "run_record_trace,report_query".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(1), "full should fail for non-pass primary scenario");
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "report_query").as_deref(),
        Some("failed"),
        "report_query should fail when the queried run status is non-pass"
    );
    assert_eq!(
        full_step_detail(&doc, "report_query").as_deref(),
        Some(".status=fail"),
        "report_query should surface the queried fail status verbatim"
    );
}

#[test]
fn full_memory_graph_skips_empty_graph_evidence() {
    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        "tests".into(),
        "--scenario-filter".into(),
        "example.fozzy.json".into(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--required-steps".into(),
        "run_record_trace,memory_graph".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(0), "full example flow should complete");
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "memory_graph").as_deref(),
        Some("skipped"),
        "memory_graph should skip when the graph payload has no nodes or edges"
    );
    assert_eq!(
        full_step_detail(&doc, "memory_graph").as_deref(),
        Some("nodes=0 edges=0"),
        "memory_graph should still report the empty graph evidence explicitly"
    );
}

#[test]
fn full_ci_surfaces_concrete_detail() {
    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        "tests".into(),
        "--scenario-filter".into(),
        "memory.pass".into(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--required-steps".into(),
        "run_record_trace,ci".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "full should complete for CI detail proof: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "ci").as_deref(),
        Some("passed"),
        "ci should pass on clean trace"
    );
    let detail = full_step_detail(&doc, "ci").expect("ci detail");
    assert!(
        detail.contains("checks=") && detail.contains("failed=<none>"),
        "ci detail should surface concrete check detail instead of a lossy count, got: {detail}"
    );
}

#[test]
fn full_fails_when_no_scenarios_are_discovered() {
    let ws = temp_workspace("full-no-scenarios");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");

    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        scenario_root.to_string_lossy().to_string(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "full should fail when no scenarios are discovered: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "discover_scenarios"),
        Some("failed".to_string())
    );
    assert!(
        full_step_detail(&doc, "discover_scenarios")
            .unwrap_or_default()
            .contains("step_scenarios=0"),
        "discover_scenarios should report empty discovery detail"
    );
}

#[test]
fn full_fails_discover_when_only_distributed_scenarios_exist() {
    let ws = temp_workspace("full-distributed-only");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("distributed.fozzy.json"),
        r#"{
          "version":1,
          "name":"distributed-only",
          "distributed":{
            "node_count":2,
            "steps":[{"type":"tick","duration":"1ms"}],
            "invariants":[]
          }
        }"#,
    )
    .expect("write distributed scenario");

    let out = run_cli(&[
        "full".into(),
        "--scenario-root".into(),
        scenario_root.to_string_lossy().to_string(),
        "--seed".into(),
        "7".into(),
        "--doctor-runs".into(),
        "2".into(),
        "--fuzz-time".into(),
        "10ms".into(),
        "--json".into(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "full should fail when only distributed scenarios are discovered: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "discover_scenarios").as_deref(),
        Some("failed"),
        "discover_scenarios should fail when there are no executable step scenarios"
    );
    assert!(
        full_step_detail(&doc, "discover_scenarios")
            .unwrap_or_default()
            .contains("step_scenarios=0 distributed_scenarios=1"),
        "discover_scenarios should report the distributed-only shape"
    );
    assert!(
        doc.get("guidance")
            .and_then(|v| v.as_array())
            .is_some_and(|items| items.iter().filter_map(|v| v.as_str()).any(|s| s.contains("distributed-only roots cannot exercise"))),
        "full guidance should explain why distributed-only roots are insufficient"
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
        &["replay".into(), trace.display().to_string(), "--json".into()],
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
fn full_flags_conflict_with_required_steps_surfaces_policy_conflict() {
    let ws = temp_workspace("full-policy-conflict");
    let tests_dir = ws.join("tests");
    std::fs::create_dir_all(&tests_dir).expect("mkdir tests");
    std::fs::write(
        tests_dir.join("app.pass.fozzy.json"),
        fixture("example.fozzy.json"),
    )
    .expect("write scenario");
    let out = run_cli(&[
        "--cwd".into(),
        ws.to_string_lossy().to_string(),
        "full".into(),
        "--scenario-root".into(),
        "tests".into(),
        "--required-steps".into(),
        "usage,version,test_det".into(),
        "--require-topology-coverage".into(),
        ".".into(),
        "--json".into(),
    ]);
    assert_eq!(out.status.code(), Some(1));
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "policy_conflict"),
        Some("failed".to_string())
    );
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

#[test]
fn gate_targeted_profile_runs_scoped_strict_bundle() {
    let ws = temp_workspace("gate-targeted");
    let tests_dir = ws.join("tests");
    std::fs::create_dir_all(&tests_dir).expect("mkdir tests");
    std::fs::write(
        tests_dir.join("gateway.pass.fozzy.json"),
        br#"{
  "version": 1,
  "name": "gateway-pass",
  "steps": [
    { "type": "assert_eq_int", "a": 1, "b": 1 }
  ]
}"#,
    )
    .expect("write gateway scenario");
    std::fs::write(
        tests_dir.join("other.pass.fozzy.json"),
        br#"{
  "version": 1,
  "name": "other-pass",
  "steps": [
    { "type": "assert_eq_int", "a": 2, "b": 2 }
  ]
}"#,
    )
    .expect("write other scenario");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            tests_dir.to_str().expect("tests str"),
            "--scope",
            "gateway",
            "--seed",
            "1337",
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");
    assert_eq!(
        out.status.code(),
        Some(1),
        "gate stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        doc.get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.gate_report.v1"
    );
    assert_eq!(
        doc.get("profile")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "targeted"
    );
    assert_eq!(
        doc.get("matchedScenarios")
            .and_then(|v| v.as_array())
            .map(|v| v.len())
            .unwrap_or_default(),
        1
    );
    assert_eq!(
        full_step_status(&doc, "clean_tree").as_deref(),
        Some("skipped"),
        "clean_tree should be skipped outside a git repo"
    );
    let profile_top = full_step_detail(&doc, "profile_top").expect("profile_top detail");
    assert_eq!(
        full_step_status(&doc, "profile_top").as_deref(),
        Some("failed"),
        "profile_top should fail when requested domains are empty"
    );
    assert!(
        profile_top.contains("warnings=<none>")
            && profile_top.contains("empty_domains=")
            && profile_top.contains("heap:no heap samples in trace")
            && profile_top.contains("io:no io events in trace"),
        "profile_top should report concrete profile evidence, got: {profile_top}"
    );
    let trace_verify = full_step_detail(&doc, "trace_verify").expect("trace_verify detail");
    assert!(
        trace_verify.contains("warnings=<none>"),
        "trace_verify should report concrete warning detail, got: {trace_verify}"
    );
    let profile_diff = full_step_detail(&doc, "profile_diff").expect("profile_diff detail");
    assert!(
        profile_diff.contains("verdict=")
            && profile_diff.contains("regressions=")
            && profile_diff.contains("significant_regressions="),
        "profile_diff should report concrete diff evidence, got: {profile_diff}"
    );
    let profile_explain =
        full_step_detail(&doc, "profile_explain").expect("profile_explain detail");
    let profile_explain_status = full_step_status(&doc, "profile_explain");
    if profile_explain.contains("cause_domain=unknown") || profile_explain.contains("shifted_path=n/a")
    {
        assert_eq!(
            profile_explain_status.as_deref(),
            Some("skipped"),
            "profile_explain should skip when no concrete diagnosis exists"
        );
    } else {
        assert_eq!(
            profile_explain_status.as_deref(),
            Some("passed"),
            "profile_explain should pass when it reports a concrete diagnosis"
        );
    }
    assert!(
        profile_explain.contains("cause_domain=") && profile_explain.contains("shifted_path="),
        "profile_explain should report concrete explain evidence, got: {profile_explain}"
    );
}

#[test]
fn gate_rejects_non_pass_primary_summaries_even_without_strict_warnings() {
    let ws = temp_workspace("gate-non-pass-primary");
    let tests_dir = ws.join("tests");
    std::fs::create_dir_all(&tests_dir).expect("mkdir tests");
    std::fs::write(
        tests_dir.join("fail.fozzy.json"),
        r#"{
          "version":1,
          "name":"fail-fast",
          "steps":[
            {"type":"assert_ok","value":false}
          ]
        }"#,
    )
    .expect("write fail scenario");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            tests_dir.to_str().expect("tests str"),
            "--scope",
            "fail.fozzy.json",
            "--seed",
            "7",
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");
    assert_eq!(out.status.code(), Some(1), "gate should fail for non-pass primary scenario");
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "test_det_strict").as_deref(),
        Some("failed"),
        "test_det_strict should fail when the suite summary is non-pass"
    );
    assert_eq!(
        full_step_status(&doc, "run_record_trace").as_deref(),
        Some("failed"),
        "run_record_trace should fail when the recorded run summary is non-pass"
    );
    assert!(
        full_step_detail(&doc, "test_det_strict")
            .unwrap_or_default()
            .contains("status=Fail"),
        "test_det_strict detail should surface the underlying fail status"
    );
    assert!(
        full_step_detail(&doc, "run_record_trace")
            .unwrap_or_default()
            .contains("status=Fail"),
        "run_record_trace detail should surface the underlying fail status"
    );
}

#[test]
fn gate_fails_when_no_scenarios_are_discovered() {
    let ws = temp_workspace("gate-no-scenarios");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--seed",
            "7",
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");
    assert_eq!(
        out.status.code(),
        Some(1),
        "gate should fail when no scenarios are discovered: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "discover").as_deref(),
        Some("failed"),
        "discover should fail on an empty scenario root"
    );
    assert_eq!(
        full_step_status(&doc, "scope_match").as_deref(),
        Some("failed"),
        "scope_match should also fail when no step scenarios are available"
    );
}

#[test]
fn gate_fails_discover_when_only_distributed_scenarios_exist() {
    let ws = temp_workspace("gate-distributed-only");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    std::fs::write(
        scenario_root.join("distributed.fozzy.json"),
        r#"{
          "version":1,
          "name":"distributed-only",
          "distributed":{
            "node_count":2,
            "steps":[{"type":"tick","duration":"1ms"}],
            "invariants":[]
          }
        }"#,
    )
    .expect("write distributed scenario");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--seed",
            "7",
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");
    assert_eq!(
        out.status.code(),
        Some(1),
        "gate should fail when only distributed scenarios are discovered: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "discover").as_deref(),
        Some("failed"),
        "discover should fail when gate has no executable step scenarios"
    );
    assert!(
        full_step_detail(&doc, "discover")
            .unwrap_or_default()
            .contains("step_scenarios=0 distributed_scenarios=1"),
        "discover should report that only distributed scenarios were found"
    );
    assert_eq!(
        full_step_status(&doc, "scope_match").as_deref(),
        Some("failed"),
        "scope_match should still fail when no step scenarios are available"
    );
}

#[test]
fn profile_golden_run_top_flame_timeline_export_flow() {
    let ws = temp_workspace("profile-golden-flow");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("write scenario");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let trace = ws.join("golden.trace.fozzy");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.to_string_lossy().to_string(),
            "--det".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--record".into(),
            trace.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "run stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(trace.exists(), "expected recorded trace");

    let top = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "top".into(),
            trace.to_string_lossy().to_string(),
            "--heap".into(),
            "--latency".into(),
            "--io".into(),
            "--sched".into(),
            "--limit".into(),
            "10".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        top.status.code(),
        Some(0),
        "profile top stderr={}",
        String::from_utf8_lossy(&top.stderr)
    );
    let top_doc = parse_json_stdout(&top);
    assert_eq!(
        top_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_top.v1"
    );
    assert!(top_doc.get("heap").is_some());
    assert!(top_doc.get("latency").is_some());

    let folded_out = ws.join("heap.folded.txt");
    let flame = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "flame".into(),
            trace.to_string_lossy().to_string(),
            "--heap".into(),
            "--format".into(),
            "folded".into(),
            "--out".into(),
            folded_out.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        flame.status.code(),
        Some(0),
        "profile flame stderr={}",
        String::from_utf8_lossy(&flame.stderr)
    );
    assert!(folded_out.exists(), "folded output must exist");
    assert!(
        std::fs::metadata(&folded_out)
            .map(|m| m.len() > 0)
            .unwrap_or(false),
        "folded output should be non-empty"
    );

    let timeline_out = ws.join("timeline.json");
    let timeline = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "timeline".into(),
            trace.to_string_lossy().to_string(),
            "--format".into(),
            "json".into(),
            "--out".into(),
            timeline_out.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        timeline.status.code(),
        Some(0),
        "profile timeline stderr={}",
        String::from_utf8_lossy(&timeline.stderr)
    );
    assert!(timeline_out.exists(), "timeline output must exist");

    let export_out = ws.join("profile.otlp.json");
    let export = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "export".into(),
            trace.to_string_lossy().to_string(),
            "--format".into(),
            "otlp".into(),
            "--out".into(),
            export_out.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        export.status.code(),
        Some(0),
        "profile export stderr={}",
        String::from_utf8_lossy(&export.stderr)
    );
    assert!(export_out.exists(), "profile export output must exist");
}

#[test]
fn profile_record_replay_diff_explain_and_artifact_parity_flow() {
    let ws = temp_workspace("profile-diff-flow");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let left_scenario = ws.join("left.fozzy.json");
    let right_scenario = ws.join("right.fozzy.json");
    std::fs::write(&left_scenario, fixture("example.fozzy.json")).expect("left scenario");
    std::fs::write(&right_scenario, fixture("memory.pass.fozzy.json")).expect("right scenario");
    let left_trace = ws.join("left.trace.fozzy");
    let right_trace = ws.join("right.trace.fozzy");

    let mut left_run_id = String::new();
    let mut right_run_id = String::new();
    for (idx, (scenario, trace, seed)) in [
        (left_scenario.as_path(), left_trace.as_path(), "7"),
        (right_scenario.as_path(), right_trace.as_path(), "13"),
    ]
    .into_iter()
    .enumerate()
    {
        let run = run_cli_in(
            &ws,
            &[
                "run".into(),
                scenario.to_string_lossy().to_string(),
                "--det".into(),
                "--seed".into(),
                seed.into(),
                "--profile-capture".into(),
                "full".into(),
                "--record".into(),
                trace.to_string_lossy().to_string(),
                "--config".into(),
                cfg.to_string_lossy().to_string(),
                "--json".into(),
            ],
        );
        assert_eq!(
            run.status.code(),
            Some(0),
            "run stderr={}",
            String::from_utf8_lossy(&run.stderr)
        );
        let run_doc = parse_json_stdout(&run);
        if idx == 0 {
            left_run_id = json_run_id(&run_doc);
        } else {
            right_run_id = json_run_id(&run_doc);
        }
        let replay = run_cli_in(
            &ws,
            &[
                "replay".into(),
                trace.to_string_lossy().to_string(),
                "--profile-capture".into(),
                "full".into(),
                "--config".into(),
                cfg.to_string_lossy().to_string(),
                "--json".into(),
            ],
        );
        assert_eq!(
            replay.status.code(),
            Some(0),
            "replay stderr={}",
            String::from_utf8_lossy(&replay.stderr)
        );
    }

    let diff = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "diff".into(),
            left_trace.to_string_lossy().to_string(),
            right_trace.to_string_lossy().to_string(),
            "--heap".into(),
            "--latency".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        diff.status.code(),
        Some(0),
        "profile diff stderr={}",
        String::from_utf8_lossy(&diff.stderr)
    );
    let diff_doc = parse_json_stdout(&diff);
    assert_eq!(
        diff_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_diff.v2"
    );
    assert!(diff_doc.get("regressions").is_some());

    let explain = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "explain".into(),
            left_trace.to_string_lossy().to_string(),
            "--diff-with".into(),
            right_trace.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        explain.status.code(),
        Some(0),
        "profile explain stderr={}",
        String::from_utf8_lossy(&explain.stderr)
    );
    let explain_doc = parse_json_stdout(&explain);
    assert_eq!(
        explain_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_explain.v1"
    );

    let ls = run_cli_in(
        &ws,
        &[
            "artifacts".into(),
            "ls".into(),
            left_run_id.clone(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        ls.status.code(),
        Some(0),
        "artifacts ls stderr={}",
        String::from_utf8_lossy(&ls.stderr)
    );
    let ls_stdout = String::from_utf8_lossy(&ls.stdout);
    assert!(ls_stdout.contains("profile.timeline.json"));
    assert!(ls_stdout.contains("profile.cpu.json"));
    assert!(ls_stdout.contains("profile.heap.json"));
    assert!(ls_stdout.contains("profile.latency.json"));
    assert!(ls_stdout.contains("profile.metrics.json"));

    let adiff = run_cli_in(
        &ws,
        &[
            "artifacts".into(),
            "diff".into(),
            left_run_id.clone(),
            right_run_id.clone(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        adiff.status.code(),
        Some(0),
        "artifacts diff stderr={}",
        String::from_utf8_lossy(&adiff.stderr)
    );
    let adiff_doc = parse_json_stdout(&adiff);
    assert_eq!(
        adiff_doc
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "diff"
    );

    let export_zip = ws.join("artifacts.export.zip");
    let pack_zip = ws.join("artifacts.pack.zip");
    let bundle_zip = ws.join("artifacts.bundle.zip");
    for (sub, out, target) in [
        ("export", &export_zip, left_run_id.clone()),
        ("pack", &pack_zip, left_run_id.clone()),
        (
            "bundle",
            &bundle_zip,
            left_trace.to_string_lossy().to_string(),
        ),
    ] {
        let out_cmd = run_cli_in(
            &ws,
            &[
                "artifacts".into(),
                sub.into(),
                target,
                "--out".into(),
                out.to_string_lossy().to_string(),
                "--config".into(),
                cfg.to_string_lossy().to_string(),
                "--json".into(),
            ],
        );
        assert_eq!(
            out_cmd.status.code(),
            Some(0),
            "artifacts {sub} stderr={}",
            String::from_utf8_lossy(&out_cmd.stderr)
        );
        let size = std::fs::metadata(out).expect("zip metadata").len();
        assert!(size > 0, "zip should be non-empty");
        assert!(size <= 8 * 1024 * 1024, "zip should stay within budget");
    }
}

#[test]
fn profile_strict_and_unsafe_legacy_behavior_and_capture_mode_budgets() {
    let ws = temp_workspace("profile-strict-unsafe");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let missing = ws.join("missing.trace.fozzy");

    let strict = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "top".into(),
            missing.to_string_lossy().to_string(),
            "--heap".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_ne!(strict.status.code(), Some(0), "strict should fail");

    let unsafe_out = run_cli_in(
        &ws,
        &[
            "--unsafe".into(),
            "profile".into(),
            "top".into(),
            missing.to_string_lossy().to_string(),
            "--heap".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(unsafe_out.status.code(), Some(0), "unsafe should warn");
    let unsafe_doc = parse_json_stdout(&unsafe_out);
    assert_eq!(
        unsafe_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_contract_warning.v1"
    );

    let run_dir = ws.join(".fozzy/runs/legacy-partial");
    std::fs::create_dir_all(&run_dir).expect("legacy run dir");
    std::fs::write(
        run_dir.join("profile.metrics.json"),
        br#"{"schemaVersion":"fozzy.profile_metrics.v2","runId":"legacy","timeDomains":{"virtualTime":"deterministic","hostMonotonicTime":"host"},"virtualTimeMs":0,"hostTimeMs":0,"cpuTimeMs":0,"allocBytes":0,"inUseBytes":0,"p50LatencyMs":0,"p95LatencyMs":0,"p99LatencyMs":0,"maxLatencyMs":0,"ioOps":0,"schedOps":0}"#,
    )
    .expect("legacy metrics");

    let legacy_strict = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "top".into(),
            "legacy-partial".into(),
            "--heap".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_ne!(
        legacy_strict.status.code(),
        Some(0),
        "strict should fail for legacy partial artifacts"
    );
    let legacy_unsafe = run_cli_in(
        &ws,
        &[
            "--unsafe".into(),
            "profile".into(),
            "top".into(),
            "legacy-partial".into(),
            "--heap".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        legacy_unsafe.status.code(),
        Some(0),
        "unsafe should downgrade legacy contract error"
    );

    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    for (level, expect_profile) in [("baseline", false), ("full", true)] {
        let out = run_cli_in(
            &ws,
            &[
                "run".into(),
                scenario.to_string_lossy().to_string(),
                "--det".into(),
                "--seed".into(),
                "7".into(),
                "--profile-capture".into(),
                level.into(),
                "--config".into(),
                cfg.to_string_lossy().to_string(),
                "--json".into(),
            ],
        );
        assert_eq!(
            out.status.code(),
            Some(0),
            "run ({level}) stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
        let doc = parse_json_stdout(&out);
        let run_id = json_run_id(&doc);
        let metrics_path = ws
            .join(".fozzy")
            .join("runs")
            .join(run_id)
            .join("profile.metrics.json");
        assert_eq!(
            metrics_path.exists(),
            expect_profile,
            "profile artifact policy mismatch for {level}"
        );
        if expect_profile {
            let size = std::fs::metadata(&metrics_path)
                .expect("metrics metadata")
                .len();
            assert!(size > 0, "metrics artifact should be non-empty");
            assert!(
                size < 2 * 1024 * 1024,
                "metrics artifact should stay bounded"
            );
        }
    }
}

#[test]
fn report_show_omits_profile_diagnosis_when_only_contract_warning_is_available() {
    let ws = temp_workspace("report-profile-diagnosis-contract-warning");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");

    let run_dir = ws.join(".fozzy/runs/legacy-report");
    std::fs::create_dir_all(&run_dir).expect("legacy run dir");
    std::fs::write(
        run_dir.join("report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "pass",
            "mode": "run",
            "identity": {
                "runId": "legacy-report",
                "seed": 7
            },
            "startedAt": "2026-01-01T00:00:00Z",
            "finishedAt": "2026-01-01T00:00:00Z",
            "durationMs": 1,
            "durationNs": 1000000
        }))
        .expect("report json"),
    )
    .expect("write report");
    std::fs::write(
        run_dir.join("profile.metrics.json"),
        br#"{"schemaVersion":"fozzy.profile_metrics.v2","runId":"legacy-report","timeDomains":{"virtualTime":"deterministic","hostMonotonicTime":"host"},"virtualTimeMs":0,"hostTimeMs":0,"cpuTimeMs":0,"allocBytes":0,"inUseBytes":0,"p50LatencyMs":0,"p95LatencyMs":0,"p99LatencyMs":0,"maxLatencyMs":0,"ioOps":0,"schedOps":0}"#,
    )
    .expect("legacy metrics");

    let out = run_cli_in(
        &ws,
        &[
            "report".into(),
            "show".into(),
            "legacy-report".into(),
            "--format".into(),
            "json".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "report show stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert!(
        doc.get("profileDiagnosis").is_none(),
        "contract warning should not be injected as profile diagnosis"
    );
}

#[test]
fn report_show_omits_profile_diagnosis_for_single_run_summary_only() {
    let ws = temp_workspace("report-show-no-single-run-diagnosis");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("write scenario");
    let trace = ws.join("pass.trace.fozzy");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.to_string_lossy().to_string(),
            "--det".into(),
            "--record".into(),
            trace.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(run.status.code(), Some(0), "run should succeed");

    let report = run_cli_in(
        &ws,
        &[
            "report".into(),
            "show".into(),
            trace.to_string_lossy().to_string(),
            "--format".into(),
            "json".into(),
            "--json".into(),
        ],
    );
    assert_eq!(report.status.code(), Some(0), "report show should succeed");
    let doc = parse_json_stdout(&report);
    assert!(
        doc.get("profileDiagnosis").is_none(),
        "single-run profile summary should not be injected as a diagnosis"
    );
}
