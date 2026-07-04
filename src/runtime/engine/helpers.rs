use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng as _;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{Decision, Finding, FindingKind, FindingLocation, MemoryState};

pub(super) fn should_emit_heavy_artifacts(
    status: crate::ExitStatus,
    explicit_request: bool,
) -> bool {
    explicit_request
        || status != crate::ExitStatus::Pass
        || std::env::var("FOZZY_ARTIFACTS_FULL")
            .ok()
            .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub(super) fn rng_from_seed(seed: u64) -> ChaCha20Rng {
    let seed_bytes = blake3::hash(&seed.to_le_bytes()).as_bytes().to_owned();
    let mut seed32 = [0u8; 32];
    seed32.copy_from_slice(&seed_bytes[..32]);
    ChaCha20Rng::from_seed(seed32)
}

#[derive(Clone)]
pub(super) struct ExecCheckpoint {
    pub(super) rng: ChaCha20Rng,
    pub(super) clock: crate::VirtualClock,
    pub(super) kv: BTreeMap<String, String>,
    pub(super) fs: BTreeMap<String, String>,
    pub(super) fs_snapshots: BTreeMap<String, BTreeMap<String, String>>,
    pub(super) replay_host_fs: BTreeMap<String, Vec<u8>>,
    pub(super) replay_host_fs_snapshots: BTreeMap<String, BTreeMap<String, Option<Vec<u8>>>>,
    pub(super) host_fs_touched: BTreeSet<PathBuf>,
    pub(super) host_fs_snapshots: BTreeMap<String, BTreeMap<PathBuf, Option<Vec<u8>>>>,
    pub(super) http_rules: Vec<HttpRule>,
    pub(super) proc_rules: Vec<ProcRule>,
    pub(super) net_queue: VecDeque<NetMessage>,
    pub(super) net_inbox: BTreeMap<String, Vec<NetMessage>>,
    pub(super) net_partitions: BTreeSet<(String, String)>,
    pub(super) net_next_id: u64,
    pub(super) net_drop_rate: f64,
    pub(super) net_reorder: bool,
    pub(super) memory: MemoryState,
}

#[derive(Debug, Clone)]
pub(super) struct HttpRule {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) status: u16,
    pub(super) headers: BTreeMap<String, String>,
    pub(super) body: Option<String>,
    pub(super) json: Option<serde_json::Value>,
    pub(super) delay_ms: u64,
    pub(super) remaining: u64,
}

#[derive(Debug, Clone)]
pub(super) struct ProcRule {
    pub(super) cmd: String,
    pub(super) args: Vec<String>,
    pub(super) exit_code: i32,
    pub(super) stdout: String,
    pub(super) stderr: String,
    pub(super) remaining: u64,
}

pub(super) fn proc_rule(
    cmd: &str,
    args: &[String],
    exit_code: i32,
    stdout: String,
    stderr: String,
) -> ProcRule {
    ProcRule {
        cmd: cmd.to_string(),
        args: args.to_vec(),
        exit_code,
        stdout,
        stderr,
        remaining: 0,
    }
}

#[derive(Debug, Clone)]
pub(super) struct NetMessage {
    pub(super) id: u64,
    pub(super) from: String,
    pub(super) to: String,
    pub(super) payload: String,
}

pub(super) fn sorted_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ReplayCursor<'a> {
    decisions: &'a [Decision],
    index: usize,
}

impl<'a> ReplayCursor<'a> {
    pub(super) fn new(decisions: &'a [Decision]) -> Self {
        Self {
            decisions,
            index: 0,
        }
    }

    pub(super) fn next(&mut self) -> Option<&Decision> {
        let d = self.decisions.get(self.index);
        self.index = self.index.saturating_add(1);
        d
    }

    pub(super) fn peek(&self) -> Option<&Decision> {
        self.decisions.get(self.index)
    }

    pub(super) fn remaining(&self) -> usize {
        self.decisions.len().saturating_sub(self.index)
    }
}

pub(super) fn duration_to_ms(d: Duration) -> u64 {
    d.as_millis().min(u128::from(u64::MAX)) as u64
}

pub(super) fn measure_duration_ms<T, E, F>(f: F) -> Result<(T, u64), E>
where
    F: FnOnce() -> Result<T, E>,
{
    let started = Instant::now();
    let out = f()?;
    let duration_ms = crate::duration_fields(started.elapsed()).0;
    Ok((out, duration_ms))
}

pub(super) fn assert_proc_when_matches_host(
    cmd: &str,
    args: &[String],
    expected: &ProcRule,
    actual: &ProcRule,
    location: Option<FindingLocation>,
) -> Result<(), Finding> {
    let detail_payload = || {
        serde_json::json!({
            "requestKind": "process_assertion",
            "command": cmd,
            "args": args,
            "expected": {
                "exitCode": expected.exit_code,
                "stdoutPreview": truncate_event_text(&expected.stdout),
                "stderrPreview": truncate_event_text(&expected.stderr),
            },
            "actual": {
                "exitCode": actual.exit_code,
                "stdoutPreview": truncate_event_text(&actual.stdout),
                "stderrPreview": truncate_event_text(&actual.stderr),
            }
        })
    };
    let detail_location = || {
        location.clone().map(|mut location| {
            location.details = Some(detail_payload());
            location
        })
    };
    if actual.exit_code != expected.exit_code {
        return Err(Finding {
            kind: FindingKind::Assertion,
            title: "proc_when_host_exit".to_string(),
            message: format!(
                "host proc exit mismatch for {cmd:?} {:?}: expected {}, got {}",
                args, expected.exit_code, actual.exit_code
            ),
            location: detail_location(),
        });
    }
    if actual.stdout != expected.stdout {
        return Err(Finding {
            kind: FindingKind::Assertion,
            title: "proc_when_host_stdout".to_string(),
            message: format!("host proc stdout mismatch for {cmd:?} {:?}", args),
            location: detail_location(),
        });
    }
    if actual.stderr != expected.stderr {
        return Err(Finding {
            kind: FindingKind::Assertion,
            title: "proc_when_host_stderr".to_string(),
            message: format!("host proc stderr mismatch for {cmd:?} {:?}", args),
            location: detail_location(),
        });
    }
    Ok(())
}

pub(super) fn encode_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for b in bytes {
        out.push(TABLE[(b >> 4) as usize] as char);
        out.push(TABLE[(b & 0x0F) as usize] as char);
    }
    out
}

pub(super) fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err("hex payload must contain an even number of characters".to_string());
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let hi = decode_hex_nibble(bytes[i])
            .ok_or_else(|| format!("invalid hex character {:?}", bytes[i] as char))?;
        let lo = decode_hex_nibble(bytes[i + 1])
            .ok_or_else(|| format!("invalid hex character {:?}", bytes[i + 1] as char))?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

const TRACE_EVENT_TEXT_LIMIT_BYTES: usize = 16 * 1024;

pub(super) fn proc_unmatched_message(
    cmd: &str,
    _args: &[String],
    scenario_path: Option<&Path>,
    step_index: usize,
) -> String {
    let scenario_hint = scenario_path
        .map(|path| format!(" in {}", path.display()))
        .unwrap_or_default();
    format!(
        "Strict proc backend blocked an undeclared subprocess for `cmd={cmd}` before step #{} (`proc_spawn`){scenario_hint}. Add a matching `proc_when` step or opt into host proc execution intentionally.",
        step_index + 1
    )
}

pub(crate) fn proc_unmatched_hint() -> String {
    "Add a matching `proc_when` step with the emitted scaffold details, or remove strict proc preflight if unrestricted host subprocess execution is intentional.".to_string()
}

pub(super) fn proc_unmatched_details(
    cmd: &str,
    args: &[String],
    scenario_path: Option<&Path>,
    step_index: usize,
) -> serde_json::Value {
    serde_json::json!({
        "command": cmd,
        "args": args,
        "scenarioPath": scenario_path.map(|path| path.display().to_string()),
        "stepIndex": step_index,
        "suggestedProcWhen": {
            "type": "proc_when",
            "cmd": cmd,
            "args": args,
            "exit_code": 0,
            "stdout": "",
            "stderr": "",
            "times": 1
        }
    })
}

pub(super) fn truncate_event_text(text: &str) -> String {
    if text.len() <= TRACE_EVENT_TEXT_LIMIT_BYTES {
        return text.to_string();
    }
    let mut out = text
        .chars()
        .take(TRACE_EVENT_TEXT_LIMIT_BYTES)
        .collect::<String>();
    out.push_str("...[truncated]");
    out
}
