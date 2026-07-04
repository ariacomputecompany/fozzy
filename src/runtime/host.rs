use std::collections::BTreeMap;
use std::io::Read;
use std::process::Stdio;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use crate::{Finding, FindingKind};

#[derive(Debug)]
pub(crate) enum HostProcDispatch {
    Completed(HostProcOutput),
    TimedOut {
        stdout: String,
        stderr: String,
        peak_rss_bytes: u64,
        rss_sample_count: u64,
    },
}

#[derive(Debug)]
pub(crate) struct HostProcOutput {
    pub(crate) exit_code: i32,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) peak_rss_bytes: u64,
    pub(crate) rss_sample_count: u64,
}

#[derive(Debug)]
pub(crate) enum HostHttpDispatch {
    Completed(HostHttpResponse),
    TimedOut,
}

#[derive(Debug, Clone)]
pub(crate) struct HostHttpResponse {
    pub(crate) status: u16,
    pub(crate) headers: BTreeMap<String, String>,
    pub(crate) body: String,
    pub(crate) request_kind: String,
    pub(crate) completion_boundary: String,
    pub(crate) upgrade_accepted: bool,
}

#[derive(Debug)]
enum StreamReadError {
    Io(String),
    LimitExceeded { observed: usize, limit: usize },
}

const HOST_PROC_MAX_STDOUT_BYTES: usize = 8 * 1024 * 1024;
const HOST_PROC_MAX_STDERR_BYTES: usize = 8 * 1024 * 1024;
const HOST_HTTP_MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const HOST_PROC_MEMORY_SAMPLE_INTERVAL_MS: u64 = 25;

#[derive(Debug, Default, Clone, Copy)]
struct HostProcMemoryStats {
    peak_rss_bytes: u64,
    sample_count: u64,
}

#[cfg(unix)]
fn sample_host_proc_tree_rss_bytes(root_pid: u32) -> Option<u64> {
    let output = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,rss="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let mut children = BTreeMap::<u32, Vec<u32>>::new();
    let mut rss_kb = BTreeMap::<u32, u64>::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let mut cols = line.split_whitespace();
        let (Some(pid), Some(ppid), Some(rss)) = (cols.next(), cols.next(), cols.next()) else {
            continue;
        };
        let (Ok(pid), Ok(ppid), Ok(rss)) =
            (pid.parse::<u32>(), ppid.parse::<u32>(), rss.parse::<u64>())
        else {
            continue;
        };
        rss_kb.insert(pid, rss);
        children.entry(ppid).or_default().push(pid);
    }

    if !rss_kb.contains_key(&root_pid) {
        return None;
    }

    let mut total_kb = 0u64;
    let mut stack = vec![root_pid];
    while let Some(pid) = stack.pop() {
        total_kb = total_kb.saturating_add(rss_kb.get(&pid).copied().unwrap_or(0));
        if let Some(descendants) = children.get(&pid) {
            stack.extend(descendants.iter().copied());
        }
    }
    Some(total_kb.saturating_mul(1024))
}

#[cfg(not(unix))]
fn sample_host_proc_tree_rss_bytes(_root_pid: u32) -> Option<u64> {
    None
}

fn record_host_proc_memory_sample(stats: &mut HostProcMemoryStats, root_pid: u32) {
    let Some(bytes) = sample_host_proc_tree_rss_bytes(root_pid) else {
        return;
    };
    stats.sample_count = stats.sample_count.saturating_add(1);
    stats.peak_rss_bytes = stats.peak_rss_bytes.max(bytes);
}

fn host_http_response_has_no_body(method: &str, status: u16) -> bool {
    method.eq_ignore_ascii_case("HEAD")
        || (100..200).contains(&status)
        || status == 204
        || status == 304
}

pub(crate) fn host_http_request_kind(
    method: &str,
    headers: &BTreeMap<String, String>,
) -> &'static str {
    if method.eq_ignore_ascii_case("GET") && host_http_upgrade_requested(headers) {
        "websocket_upgrade"
    } else {
        "http_request"
    }
}

pub(crate) fn host_http_upgrade_requested(headers: &BTreeMap<String, String>) -> bool {
    headers
        .get("connection")
        .is_some_and(|value| value.to_ascii_lowercase().contains("upgrade"))
        && headers
            .get("upgrade")
            .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
}

pub(crate) fn host_http_upgrade_accepted(status: u16, headers: &BTreeMap<String, String>) -> bool {
    status == 101
        && headers
            .get("upgrade")
            .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
}

pub(crate) fn host_http_request_details(
    method: &str,
    path: &str,
    headers: &BTreeMap<String, String>,
) -> serde_json::Value {
    serde_json::json!({
        "requestKind": host_http_request_kind(method, headers),
        "method": method,
        "path": path,
        "upgradeRequested": host_http_upgrade_requested(headers),
        "headers": headers,
    })
}

fn spawn_stream_reader<T>(
    mut stream: T,
    max_bytes: usize,
) -> (
    Arc<Mutex<Vec<u8>>>,
    mpsc::Receiver<Result<(), StreamReadError>>,
)
where
    T: Read + Send + 'static,
{
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let writer = Arc::clone(&buffer);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut chunk = [0u8; 8192];
        let mut total = 0usize;
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => {
                    let _ = tx.send(Ok(()));
                    return;
                }
                Ok(n) => {
                    total = total.saturating_add(n);
                    if let Ok(mut guard) = writer.lock() {
                        let remaining = max_bytes.saturating_sub(guard.len());
                        guard.extend_from_slice(&chunk[..n.min(remaining)]);
                    }
                    if total > max_bytes {
                        let _ = tx.send(Err(StreamReadError::LimitExceeded {
                            observed: total,
                            limit: max_bytes,
                        }));
                        return;
                    }
                }
                Err(err) => {
                    let _ = tx.send(Err(StreamReadError::Io(err.to_string())));
                    return;
                }
            }
        }
    });
    (buffer, rx)
}

fn snapshot_stream(buffer: &Arc<Mutex<Vec<u8>>>) -> Vec<u8> {
    buffer.lock().map(|guard| guard.clone()).unwrap_or_default()
}

fn wait_stream_reader(
    rx: &mpsc::Receiver<Result<(), StreamReadError>>,
    label: &str,
    invocation: &str,
) -> Result<(), String> {
    match rx.recv_timeout(Duration::from_secs(1)) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(StreamReadError::Io(err))) => Err(format!(
            "host proc {label} read failed for {invocation}: {err}"
        )),
        Ok(Err(StreamReadError::LimitExceeded { observed, limit })) => Err(format!(
            "host proc {label} exceeded limit for {invocation}: {observed} bytes > {limit} bytes"
        )),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "host proc {label} reader did not flush after process exit for {invocation}"
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(format!(
            "host proc {label} reader disconnected for {invocation}"
        )),
    }
}

fn poll_stream_reader(
    rx: &mpsc::Receiver<Result<(), StreamReadError>>,
    label: &str,
    invocation: &str,
) -> Result<bool, String> {
    match rx.try_recv() {
        Ok(Ok(())) => Ok(true),
        Ok(Err(StreamReadError::Io(err))) => Err(format!(
            "host proc {label} read failed for {invocation}: {err}"
        )),
        Ok(Err(StreamReadError::LimitExceeded { observed, limit })) => Err(format!(
            "host proc {label} exceeded limit for {invocation}: {observed} bytes > {limit} bytes"
        )),
        Err(mpsc::TryRecvError::Empty) => Ok(false),
        Err(mpsc::TryRecvError::Disconnected) => Err(format!(
            "host proc {label} reader disconnected for {invocation}"
        )),
    }
}

pub(crate) fn dispatch_host_proc(
    cmd: &str,
    args: &[String],
    deadline: Option<Instant>,
) -> Result<HostProcDispatch, String> {
    let invocation = if args.is_empty() {
        format!("{cmd:?}")
    } else {
        format!("{cmd:?} {:?}", args)
    };
    let mut child = std::process::Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("host proc spawn failed for {cmd:?} {:?}: {e}", args))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("host proc stdout pipe missing for {invocation}"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("host proc stderr pipe missing for {invocation}"))?;
    let (stdout_buf, stdout_rx) = spawn_stream_reader(stdout, HOST_PROC_MAX_STDOUT_BYTES);
    let (stderr_buf, stderr_rx) = spawn_stream_reader(stderr, HOST_PROC_MAX_STDERR_BYTES);
    let child_pid = child.id();
    let mut memory_stats = HostProcMemoryStats::default();
    let mut next_memory_sample = Instant::now();
    record_host_proc_memory_sample(&mut memory_stats, child_pid);

    loop {
        if Instant::now() >= next_memory_sample {
            record_host_proc_memory_sample(&mut memory_stats, child_pid);
            next_memory_sample =
                Instant::now() + Duration::from_millis(HOST_PROC_MEMORY_SAMPLE_INTERVAL_MS);
        }

        if let Some(deadline) = deadline
            && Instant::now() >= deadline
        {
            let _ = child.kill();
            let _ = child.wait();
            let _ = wait_stream_reader(&stdout_rx, "stdout", &invocation);
            let _ = wait_stream_reader(&stderr_rx, "stderr", &invocation);
            return Ok(HostProcDispatch::TimedOut {
                stdout: String::from_utf8_lossy(&snapshot_stream(&stdout_buf)).to_string(),
                stderr: String::from_utf8_lossy(&snapshot_stream(&stderr_buf)).to_string(),
                peak_rss_bytes: memory_stats.peak_rss_bytes,
                rss_sample_count: memory_stats.sample_count,
            });
        }

        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("host proc wait failed for {invocation}: {e}"))?
        {
            wait_stream_reader(&stdout_rx, "stdout", &invocation)?;
            wait_stream_reader(&stderr_rx, "stderr", &invocation)?;
            record_host_proc_memory_sample(&mut memory_stats, child_pid);
            return Ok(HostProcDispatch::Completed(HostProcOutput {
                exit_code: status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&snapshot_stream(&stdout_buf)).to_string(),
                stderr: String::from_utf8_lossy(&snapshot_stream(&stderr_buf)).to_string(),
                peak_rss_bytes: memory_stats.peak_rss_bytes,
                rss_sample_count: memory_stats.sample_count,
            }));
        }

        let stdout_done = poll_stream_reader(&stdout_rx, "stdout", &invocation)?;
        let stderr_done = poll_stream_reader(&stderr_rx, "stderr", &invocation)?;
        if stdout_done && stderr_done {
            let status = child
                .wait()
                .map_err(|e| format!("host proc wait failed for {invocation}: {e}"))?;
            record_host_proc_memory_sample(&mut memory_stats, child_pid);
            return Ok(HostProcDispatch::Completed(HostProcOutput {
                exit_code: status.code().unwrap_or(-1),
                stdout: String::from_utf8_lossy(&snapshot_stream(&stdout_buf)).to_string(),
                stderr: String::from_utf8_lossy(&snapshot_stream(&stderr_buf)).to_string(),
                peak_rss_bytes: memory_stats.peak_rss_bytes,
                rss_sample_count: memory_stats.sample_count,
            }));
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(crate) fn canonical_headers(
    headers: Option<&BTreeMap<String, String>>,
) -> Result<BTreeMap<String, String>, Finding> {
    let mut out = BTreeMap::new();
    let Some(headers) = headers else {
        return Ok(out);
    };
    for (k, v) in headers {
        let key = k.trim().to_ascii_lowercase();
        if key.is_empty() {
            return Err(Finding {
                kind: FindingKind::Checker,
                title: "http_header_invalid".to_string(),
                message: "http header name cannot be empty".to_string(),
                location: None,
            });
        }
        if key.contains('\n') || key.contains('\r') || v.contains('\n') || v.contains('\r') {
            return Err(Finding {
                kind: FindingKind::Checker,
                title: "http_header_invalid".to_string(),
                message: format!("http header contains forbidden newline: {k:?}"),
                location: None,
            });
        }
        out.insert(key, v.to_string());
    }
    Ok(out)
}

pub(crate) fn dispatch_host_http(
    method: &str,
    url: &str,
    headers: &BTreeMap<String, String>,
    body: Option<&str>,
    timeout: Option<Duration>,
) -> Result<HostHttpDispatch, String> {
    let method = method.to_ascii_uppercase();
    if !matches!(
        method.as_str(),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    ) {
        return Err(format!(
            "unsupported host http method {method:?}; expected GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS"
        ));
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!(
            "invalid host http url {url:?}; expected http(s)://<host>[:port]/path"
        ));
    }
    if matches!(timeout, Some(limit) if limit.is_zero()) {
        return Ok(HostHttpDispatch::TimedOut);
    }
    let mut agent = ureq::AgentBuilder::new();
    if let Some(limit) = timeout {
        agent = agent
            .timeout_connect(limit)
            .timeout_read(limit)
            .timeout_write(limit);
    }
    let mut req = agent.build().request(&method, url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    let request_kind = host_http_request_kind(&method, headers).to_string();
    let result = if let Some(payload) = body {
        req.send_string(payload)
    } else {
        req.call()
    };
    let response = match result {
        Ok(resp) => resp,
        Err(ureq::Error::Status(_, resp)) => resp,
        Err(err) => {
            let message = format!("host http request failed for {method} {url}: {err}");
            if message.contains("timed out") || message.contains("timeout") {
                return Ok(HostHttpDispatch::TimedOut);
            }
            return Err(message);
        }
    };
    let mut out_headers = BTreeMap::new();
    for name in response.headers_names() {
        if let Some(val) = response.header(&name) {
            out_headers.insert(name.to_ascii_lowercase(), val.to_string());
        }
    }
    let status_code = response.status();
    let upgrade_accepted = host_http_upgrade_accepted(status_code, &out_headers);
    if host_http_response_has_no_body(&method, status_code) {
        return Ok(HostHttpDispatch::Completed(HostHttpResponse {
            status: status_code,
            headers: out_headers,
            body: String::new(),
            request_kind,
            completion_boundary: if upgrade_accepted {
                "upgrade_headers".to_string()
            } else {
                "http_no_body_semantics".to_string()
            },
            upgrade_accepted,
        }));
    }
    let mut reader = response.into_reader();
    let mut body_bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                body_bytes.extend_from_slice(&chunk[..n]);
                if body_bytes.len() > HOST_HTTP_MAX_BODY_BYTES {
                    return Err(format!(
                        "host http body exceeded limit for {method} {url}: {} bytes > {} bytes",
                        body_bytes.len(),
                        HOST_HTTP_MAX_BODY_BYTES
                    ));
                }
            }
            Err(err) => {
                let message = format!("host http body read failed for {method} {url}: {err}");
                if message.contains("timed out") || message.contains("timeout") {
                    return Ok(HostHttpDispatch::TimedOut);
                }
                return Err(message);
            }
        }
    }
    Ok(HostHttpDispatch::Completed(HostHttpResponse {
        status: status_code,
        headers: out_headers,
        body: String::from_utf8_lossy(&body_bytes).to_string(),
        request_kind,
        completion_boundary: "response_body_complete".to_string(),
        upgrade_accepted,
    }))
}

pub(crate) fn host_http_rule_path_supported(path: &str) -> bool {
    path.starts_with("http://") || path.starts_with("https://") || path.starts_with('/')
}

pub(crate) fn host_http_rule_matches(rule_path: &str, request_url: &str) -> bool {
    if rule_path.starts_with("http://") || rule_path.starts_with("https://") {
        return rule_path == request_url;
    }
    if let Some(request_path) = extract_http_path_and_query(request_url) {
        return request_path == rule_path;
    }
    false
}

fn extract_http_path_and_query(url: &str) -> Option<&str> {
    let rest = if let Some(v) = url.strip_prefix("http://") {
        v
    } else if let Some(v) = url.strip_prefix("https://") {
        v
    } else {
        return None;
    };
    if let Some(idx) = rest.find('/') {
        Some(&rest[idx..])
    } else {
        Some("/")
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn assert_http_when_response_matches_host(
    method: &str,
    path: &str,
    expected_status: u16,
    expected_headers: &BTreeMap<String, String>,
    expected_body: Option<&str>,
    expected_json: Option<&serde_json::Value>,
    status_code: u16,
    headers: &BTreeMap<String, String>,
    body: &str,
) -> Result<(), Finding> {
    if status_code != expected_status {
        return Err(Finding {
            kind: FindingKind::Assertion,
            title: "http_when_host_status".to_string(),
            message: format!(
                "http_when expected status {expected_status} for {method} {path}, got {status_code}"
            ),
            location: None,
        });
    }
    for (k, v) in expected_headers {
        let got = headers.get(k);
        if got != Some(v) {
            return Err(Finding {
                kind: FindingKind::Assertion,
                title: "http_when_host_headers".to_string(),
                message: format!(
                    "http_when header mismatch for {method} {path} header {k:?}: expected {v:?}, got {got:?}"
                ),
                location: None,
            });
        }
    }
    if let Some(expected_body) = expected_body
        && body != expected_body
    {
        return Err(Finding {
            kind: FindingKind::Assertion,
            title: "http_when_host_body".to_string(),
            message: format!("http_when body mismatch for {method} {path}"),
            location: None,
        });
    }
    if let Some(expected_json) = expected_json {
        let got: serde_json::Value = serde_json::from_str(body).map_err(|e| Finding {
            kind: FindingKind::Assertion,
            title: "http_when_host_json_parse".to_string(),
            message: format!("http_when expected json response for {method} {path}: {e}"),
            location: None,
        })?;
        if &got != expected_json {
            let mismatch_details = json_mismatch_details(expected_json, &got);
            let mismatch_count = mismatch_details
                .get("mismatches")
                .and_then(|value| value.as_array())
                .map_or(0, Vec::len);
            return Err(Finding {
                kind: FindingKind::Assertion,
                title: "http_when_host_json".to_string(),
                message: format!(
                    "http_when json mismatch for {method} {path} ({mismatch_count} structural difference(s))"
                ),
                location: Some(crate::FindingLocation {
                    file: None,
                    line: None,
                    col: None,
                    details: Some(mismatch_details),
                }),
            });
        }
    }
    Ok(())
}

fn json_mismatch_details(
    expected: &serde_json::Value,
    actual: &serde_json::Value,
) -> serde_json::Value {
    let mut mismatches = Vec::new();
    collect_json_mismatches("$", expected, actual, &mut mismatches);
    serde_json::json!({
        "expected": expected,
        "actual": actual,
        "mismatches": mismatches,
    })
}

fn collect_json_mismatches(
    path: &str,
    expected: &serde_json::Value,
    actual: &serde_json::Value,
    out: &mut Vec<serde_json::Value>,
) {
    if out.len() >= 64 {
        return;
    }
    match (expected, actual) {
        (serde_json::Value::Object(expected_map), serde_json::Value::Object(actual_map)) => {
            for (key, expected_value) in expected_map {
                let child_path = format!("{path}.{}", escape_json_path_segment(key));
                match actual_map.get(key) {
                    Some(actual_value) => {
                        collect_json_mismatches(&child_path, expected_value, actual_value, out);
                    }
                    None => out.push(serde_json::json!({
                        "kind": "missing_key",
                        "path": child_path,
                        "expected": expected_value,
                    })),
                }
                if out.len() >= 64 {
                    return;
                }
            }
            for (key, actual_value) in actual_map {
                if expected_map.contains_key(key) {
                    continue;
                }
                out.push(serde_json::json!({
                    "kind": "extra_key",
                    "path": format!("{path}.{}", escape_json_path_segment(key)),
                    "actual": actual_value,
                }));
                if out.len() >= 64 {
                    return;
                }
            }
        }
        (serde_json::Value::Array(expected_items), serde_json::Value::Array(actual_items)) => {
            let shared = expected_items.len().min(actual_items.len());
            for idx in 0..shared {
                collect_json_mismatches(
                    &format!("{path}[{idx}]"),
                    &expected_items[idx],
                    &actual_items[idx],
                    out,
                );
                if out.len() >= 64 {
                    return;
                }
            }
            for (idx, expected_value) in expected_items.iter().enumerate().skip(shared) {
                out.push(serde_json::json!({
                    "kind": "missing_index",
                    "path": format!("{path}[{idx}]"),
                    "expected": expected_value,
                }));
                if out.len() >= 64 {
                    return;
                }
            }
            for (idx, actual_value) in actual_items.iter().enumerate().skip(shared) {
                out.push(serde_json::json!({
                    "kind": "extra_index",
                    "path": format!("{path}[{idx}]"),
                    "actual": actual_value,
                }));
                if out.len() >= 64 {
                    return;
                }
            }
        }
        _ if expected != actual => out.push(serde_json::json!({
            "kind": "value_mismatch",
            "path": path,
            "expected": expected,
            "actual": actual,
        })),
        _ => {}
    }
}

fn escape_json_path_segment(segment: &str) -> String {
    if segment
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        segment.to_string()
    } else {
        format!("[{}]", serde_json::Value::String(segment.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    fn spawn_upgrade_response_server() -> (String, mpsc::Sender<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind upgrade listener");
        listener
            .set_nonblocking(true)
            .expect("set nonblocking listener");
        let addr = listener.local_addr().expect("listener addr");
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        thread::spawn(move || {
            let start = Instant::now();
            loop {
                if stop_rx.try_recv().is_ok() || start.elapsed() > Duration::from_secs(5) {
                    break;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buf = [0u8; 2048];
                        let _ = stream.read(&mut buf);
                        let response = concat!(
                            "HTTP/1.1 101 Switching Protocols\r\n",
                            "Connection: Upgrade\r\n",
                            "Upgrade: websocket\r\n",
                            "Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n",
                            "\r\n",
                            "{\"type\":\"status_stream_ready\"}\n"
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                        thread::sleep(Duration::from_secs(2));
                        break;
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        (
            format!("http://{addr}/ws/status?client=test&deviceKey=test"),
            stop_tx,
        )
    }

    #[test]
    fn host_http_upgrade_response_completes_without_draining_stream() {
        let (url, stop_tx) = spawn_upgrade_response_server();
        let response = dispatch_host_http(
            "GET",
            &url,
            &BTreeMap::from([
                ("connection".to_string(), "Upgrade".to_string()),
                ("upgrade".to_string(), "websocket".to_string()),
                ("sec-websocket-version".to_string(), "13".to_string()),
                (
                    "sec-websocket-key".to_string(),
                    "dGhlIHNhbXBsZSBub25jZQ==".to_string(),
                ),
            ]),
            None,
            Some(Duration::from_millis(200)),
        )
        .expect("dispatch upgrade request");
        let _ = stop_tx.send(());
        match response {
            HostHttpDispatch::Completed(resp) => {
                assert_eq!(resp.status, 101);
                assert_eq!(resp.body, "");
                assert_eq!(resp.request_kind, "websocket_upgrade");
                assert_eq!(resp.completion_boundary, "upgrade_headers");
                assert!(resp.upgrade_accepted);
                assert_eq!(
                    resp.headers.get("upgrade").map(String::as_str),
                    Some("websocket")
                );
            }
            HostHttpDispatch::TimedOut => {
                panic!("upgrade response should complete without timing out")
            }
        }
    }

    #[test]
    fn host_http_response_has_no_body_covers_upgrade_and_head_semantics() {
        assert!(host_http_response_has_no_body("GET", 101));
        assert!(host_http_response_has_no_body("HEAD", 200));
        assert!(host_http_response_has_no_body("GET", 204));
        assert!(host_http_response_has_no_body("GET", 304));
        assert!(!host_http_response_has_no_body("GET", 200));
    }
}
