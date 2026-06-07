use rand_core::RngCore as _;

use crate::{Decision, Finding, FindingKind, TraceEvent};

use super::ExecCtx;

impl ExecCtx<'_> {
    pub(super) fn exec_basic_step(&mut self, step: &crate::Step) -> Result<bool, Finding> {
        match step {
            crate::Step::TraceEvent { name, fields } => {
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: name.clone(),
                    fields: fields.clone(),
                });
                Ok(true)
            }

            crate::Step::RandU64 { key } => {
                let value = self.rng.next_u64();
                self.decisions.push(Decision::RandU64 { value });
                if let Some(cur) = self.replay.as_mut() {
                    match cur.next() {
                        Some(Decision::RandU64 { value: expected }) if *expected == value => {}
                        Some(other) => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!("expected RandU64({value}), got {other:?}"),
                                location: None,
                            });
                        }
                        None => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: "missing RandU64 decision".to_string(),
                                location: None,
                            });
                        }
                    }
                }
                if let Some(key) = key {
                    self.kv.insert(key.clone(), value.to_string());
                }
                Ok(true)
            }

            crate::Step::AssertOk { value, msg } => {
                if !value {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "assert_ok".to_string(),
                        message: msg
                            .clone()
                            .unwrap_or_else(|| "assert_ok failed".to_string()),
                        location: None,
                    });
                }
                Ok(true)
            }

            crate::Step::AssertEqInt { a, b, msg } => {
                if a != b {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "assert_eq_int".to_string(),
                        message: msg
                            .clone()
                            .unwrap_or_else(|| format!("expected {a} == {b}")),
                        location: None,
                    });
                }
                Ok(true)
            }

            crate::Step::AssertNeInt { a, b, msg } => {
                if a == b {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "assert_ne_int".to_string(),
                        message: msg
                            .clone()
                            .unwrap_or_else(|| format!("expected {a} != {b}")),
                        location: None,
                    });
                }
                Ok(true)
            }

            crate::Step::AssertEqStr { a, b, msg } => {
                if a != b {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "assert_eq_str".to_string(),
                        message: msg
                            .clone()
                            .unwrap_or_else(|| format!("expected {a:?} == {b:?}")),
                        location: None,
                    });
                }
                Ok(true)
            }

            crate::Step::AssertNeStr { a, b, msg } => {
                if a == b {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "assert_ne_str".to_string(),
                        message: msg
                            .clone()
                            .unwrap_or_else(|| format!("expected {a:?} != {b:?}")),
                        location: None,
                    });
                }
                Ok(true)
            }

            crate::Step::Sleep { duration } => {
                let d = crate::parse_duration(duration).map_err(|e| Finding {
                    kind: FindingKind::Checker,
                    title: "invalid_duration".to_string(),
                    message: e.to_string(),
                    location: None,
                })?;
                let ms = d.as_millis().min(u128::from(u64::MAX)) as u64;
                if self.det {
                    self.clock.sleep(d);
                    self.decisions.push(Decision::TimeSleepMs { ms });
                } else {
                    std::thread::sleep(d);
                }
                if let Some(cur) = self.replay.as_mut() {
                    match cur.next() {
                        Some(Decision::TimeSleepMs { ms: expected }) if *expected == ms => {}
                        Some(other) => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!("expected TimeSleepMs({ms}), got {other:?}"),
                                location: None,
                            });
                        }
                        None => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: "missing TimeSleepMs decision".to_string(),
                                location: None,
                            });
                        }
                    }
                }
                Ok(true)
            }

            crate::Step::Advance { duration } => {
                let d = crate::parse_duration(duration).map_err(|e| Finding {
                    kind: FindingKind::Checker,
                    title: "invalid_duration".to_string(),
                    message: e.to_string(),
                    location: None,
                })?;
                let ms = d.as_millis().min(u128::from(u64::MAX)) as u64;
                if !self.det {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "advance_requires_det".to_string(),
                        message: "Advance is only supported in deterministic mode (--det)"
                            .to_string(),
                        location: None,
                    });
                }

                self.clock.advance(d);
                self.decisions.push(Decision::TimeAdvanceMs { ms });
                if let Some(cur) = self.replay.as_mut() {
                    match cur.next() {
                        Some(Decision::TimeAdvanceMs { ms: expected }) if *expected == ms => {}
                        Some(other) => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!("expected TimeAdvanceMs({ms}), got {other:?}"),
                                location: None,
                            });
                        }
                        None => {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: "missing TimeAdvanceMs decision".to_string(),
                                location: None,
                            });
                        }
                    }
                }
                Ok(true)
            }

            crate::Step::Freeze { at_ms } => {
                self.clock.freeze(*at_ms);
                Ok(true)
            }

            crate::Step::Unfreeze => {
                self.clock.unfreeze();
                Ok(true)
            }

            crate::Step::SetKv { key, value } => {
                self.kv.insert(key.clone(), value.clone());
                Ok(true)
            }

            crate::Step::GetKvAssert {
                key,
                equals,
                is_null,
            } => {
                let v = self.kv.get(key).cloned();
                if is_null.unwrap_or(false) {
                    if v.is_some() {
                        return Err(Finding {
                            kind: FindingKind::Assertion,
                            title: "get_kv_assert".to_string(),
                            message: format!("expected {key:?} to be null"),
                            location: None,
                        });
                    }
                    return Ok(true);
                }

                if let Some(expected) = equals {
                    if v.as_deref() != Some(expected.as_str()) {
                        return Err(Finding {
                            kind: FindingKind::Assertion,
                            title: "get_kv_assert".to_string(),
                            message: format!("expected {key:?} == {expected:?}, got {v:?}"),
                            location: None,
                        });
                    }
                } else if v.is_none() {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "get_kv_assert".to_string(),
                        message: format!("expected {key:?} to exist"),
                        location: None,
                    });
                }

                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
