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

fn resolve_output_path(base: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

fn resolve_identity_artifacts_dir(base: &Path, out: &serde_json::Value) -> PathBuf {
    resolve_output_path(
        base,
        out.get("identity")
            .and_then(|v| v.get("artifactsDir"))
            .and_then(|v| v.as_str())
            .expect("artifacts dir"),
    )
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

#[path = "cli_parity/cli_contract.rs"]
mod cli_contract;
#[path = "cli_parity/host_backends.rs"]
mod host_backends;
#[path = "cli_parity/trace_artifacts.rs"]
mod trace_artifacts;
#[path = "cli_parity/gate_full.rs"]
mod gate_full;
#[path = "cli_parity/profile_report.rs"]
mod profile_report;
