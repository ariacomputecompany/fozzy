use rand_core::RngCore as _;

use crate::host::{HostProcDispatch, dispatch_host_proc};
use crate::{Decision, Finding, FindingKind, TraceEvent};

use super::super::helpers::{
    NetMessage, ProcRule, assert_proc_when_matches_host, measure_duration_ms, proc_rule,
    proc_unmatched_details, proc_unmatched_message, sorted_pair, truncate_event_text,
};
use super::super::types::ProcBackend;
use super::ExecCtx;

fn proc_invocation_details(
    cmd: &str,
    args: &[String],
    stdout: Option<&str>,
    stderr: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "requestKind": "process_spawn",
        "command": cmd,
        "args": args,
        "stdoutPreview": stdout.map(|value| truncate_event_text(value)),
        "stderrPreview": stderr.map(|value| truncate_event_text(value)),
    })
}

impl ExecCtx<'_> {
    pub(super) fn exec_proc_net_step(&mut self, step: &crate::Step) -> Result<bool, Finding> {
        match step {
            crate::Step::ProcWhen {
                cmd,
                args,
                exit_code,
                stdout,
                stderr,
                times,
            } => {
                self.proc_rules.push(ProcRule {
                    cmd: cmd.clone(),
                    args: args.clone().unwrap_or_default(),
                    exit_code: *exit_code,
                    stdout: stdout.clone().unwrap_or_default(),
                    stderr: stderr.clone().unwrap_or_default(),
                    remaining: times.unwrap_or(u64::MAX),
                });
                Ok(true)
            }

            crate::Step::ProcSpawn {
                cmd,
                args,
                expect_exit,
                expect_stdout,
                expect_stderr,
                save_stdout_as,
            } => {
                let start_ms = self.clock.now_ms();
                let call_args = args.clone().unwrap_or_default();
                let mut observed_peak_rss_bytes = 0u64;
                let mut observed_rss_sample_count = 0u64;
                let replay_rule = match self.replay_peek().cloned() {
                    Some(Decision::ProcSpawn {
                        cmd: replay_cmd,
                        args: replay_args,
                        exit_code,
                        stdout,
                        stderr,
                        peak_rss_bytes,
                        rss_sample_count,
                        duration_ms,
                    }) => {
                        if replay_cmd != *cmd || replay_args != call_args {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!(
                                    "replay proc drift: expected {replay_cmd:?} {:?}, got {cmd:?} {:?}",
                                    replay_args, call_args
                                ),
                                location: None,
                            });
                        }
                        let _ = self.replay_take_if(|d| matches!(d, Decision::ProcSpawn { .. }));
                        self.advance_recorded_time(duration_ms);
                        observed_peak_rss_bytes = peak_rss_bytes;
                        observed_rss_sample_count = rss_sample_count;
                        self.memory.record_host_proc_peak(
                            cmd,
                            &call_args,
                            peak_rss_bytes,
                            rss_sample_count,
                            self.clock.now_ms(),
                        );
                        Some((
                            proc_rule(cmd, &call_args, exit_code, stdout, stderr),
                            "replay",
                        ))
                    }
                    Some(Decision::ProcSpawnTimeout {
                        cmd: replay_cmd,
                        args: replay_args,
                        peak_rss_bytes,
                        rss_sample_count,
                        duration_ms,
                        ..
                    }) => {
                        if replay_cmd != *cmd || replay_args != call_args {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!(
                                    "replay proc timeout drift: expected {replay_cmd:?} {:?}, got {cmd:?} {:?}",
                                    replay_args, call_args
                                ),
                                location: None,
                            });
                        }
                        let _ =
                            self.replay_take_if(|d| matches!(d, Decision::ProcSpawnTimeout { .. }));
                        self.advance_recorded_time(duration_ms);
                        self.memory.record_host_proc_peak(
                            cmd,
                            &call_args,
                            peak_rss_bytes,
                            rss_sample_count,
                            self.clock.now_ms(),
                        );
                        return Err(Finding {
                            kind: FindingKind::Hang,
                            title: "timeout".to_string(),
                            message: format!("host proc timed out for {cmd:?} {:?}", call_args),
                            location: self.current_finding_location().map(|mut location| {
                                location.details =
                                    Some(proc_invocation_details(cmd, &call_args, None, None));
                                location
                            }),
                        });
                    }
                    Some(_) | None => None,
                };

                let host_rule_idx = self
                    .proc_rules
                    .iter()
                    .position(|r| r.remaining > 0 && r.cmd == *cmd && r.args == call_args);
                let (rule, backend, completion_boundary) = if let Some((rule, backend)) =
                    replay_rule
                {
                    (rule, backend, "recorded_decision")
                } else if matches!(self.proc_backend, ProcBackend::Host) {
                    if !self.proc_rules.is_empty() && host_rule_idx.is_none() {
                        return Err(Finding {
                            kind: FindingKind::Assertion,
                            title: "proc_when_host_unmatched".to_string(),
                            message: format!(
                                "no proc_when matched host proc {cmd:?} {:?}. remediation: \
                                 1) align proc_when.cmd/args with this invocation, \
                                 2) remove proc_when if you want unrestricted host proc execution, \
                                 3) run with --proc-backend scripted to use mocked responses.",
                                call_args
                            ),
                            location: self.current_finding_location().map(|mut location| {
                                location.details =
                                    Some(proc_invocation_details(cmd, &call_args, None, None));
                                location
                            }),
                        });
                    }
                    let (dispatch, duration_ms) = measure_duration_ms(|| {
                        dispatch_host_proc(cmd, &call_args, self.host_deadline).map_err(|message| {
                            Finding {
                                kind: FindingKind::Assertion,
                                title: "proc_spawn_host".to_string(),
                                message,
                                location: None,
                            }
                        })
                    })?;
                    match dispatch {
                        HostProcDispatch::Completed(output) => {
                            let rule = proc_rule(
                                cmd,
                                &call_args,
                                output.exit_code,
                                output.stdout,
                                output.stderr,
                            );
                            if let Some(idx) = host_rule_idx {
                                let mut expected = self.proc_rules[idx].clone();
                                if expected.remaining != u64::MAX {
                                    expected.remaining = expected.remaining.saturating_sub(1);
                                }
                                self.proc_rules[idx] = expected.clone();
                                assert_proc_when_matches_host(
                                    cmd,
                                    &call_args,
                                    &expected,
                                    &rule,
                                    self.current_finding_location(),
                                )?;
                            }
                            observed_peak_rss_bytes = output.peak_rss_bytes;
                            observed_rss_sample_count = output.rss_sample_count;
                            self.memory.record_host_proc_peak(
                                cmd,
                                &call_args,
                                observed_peak_rss_bytes,
                                observed_rss_sample_count,
                                self.clock.now_ms(),
                            );
                            self.decisions.push(Decision::ProcSpawn {
                                cmd: cmd.clone(),
                                args: call_args.clone(),
                                exit_code: rule.exit_code,
                                stdout: rule.stdout.clone(),
                                stderr: rule.stderr.clone(),
                                peak_rss_bytes: observed_peak_rss_bytes,
                                rss_sample_count: observed_rss_sample_count,
                                duration_ms,
                            });
                            self.advance_recorded_time(duration_ms);
                            (rule, "host", "process_exit")
                        }
                        HostProcDispatch::TimedOut {
                            stdout,
                            stderr,
                            peak_rss_bytes,
                            rss_sample_count,
                        } => {
                            observed_peak_rss_bytes = peak_rss_bytes;
                            observed_rss_sample_count = rss_sample_count;
                            self.memory.record_host_proc_peak(
                                cmd,
                                &call_args,
                                observed_peak_rss_bytes,
                                observed_rss_sample_count,
                                self.clock.now_ms(),
                            );
                            let details = proc_invocation_details(
                                cmd,
                                &call_args,
                                Some(&stdout),
                                Some(&stderr),
                            );
                            self.decisions.push(Decision::ProcSpawnTimeout {
                                cmd: cmd.clone(),
                                args: call_args.clone(),
                                stdout,
                                stderr,
                                peak_rss_bytes: observed_peak_rss_bytes,
                                rss_sample_count: observed_rss_sample_count,
                                duration_ms,
                            });
                            self.advance_recorded_time(duration_ms);
                            return Err(Finding {
                                kind: FindingKind::Hang,
                                title: "timeout".to_string(),
                                message: format!(
                                    "host proc timed out for {cmd:?} {:?}; no terminal process-exit boundary was observed before timeout",
                                    call_args
                                ),
                                location: self.current_finding_location().map(|mut location| {
                                    location.details = Some(details);
                                    location
                                }),
                            });
                        }
                    }
                } else if let Some(idx) = host_rule_idx {
                    let mut rule = self.proc_rules[idx].clone();
                    if rule.remaining != u64::MAX {
                        rule.remaining = rule.remaining.saturating_sub(1);
                    }
                    self.proc_rules[idx] = rule.clone();
                    self.decisions.push(Decision::ProcSpawn {
                        cmd: cmd.clone(),
                        args: call_args.clone(),
                        exit_code: rule.exit_code,
                        stdout: rule.stdout.clone(),
                        stderr: rule.stderr.clone(),
                        peak_rss_bytes: 0,
                        rss_sample_count: 0,
                        duration_ms: 0,
                    });
                    (rule, "scripted", "process_exit")
                } else {
                    let step_index = self.current_step_index.unwrap_or_default();
                    let mut location = self.current_finding_location();
                    if let Some(location) = location.as_mut() {
                        location.details = Some(proc_unmatched_details(
                            cmd,
                            &call_args,
                            self.scenario_path.as_deref(),
                            step_index,
                        ));
                    }
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "proc_unmatched".to_string(),
                        message: proc_unmatched_message(
                            cmd,
                            &call_args,
                            self.scenario_path.as_deref(),
                            step_index,
                        ),
                        location,
                    });
                };

                let backend = backend.to_string();
                let mut proc_fields = serde_json::Map::new();
                proc_fields.insert("cmd".to_string(), serde_json::Value::String(cmd.clone()));
                proc_fields.insert(
                    "backend".to_string(),
                    serde_json::Value::String(backend.clone()),
                );
                proc_fields.insert(
                    "request_kind".to_string(),
                    serde_json::Value::String("process_spawn".to_string()),
                );
                proc_fields.insert(
                    "completion_boundary".to_string(),
                    serde_json::Value::String(completion_boundary.to_string()),
                );
                proc_fields.insert(
                    "exit_code".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(rule.exit_code)),
                );
                proc_fields.insert(
                    "stdout".to_string(),
                    serde_json::Value::String(truncate_event_text(&rule.stdout)),
                );
                proc_fields.insert(
                    "stderr".to_string(),
                    serde_json::Value::String(truncate_event_text(&rule.stderr)),
                );
                proc_fields.insert(
                    "stdout_bytes".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(rule.stdout.len() as u64)),
                );
                proc_fields.insert(
                    "stderr_bytes".to_string(),
                    serde_json::Value::Number(serde_json::Number::from(rule.stderr.len() as u64)),
                );
                if observed_peak_rss_bytes > 0 {
                    proc_fields.insert(
                        "peak_rss_bytes".to_string(),
                        serde_json::json!(observed_peak_rss_bytes),
                    );
                    proc_fields.insert(
                        "rss_sample_count".to_string(),
                        serde_json::json!(observed_rss_sample_count),
                    );
                }
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "proc_spawn".to_string(),
                    fields: proc_fields,
                });
                let mut capability_fields = serde_json::Map::from_iter([
                    ("op".to_string(), serde_json::json!("spawn")),
                    ("cmd".to_string(), serde_json::json!(cmd)),
                    (
                        "payload_bytes".to_string(),
                        serde_json::json!((rule.stdout.len() + rule.stderr.len()) as u64),
                    ),
                    (
                        "duration_ms".to_string(),
                        serde_json::json!(self.clock.now_ms().saturating_sub(start_ms)),
                    ),
                ]);
                if observed_peak_rss_bytes > 0 {
                    capability_fields.insert(
                        "peak_rss_bytes".to_string(),
                        serde_json::json!(observed_peak_rss_bytes),
                    );
                    capability_fields.insert(
                        "rss_sample_count".to_string(),
                        serde_json::json!(observed_rss_sample_count),
                    );
                }
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "capability_proc".to_string(),
                    fields: capability_fields,
                });

                if let Some(expected) = expect_exit
                    && rule.exit_code != *expected
                {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "proc_exit".to_string(),
                        message: format!("expected exit {expected}, got {}", rule.exit_code),
                        location: None,
                    });
                }
                if let Some(expected) = expect_stdout
                    && &rule.stdout != expected
                {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "proc_stdout".to_string(),
                        message: "proc stdout mismatch".to_string(),
                        location: None,
                    });
                }
                if let Some(expected) = expect_stderr
                    && &rule.stderr != expected
                {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "proc_stderr".to_string(),
                        message: "proc stderr mismatch".to_string(),
                        location: None,
                    });
                }
                if let Some(key) = save_stdout_as {
                    self.kv.insert(key.clone(), rule.stdout.clone());
                }

                Ok(true)
            }

            crate::Step::NetPartition { a, b } => {
                self.net_partitions.insert(sorted_pair(a, b));
                Ok(true)
            }

            crate::Step::NetHeal { a, b } => {
                self.net_partitions.remove(&sorted_pair(a, b));
                Ok(true)
            }

            crate::Step::NetSetDropRate { rate } => {
                if !(0.0..=1.0).contains(rate) {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "net_drop_rate".to_string(),
                        message: format!("invalid drop rate {rate}; expected [0,1]"),
                        location: None,
                    });
                }
                self.net_drop_rate = *rate;
                Ok(true)
            }

            crate::Step::NetSetReorder { enabled } => {
                self.net_reorder = *enabled;
                Ok(true)
            }

            crate::Step::NetSend { from, to, payload } => {
                let id = self.net_next_id;
                self.net_next_id = self.net_next_id.saturating_add(1);
                self.net_queue.push_back(NetMessage {
                    id,
                    from: from.clone(),
                    to: to.clone(),
                    payload: payload.clone(),
                });
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "net_send".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("id".to_string(), serde_json::json!(id)),
                        ("from".to_string(), serde_json::json!(from)),
                        ("to".to_string(), serde_json::json!(to)),
                        (
                            "payload_size".to_string(),
                            serde_json::json!(payload.len() as u64),
                        ),
                    ]),
                });
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "capability_net".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("op".to_string(), serde_json::json!("send")),
                        (
                            "payload_bytes".to_string(),
                            serde_json::json!(payload.len() as u64),
                        ),
                        ("duration_ms".to_string(), serde_json::json!(0u64)),
                    ]),
                });
                Ok(true)
            }

            crate::Step::NetDeliverOne { strategy } => {
                let mut deliverable = Vec::new();
                for (idx, msg) in self.net_queue.iter().enumerate() {
                    if self
                        .net_partitions
                        .contains(&sorted_pair(&msg.from, &msg.to))
                    {
                        continue;
                    }
                    deliverable.push((idx, msg.id));
                }
                if deliverable.is_empty() {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "net_deliver".to_string(),
                        message: "no deliverable network message".to_string(),
                        location: None,
                    });
                }

                let use_random = strategy
                    .as_deref()
                    .map(|s| s.eq_ignore_ascii_case("random"))
                    .unwrap_or(self.net_reorder);

                let picked_message_id = match self.replay_peek() {
                    Some(Decision::NetDeliverPick { message_id }) => {
                        let id = *message_id;
                        if !deliverable.iter().any(|(_, m)| *m == id) {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!(
                                    "replay net delivery drift: message id {id} is not deliverable"
                                ),
                                location: None,
                            });
                        }
                        let _ =
                            self.replay_take_if(|d| matches!(d, Decision::NetDeliverPick { .. }));
                        id
                    }
                    _ => {
                        let pick_pos = if use_random {
                            (self.rng.next_u64() as usize) % deliverable.len()
                        } else {
                            0
                        };
                        deliverable[pick_pos].1
                    }
                };
                self.decisions.push(Decision::NetDeliverPick {
                    message_id: picked_message_id,
                });

                let Some((idx, _)) = deliverable
                    .into_iter()
                    .find(|(_, id)| *id == picked_message_id)
                else {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "net_deliver".to_string(),
                        message: format!(
                            "selected message id {picked_message_id} no longer in queue"
                        ),
                        location: None,
                    });
                };
                let msg = self.net_queue.remove(idx).expect("queue index exists");

                if let Some(Decision::NetDrop { message_id, .. }) = self.replay_peek()
                    && *message_id != msg.id
                {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "replay_drift".to_string(),
                        message: format!(
                            "replay net drop drift: expected message id {}, got {}",
                            msg.id, message_id
                        ),
                        location: None,
                    });
                }

                let should_drop = match self.replay_take_if(
                    |d| matches!(d, Decision::NetDrop { message_id, .. } if *message_id == msg.id),
                ) {
                    Some(Decision::NetDrop { dropped, .. }) => dropped,
                    _ => {
                        if self.net_drop_rate <= 0.0 {
                            false
                        } else {
                            let sample = (self.rng.next_u64() as f64) / (u64::MAX as f64);
                            sample < self.net_drop_rate
                        }
                    }
                };
                self.decisions.push(Decision::NetDrop {
                    message_id: msg.id,
                    dropped: should_drop,
                });

                if should_drop {
                    self.events.push(TraceEvent {
                        time_ms: self.clock.now_ms(),
                        name: "net_drop".to_string(),
                        fields: serde_json::Map::from_iter([
                            ("id".to_string(), serde_json::Value::Number(msg.id.into())),
                            ("from".to_string(), serde_json::Value::String(msg.from)),
                            ("to".to_string(), serde_json::Value::String(msg.to)),
                            (
                                "payload_size".to_string(),
                                serde_json::json!(msg.payload.len() as u64),
                            ),
                        ]),
                    });
                    return Ok(true);
                }

                self.net_inbox
                    .entry(msg.to.clone())
                    .or_default()
                    .push(msg.clone());
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "net_deliver".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("id".to_string(), serde_json::Value::Number(msg.id.into())),
                        ("from".to_string(), serde_json::Value::String(msg.from)),
                        ("to".to_string(), serde_json::Value::String(msg.to)),
                        (
                            "payload_size".to_string(),
                            serde_json::json!(msg.payload.len() as u64),
                        ),
                    ]),
                });
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "capability_net".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("op".to_string(), serde_json::json!("deliver")),
                        (
                            "payload_bytes".to_string(),
                            serde_json::json!(msg.payload.len() as u64),
                        ),
                        ("duration_ms".to_string(), serde_json::json!(0u64)),
                    ]),
                });
                Ok(true)
            }

            crate::Step::NetRecvAssert {
                node,
                from,
                payload,
            } => {
                let inbox = self.net_inbox.entry(node.clone()).or_default();
                let pos = inbox.iter().position(|m| {
                    if let Some(f) = from
                        && &m.from != f
                    {
                        return false;
                    }
                    m.payload == *payload
                });
                let Some(pos) = pos else {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "net_recv_assert".to_string(),
                        message: format!(
                            "no matching inbox message for node {node:?} payload {payload:?}"
                        ),
                        location: None,
                    });
                };
                inbox.remove(pos);
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
