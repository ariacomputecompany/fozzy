use std::time::Duration;

use crate::{Decision, DecisionLog, Finding, FindingKind, TraceEvent};

use super::ExecCtx;
use super::super::helpers::{ExecCheckpoint, duration_to_ms};

impl ExecCtx<'_> {
    pub(super) fn exec_memory_step(&mut self, step: &crate::Step) -> Result<bool, Finding> {
        match step {

            crate::Step::MemoryAlloc { bytes, key, tag } => {
                let callsite =
                    self.current_memory_callsite("memory_alloc", key.as_ref(), tag.as_ref());
                let outcome =
                    self.memory
                        .allocate(*bytes, tag.clone(), &callsite, self.clock.now_ms());
                self.decisions.push(Decision::MemoryAlloc {
                    bytes: *bytes,
                    effective_bytes: outcome.effective_bytes,
                    alloc_id: outcome.alloc_id,
                    callsite_hash: outcome.callsite_hash.clone(),
                    failed_reason: outcome.failed_reason.clone(),
                });
                if let Some(cur) = self.replay.as_mut() {
                    match cur.next() {
                        Some(Decision::MemoryAlloc {
                            bytes: expected_bytes,
                            effective_bytes: expected_effective_bytes,
                            alloc_id: expected_alloc_id,
                            failed_reason: expected_failed,
                            ..
                        }) if *expected_bytes == *bytes
                            && *expected_effective_bytes == outcome.effective_bytes
                            && *expected_alloc_id == outcome.alloc_id
                            && *expected_failed == outcome.failed_reason => {}
                        Some(other) => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!("expected MemoryAlloc({bytes}), got {other:?}"),
                                location: None,
                            });
                        }
                        None => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: "missing MemoryAlloc decision".to_string(),
                                location: None,
                            });
                        }
                    }
                }
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "memory_alloc".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("bytes".to_string(), serde_json::json!(bytes)),
                        (
                            "effective_bytes".to_string(),
                            serde_json::json!(outcome.effective_bytes),
                        ),
                        ("alloc_id".to_string(), serde_json::json!(outcome.alloc_id)),
                        (
                            "failed_reason".to_string(),
                            serde_json::json!(outcome.failed_reason.clone()),
                        ),
                        (
                            "callsite_hash".to_string(),
                            serde_json::json!(outcome.callsite_hash.clone()),
                        ),
                    ]),
                });
                if let Some(id) = outcome.alloc_id
                    && let Some(k) = key
                {
                    self.kv.insert(k.clone(), id.to_string());
                }
                if let Some(reason) = outcome.failed_reason {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "memory_alloc_failed".to_string(),
                        message: format!("memory allocation failed: {reason}"),
                        location: None,
                    });
                }
                Ok(true)
            }

            crate::Step::MemoryFree { alloc_id, key } => {
                let id = if let Some(v) = alloc_id {
                    *v
                } else if let Some(k) = key {
                    let Some(raw) = self.kv.get(k) else {
                        return Err(Finding {
                            kind: FindingKind::Checker,
                            title: "memory_free".to_string(),
                            message: format!("missing alloc id key {k:?}"),
                            location: None,
                        });
                    };
                    raw.parse::<u64>().map_err(|_| Finding {
                        kind: FindingKind::Checker,
                        title: "memory_free".to_string(),
                        message: format!("alloc id key {k:?} is not a u64"),
                        location: None,
                    })?
                } else {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "memory_free".to_string(),
                        message: "set alloc_id or key".to_string(),
                        location: None,
                    });
                };
                let existed = self.memory.free(id, self.clock.now_ms());
                self.decisions.push(Decision::MemoryFree {
                    alloc_id: id,
                    existed,
                });
                if let Some(cur) = self.replay.as_mut() {
                    match cur.next() {
                        Some(Decision::MemoryFree {
                            alloc_id: expected_id,
                            existed: expected_existed,
                        }) if *expected_id == id && *expected_existed == existed => {}
                        Some(other) => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!("expected MemoryFree({id}), got {other:?}"),
                                location: None,
                            });
                        }
                        None => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: "missing MemoryFree decision".to_string(),
                                location: None,
                            });
                        }
                    }
                }
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "memory_free".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("alloc_id".to_string(), serde_json::json!(id)),
                        ("existed".to_string(), serde_json::json!(existed)),
                    ]),
                });
                if !existed {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "memory_free_missing".to_string(),
                        message: format!("allocation id {id} was not live"),
                        location: None,
                    });
                }
                Ok(true)
            }

            crate::Step::MemoryLimitMb { mb } => {
                self.memory.set_limit_mb(*mb);
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "memory_limit_mb".to_string(),
                    fields: serde_json::Map::from_iter([("mb".to_string(), serde_json::json!(mb))]),
                });
                Ok(true)
            }

            crate::Step::MemoryFailAfterAllocs { count } => {
                self.memory.set_fail_after_allocs(*count);
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "memory_fail_after_allocs".to_string(),
                    fields: serde_json::Map::from_iter([(
                        "count".to_string(),
                        serde_json::json!(count),
                    )]),
                });
                Ok(true)
            }

            crate::Step::MemoryFragmentation { seed } => {
                self.memory.set_fragmentation_seed(*seed);
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "memory_fragmentation".to_string(),
                    fields: serde_json::Map::from_iter([(
                        "seed".to_string(),
                        serde_json::json!(seed),
                    )]),
                });
                Ok(true)
            }

            crate::Step::MemoryPressureWave { pattern } => {
                self.memory.set_pressure_wave(pattern.clone());
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "memory_pressure_wave".to_string(),
                    fields: serde_json::Map::from_iter([(
                        "pattern".to_string(),
                        serde_json::json!(pattern),
                    )]),
                });
                Ok(true)
            }

            crate::Step::MemoryCheckpoint { name } => {
                self.memory.checkpoint(name, self.clock.now_ms());
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "memory_checkpoint".to_string(),
                    fields: serde_json::Map::from_iter([(
                        "name".to_string(),
                        serde_json::json!(name),
                    )]),
                });
                Ok(true)
            }

            crate::Step::MemoryAssertInUseBytes { equals } => {
                let got = self.memory.in_use_bytes();
                if got != *equals {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "memory_assert_in_use_bytes".to_string(),
                        message: format!("expected in_use_bytes={equals}, got {got}"),
                        location: None,
                    });
                }
                Ok(true)
            }

            crate::Step::AssertThrows { steps } => {
                self.exec_expect_failure("assert_throws", steps)?;
                Ok(true)
            }
            crate::Step::AssertRejects { steps } => {
                self.exec_expect_failure("assert_rejects", steps)?;
                Ok(true)
            }

            crate::Step::AssertEventuallyKv {
                key,
                equals,
                within,
                poll,
                msg,
            } => {
                let within_d = crate::parse_duration(within).map_err(|e| Finding {
                    kind: FindingKind::Checker,
                    title: "invalid_duration".to_string(),
                    message: e.to_string(),
                    location: None,
                })?;
                let poll_d = crate::parse_duration(poll).map_err(|e| Finding {
                    kind: FindingKind::Checker,
                    title: "invalid_duration".to_string(),
                    message: e.to_string(),
                    location: None,
                })?;

                let start_virtual_ms = self.clock.now_ms();
                let deadline = start_virtual_ms.saturating_add(duration_to_ms(within_d));
                loop {
                    if self.kv.get(key).is_some_and(|v| v == equals) {
                        return Ok(true);
                    }
                    if self.clock.now_ms() >= deadline {
                        return Err(Finding {
                            kind: FindingKind::Assertion,
                            title: "assert_eventually_kv".to_string(),
                            message: msg.clone().unwrap_or_else(|| {
                                format!("key {key:?} did not become {equals:?} within {}", within)
                            }),
                            location: None,
                        });
                    }
                    self.sleep_poll(poll_d);
                }
            }

            crate::Step::AssertNeverKv {
                key,
                equals,
                within,
                poll,
                msg,
            } => {
                let within_d = crate::parse_duration(within).map_err(|e| Finding {
                    kind: FindingKind::Checker,
                    title: "invalid_duration".to_string(),
                    message: e.to_string(),
                    location: None,
                })?;
                let poll_d = crate::parse_duration(poll).map_err(|e| Finding {
                    kind: FindingKind::Checker,
                    title: "invalid_duration".to_string(),
                    message: e.to_string(),
                    location: None,
                })?;

                let start_virtual_ms = self.clock.now_ms();
                let deadline = start_virtual_ms.saturating_add(duration_to_ms(within_d));
                loop {
                    if self.kv.get(key).is_some_and(|v| v == equals) {
                        return Err(Finding {
                            kind: FindingKind::Assertion,
                            title: "assert_never_kv".to_string(),
                            message: msg.clone().unwrap_or_else(|| {
                                format!("key {key:?} became forbidden value {equals:?}")
                            }),
                            location: None,
                        });
                    }
                    if self.clock.now_ms() >= deadline {
                        return Ok(true);
                    }
                    self.sleep_poll(poll_d);
                }
            }

            crate::Step::Fail { message } => Err(Finding {
                kind: FindingKind::Assertion,
                title: "fail".to_string(),
                message: message.clone(),
                location: None,
            }),

            crate::Step::Panic { message } => Err(Finding {
                kind: FindingKind::Panic,
                title: "panic".to_string(),
                message: message.clone(),
                location: None,
            }),
            _ => Ok(false),
        }
    }

    fn exec_expect_failure(&mut self, title: &str, steps: &[crate::Step]) -> Result<(), Finding> {
        let replay = self.replay;
        let checkpoint = self.checkpoint();
        self.replay = None;
        self.decisions = DecisionLog::default();
        self.events.clear();
        self.findings.clear();
        for s in steps {
            if self.exec_step(s).is_err() {
                self.restore(checkpoint);
                self.replay = replay;
                return Ok(());
            }
        }
        self.restore(checkpoint);
        self.replay = replay;

        Err(Finding {
            kind: FindingKind::Assertion,
            title: title.to_string(),
            message: format!("{title} expected failure but nested steps passed"),
            location: None,
        })
    }

    fn sleep_poll(&mut self, d: Duration) {
        if self.det {
            self.clock.advance(d);
        } else {
            std::thread::sleep(d);
        }
    }

    fn checkpoint(&self) -> ExecCheckpoint {
        ExecCheckpoint {
            rng: self.rng.clone(),
            clock: self.clock.clone(),
            kv: self.kv.clone(),
            fs: self.fs.clone(),
            fs_snapshots: self.fs_snapshots.clone(),
            replay_host_fs: self.replay_host_fs.clone(),
            replay_host_fs_snapshots: self.replay_host_fs_snapshots.clone(),
            host_fs_touched: self.host_fs_touched.clone(),
            host_fs_snapshots: self.host_fs_snapshots.clone(),
            http_rules: self.http_rules.clone(),
            proc_rules: self.proc_rules.clone(),
            net_queue: self.net_queue.clone(),
            net_inbox: self.net_inbox.clone(),
            net_partitions: self.net_partitions.clone(),
            net_next_id: self.net_next_id,
            net_drop_rate: self.net_drop_rate,
            net_reorder: self.net_reorder,
            memory: self.memory.clone(),
        }
    }

    fn restore(&mut self, checkpoint: ExecCheckpoint) {
        self.rng = checkpoint.rng;
        self.clock = checkpoint.clock;
        self.kv = checkpoint.kv;
        self.fs = checkpoint.fs;
        self.fs_snapshots = checkpoint.fs_snapshots;
        self.replay_host_fs = checkpoint.replay_host_fs;
        self.replay_host_fs_snapshots = checkpoint.replay_host_fs_snapshots;
        self.host_fs_touched = checkpoint.host_fs_touched;
        self.host_fs_snapshots = checkpoint.host_fs_snapshots;
        self.http_rules = checkpoint.http_rules;
        self.proc_rules = checkpoint.proc_rules;
        self.net_queue = checkpoint.net_queue;
        self.net_inbox = checkpoint.net_inbox;
        self.net_partitions = checkpoint.net_partitions;
        self.net_next_id = checkpoint.net_next_id;
        self.net_drop_rate = checkpoint.net_drop_rate;
        self.net_reorder = checkpoint.net_reorder;
        self.memory = checkpoint.memory;
    }
}
