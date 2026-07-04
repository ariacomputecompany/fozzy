use rand_chacha::ChaCha20Rng;

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{
    Decision, DecisionLog, ExitStatus, Finding, FindingKind, FindingLocation, FozzyError,
    FozzyResult, MemoryOptions, MemoryState, ScenarioV1Steps, TraceEvent,
};

use super::helpers::{HttpRule, NetMessage, ProcRule, ReplayCursor, rng_from_seed};
use super::types::{FsBackend, HttpBackend, ProcBackend, ScenarioRun};

#[path = "exec/basic.rs"]
mod basic;
#[path = "exec/file_http.rs"]
mod file_http;
#[path = "exec/fs.rs"]
mod fs;
#[path = "exec/memory.rs"]
mod memory;
#[path = "exec/proc_net.rs"]
mod proc_net;

pub(crate) struct ExecCtx<'a> {
    pub(super) det: bool,
    pub(super) proc_backend: ProcBackend,
    pub(super) fs_backend: FsBackend,
    pub(super) http_backend: HttpBackend,
    pub(super) host_deadline: Option<Instant>,
    pub(super) host_root: PathBuf,
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
    pub(super) decisions: DecisionLog,
    pub(super) events: Vec<TraceEvent>,
    pub(super) findings: Vec<Finding>,
    pub(super) executed_steps: usize,
    pub(super) replay: Option<ReplayCursor<'a>>,
    pub(super) current_step_index: Option<usize>,
    pub(super) scenario_path: Option<PathBuf>,
}

impl<'a> ExecCtx<'a> {
    pub(super) fn new(
        seed: u64,
        det: bool,
        host_deadline: Option<Instant>,
        proc_backend: ProcBackend,
        fs_backend: FsBackend,
        http_backend: HttpBackend,
        memory: MemoryOptions,
    ) -> Self {
        let rng = rng_from_seed(seed);
        Self {
            det,
            proc_backend,
            fs_backend,
            http_backend,
            host_deadline,
            host_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            rng,
            clock: crate::VirtualClock::default(),
            kv: BTreeMap::new(),
            fs: BTreeMap::new(),
            fs_snapshots: BTreeMap::new(),
            replay_host_fs: BTreeMap::new(),
            replay_host_fs_snapshots: BTreeMap::new(),
            host_fs_touched: BTreeSet::new(),
            host_fs_snapshots: BTreeMap::new(),
            http_rules: Vec::new(),
            proc_rules: Vec::new(),
            net_queue: VecDeque::new(),
            net_inbox: BTreeMap::new(),
            net_partitions: BTreeSet::new(),
            net_next_id: 1,
            net_drop_rate: 0.0,
            net_reorder: false,
            memory: MemoryState::new(memory),
            decisions: DecisionLog::default(),
            events: Vec::new(),
            findings: Vec::new(),
            executed_steps: 0,
            replay: None,
            current_step_index: None,
            scenario_path: None,
        }
    }

    pub(super) fn set_active_step(&mut self, scenario_path: &Path, step_index: usize) {
        self.current_step_index = Some(step_index);
        self.scenario_path = Some(scenario_path.to_path_buf());
    }

    pub(super) fn current_finding_location(&self) -> Option<FindingLocation> {
        self.scenario_path.as_ref().map(|path| FindingLocation {
            file: Some(path.display().to_string()),
            line: None,
            col: None,
            details: None,
        })
    }

    pub(super) fn current_memory_callsite(
        &self,
        op: &str,
        key: Option<&String>,
        tag: Option<&String>,
    ) -> String {
        let mut parts = vec![op.to_string()];
        if let Some(path) = self.scenario_path.as_ref() {
            parts.push(format!("path={}", path.display()));
        }
        if let Some(step_index) = self.current_step_index {
            parts.push(format!("step={step_index}"));
        }
        if let Some(key) = key {
            parts.push(format!("key={key}"));
        }
        if let Some(tag) = tag {
            parts.push(format!("tag={tag}"));
        }
        parts.join("|")
    }

    pub(super) fn remaining_host_timeout(&self) -> Option<Duration> {
        let deadline = self.host_deadline?;
        Some(deadline.saturating_duration_since(Instant::now()))
    }

    pub(super) fn finish(
        mut self,
        mut status: ExitStatus,
        scenario_path: PathBuf,
        embedded: ScenarioV1Steps,
        started_at: String,
        elapsed: Duration,
    ) -> ScenarioRun {
        let mut memory_report = None;
        if self.memory.options.tracking_requested() || self.memory.has_activity() {
            let report = self.memory.finalize();
            if report.summary.leaked_bytes > 0
                && (report.options.fail_on_leak || report.options.leak_budget_bytes.is_none())
            {
                self.findings.push(Finding {
                    kind: FindingKind::Checker,
                    title: "memory_leak".to_string(),
                    message: format!(
                        "detected {} leaked allocation(s), leaked_bytes={}",
                        report.summary.leaked_allocs, report.summary.leaked_bytes
                    ),
                    location: None,
                });
            }
            if let Some(budget) = report.options.leak_budget_bytes
                && report.summary.leaked_bytes > budget
            {
                self.findings.push(Finding {
                    kind: FindingKind::Checker,
                    title: "memory_leak_budget".to_string(),
                    message: format!(
                        "leak budget exceeded: leaked_bytes={} budget_bytes={}",
                        report.summary.leaked_bytes, budget
                    ),
                    location: None,
                });
                if status == ExitStatus::Pass {
                    status = ExitStatus::Fail;
                }
            }
            if !report.leak_allowed_by_policy() && status == ExitStatus::Pass {
                status = ExitStatus::Fail;
            }
            memory_report = Some(report);
        }
        let finished_at = crate::wall_time_iso_utc();
        let (duration_ms, duration_ns) = crate::duration_fields(elapsed);
        ScenarioRun {
            status,
            findings: self.findings,
            memory: memory_report,
            decisions: self.decisions,
            events: self.events,
            scenario_path,
            scenario_embedded: embedded,
            started_at,
            finished_at,
            duration_ms,
            duration_ns,
        }
    }

    pub(super) fn advance_recorded_time(&mut self, duration_ms: u64) {
        if duration_ms > 0 {
            self.clock.advance(Duration::from_millis(duration_ms));
        }
    }

    pub(super) fn expect_step(&mut self, idx: usize) -> FozzyResult<()> {
        let Some(cursor) = self.replay.as_mut() else {
            return Ok(());
        };
        match cursor.next() {
            Some(Decision::Step { index, .. }) if *index == idx => Ok(()),
            Some(other) => Err(FozzyError::Trace(format!(
                "replay drift at step {idx}: expected step decision, got {other:?}"
            ))),
            None => Err(FozzyError::Trace(format!(
                "replay drift at step {idx}: missing decision"
            ))),
        }
    }

    pub(super) fn expect_scheduler_pick(&mut self, task_id: u64, _label: &str) -> FozzyResult<()> {
        let Some(cursor) = self.replay.as_mut() else {
            return Ok(());
        };
        match cursor.next() {
            Some(Decision::SchedulerPick {
                task_id: expected_id,
                ..
            }) if *expected_id == task_id => Ok(()),
            Some(other) => Err(FozzyError::Trace(format!(
                "replay drift: expected SchedulerPick(task_id={task_id}), got {other:?}"
            ))),
            None => Err(FozzyError::Trace(
                "replay drift: missing SchedulerPick decision".to_string(),
            )),
        }
    }

    pub(super) fn replay_peek(&self) -> Option<&Decision> {
        self.replay.as_ref().and_then(|c| c.peek())
    }

    pub(super) fn replay_take_if<F>(&mut self, pred: F) -> Option<Decision>
    where
        F: FnOnce(&Decision) -> bool,
    {
        let cursor = self.replay.as_mut()?;
        let next = cursor.peek()?;
        if pred(next) {
            cursor.next().cloned()
        } else {
            None
        }
    }

    pub(super) fn exec_step(&mut self, step: &crate::Step) -> Result<(), Finding> {
        if self.exec_basic_step(step)? {
            return Ok(());
        }
        if self.exec_file_http_step(step)? {
            return Ok(());
        }
        if self.exec_proc_net_step(step)? {
            return Ok(());
        }
        self.exec_memory_step(step).map(|_| ())
    }

    pub(super) fn mark_step_executed(&mut self, step: &crate::Step) {
        if !step_is_declaration_only(step) {
            self.executed_steps = self.executed_steps.saturating_add(1);
        }
    }
}

pub(crate) fn step_is_declaration_only(step: &crate::Step) -> bool {
    matches!(
        step,
        crate::Step::HttpWhen { .. } | crate::Step::ProcWhen { .. }
    )
}
