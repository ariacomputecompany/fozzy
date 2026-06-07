use std::time::Duration;

use crate::host::{HostHttpDispatch, assert_http_when_response_matches_host, canonical_headers, dispatch_host_http, host_http_rule_matches, host_http_rule_path_supported};
use crate::{Decision, Finding, FindingKind, TraceEvent};

use super::ExecCtx;
use super::super::helpers::{HttpRule, decode_hex, encode_hex, measure_duration_ms};
use super::super::types::{FsBackend, HttpBackend};

impl ExecCtx<'_> {
    pub(super) fn exec_file_http_step(&mut self, step: &crate::Step) -> Result<bool, Finding> {
        match step {

            crate::Step::FsWrite { path, data } => {
                let start_ms = self.clock.now_ms();
                if let Some(Decision::FsWrite {
                    path: replay_path,
                    data_hex,
                    duration_ms,
                }) = self.replay_peek().cloned()
                {
                    if replay_path != *path {
                        return Err(Finding {
                            kind: FindingKind::Checker,
                            title: "replay_drift".to_string(),
                            message: format!(
                                "replay fs write drift: expected path {replay_path:?}, got {path:?}"
                            ),
                            location: None,
                        });
                    }
                    let expected = decode_hex(&data_hex).map_err(|message| Finding {
                        kind: FindingKind::Checker,
                        title: "replay_drift".to_string(),
                        message: format!(
                            "replay fs write drift: invalid recorded bytes for {path:?}: {message}"
                        ),
                        location: None,
                    })?;
                    if expected != data.as_bytes() {
                        return Err(Finding {
                            kind: FindingKind::Checker,
                            title: "replay_drift".to_string(),
                            message: format!(
                                "replay fs write drift: expected payload for {path:?} to match recorded host bytes"
                            ),
                            location: None,
                        });
                    }
                    let _ = self.replay_take_if(|d| matches!(d, Decision::FsWrite { .. }));
                    self.replay_host_fs_write(path, data.as_bytes());
                    self.advance_recorded_time(duration_ms);
                } else if matches!(self.fs_backend, FsBackend::Host) {
                    let (_, duration_ms) = measure_duration_ms(|| self.host_fs_write(path, data))?;
                    self.decisions.push(Decision::FsWrite {
                        path: path.clone(),
                        data_hex: encode_hex(data.as_bytes()),
                        duration_ms,
                    });
                    self.advance_recorded_time(duration_ms);
                } else {
                    self.fs.insert(path.clone(), data.clone());
                }
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "capability_fs".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("op".to_string(), serde_json::json!("write")),
                        ("path".to_string(), serde_json::json!(path)),
                        (
                            "backend".to_string(),
                            serde_json::json!(match self.fs_backend {
                                FsBackend::Virtual => "virtual",
                                FsBackend::Host => "host",
                            }),
                        ),
                        (
                            "payload_bytes".to_string(),
                            serde_json::json!(data.len() as u64),
                        ),
                        (
                            "duration_ms".to_string(),
                            serde_json::json!(self.clock.now_ms().saturating_sub(start_ms)),
                        ),
                    ]),
                });
                Ok(true)
            }

            crate::Step::FsReadAssert { path, equals } => {
                let start_ms = self.clock.now_ms();
                if let Some(Decision::FsReadAssert {
                    path: replay_path,
                    data_hex,
                    duration_ms,
                }) = self.replay_peek().cloned()
                {
                    if replay_path != *path {
                        return Err(Finding {
                            kind: FindingKind::Checker,
                            title: "replay_drift".to_string(),
                            message: format!(
                                "replay fs read drift: expected path {replay_path:?}, got {path:?}"
                            ),
                            location: None,
                        });
                    }
                    let bytes = decode_hex(&data_hex).map_err(|message| Finding {
                        kind: FindingKind::Checker,
                        title: "replay_drift".to_string(),
                        message: format!(
                            "replay fs read drift: invalid recorded bytes for {path:?}: {message}"
                        ),
                        location: None,
                    })?;
                    let _ = self.replay_take_if(|d| matches!(d, Decision::FsReadAssert { .. }));
                    self.replay_host_fs_write(path, &bytes);
                    self.replay_host_fs_read_assert(path, equals)?;
                    self.advance_recorded_time(duration_ms);
                } else if matches!(self.fs_backend, FsBackend::Host) {
                    let (_, duration_ms) =
                        measure_duration_ms(|| self.host_fs_read_assert(path, equals))?;
                    self.decisions.push(Decision::FsReadAssert {
                        path: path.clone(),
                        data_hex: encode_hex(equals.as_bytes()),
                        duration_ms,
                    });
                    self.advance_recorded_time(duration_ms);
                } else {
                    let got = self.fs.get(path).cloned();
                    if got.as_deref() != Some(equals.as_str()) {
                        return Err(Finding {
                            kind: FindingKind::Assertion,
                            title: "fs_read_assert".to_string(),
                            message: format!("expected {path:?} == {equals:?}, got {got:?}"),
                            location: None,
                        });
                    }
                }
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "capability_fs".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("op".to_string(), serde_json::json!("read_assert")),
                        ("path".to_string(), serde_json::json!(path)),
                        (
                            "backend".to_string(),
                            serde_json::json!(match self.fs_backend {
                                FsBackend::Virtual => "virtual",
                                FsBackend::Host => "host",
                            }),
                        ),
                        (
                            "payload_bytes".to_string(),
                            serde_json::json!(equals.len() as u64),
                        ),
                        (
                            "duration_ms".to_string(),
                            serde_json::json!(self.clock.now_ms().saturating_sub(start_ms)),
                        ),
                    ]),
                });
                Ok(true)
            }

            crate::Step::FsSnapshot { name } => {
                let start_ms = self.clock.now_ms();
                if let Some(Decision::FsSnapshot {
                    name: replay_name,
                    entries,
                    duration_ms,
                }) = self.replay_peek().cloned()
                {
                    if replay_name != *name {
                        return Err(Finding {
                            kind: FindingKind::Checker,
                            title: "replay_drift".to_string(),
                            message: format!(
                                "replay fs snapshot drift: expected snapshot {replay_name:?}, got {name:?}"
                            ),
                            location: None,
                        });
                    }
                    let _ = self.replay_take_if(|d| matches!(d, Decision::FsSnapshot { .. }));
                    self.apply_replay_host_fs_snapshot(name, &entries)?;
                    self.advance_recorded_time(duration_ms);
                } else if matches!(self.fs_backend, FsBackend::Host) {
                    let (_, duration_ms) = measure_duration_ms(|| self.host_fs_snapshot(name))?;
                    let entries = self
                        .host_fs_snapshots
                        .get(name)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(path, value)| {
                            (
                                path.to_string_lossy().to_string(),
                                value.map(|bytes| encode_hex(&bytes)),
                            )
                        })
                        .collect();
                    self.decisions.push(Decision::FsSnapshot {
                        name: name.clone(),
                        entries,
                        duration_ms,
                    });
                    self.advance_recorded_time(duration_ms);
                } else {
                    self.fs_snapshots.insert(name.clone(), self.fs.clone());
                }
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "capability_fs".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("op".to_string(), serde_json::json!("snapshot")),
                        ("name".to_string(), serde_json::json!(name)),
                        (
                            "backend".to_string(),
                            serde_json::json!(match self.fs_backend {
                                FsBackend::Virtual => "virtual",
                                FsBackend::Host => "host",
                            }),
                        ),
                        (
                            "duration_ms".to_string(),
                            serde_json::json!(self.clock.now_ms().saturating_sub(start_ms)),
                        ),
                    ]),
                });
                Ok(true)
            }

            crate::Step::FsRestore { name } => {
                let start_ms = self.clock.now_ms();
                if let Some(Decision::FsRestore {
                    name: replay_name,
                    duration_ms,
                }) = self.replay_peek().cloned()
                {
                    if replay_name != *name {
                        return Err(Finding {
                            kind: FindingKind::Checker,
                            title: "replay_drift".to_string(),
                            message: format!(
                                "replay fs restore drift: expected snapshot {replay_name:?}, got {name:?}"
                            ),
                            location: None,
                        });
                    }
                    let _ = self.replay_take_if(|d| matches!(d, Decision::FsRestore { .. }));
                    self.apply_replay_host_fs_restore(name)?;
                    self.advance_recorded_time(duration_ms);
                } else if matches!(self.fs_backend, FsBackend::Host) {
                    let (_, duration_ms) = measure_duration_ms(|| self.host_fs_restore(name))?;
                    self.decisions.push(Decision::FsRestore {
                        name: name.clone(),
                        duration_ms,
                    });
                    self.advance_recorded_time(duration_ms);
                } else {
                    let Some(snapshot) = self.fs_snapshots.get(name).cloned() else {
                        return Err(Finding {
                            kind: FindingKind::Checker,
                            title: "fs_restore_missing_snapshot".to_string(),
                            message: format!("missing fs snapshot {name:?}"),
                            location: None,
                        });
                    };
                    self.fs = snapshot;
                }
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "capability_fs".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("op".to_string(), serde_json::json!("restore")),
                        ("name".to_string(), serde_json::json!(name)),
                        (
                            "backend".to_string(),
                            serde_json::json!(match self.fs_backend {
                                FsBackend::Virtual => "virtual",
                                FsBackend::Host => "host",
                            }),
                        ),
                        (
                            "duration_ms".to_string(),
                            serde_json::json!(self.clock.now_ms().saturating_sub(start_ms)),
                        ),
                    ]),
                });
                Ok(true)
            }

            crate::Step::HttpWhen {
                method,
                path,
                status,
                headers,
                body,
                json,
                delay,
                times,
            } => {
                if body.is_some() && json.is_some() {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "http_when_invalid".to_string(),
                        message: "HttpWhen: cannot set both body and json".to_string(),
                        location: None,
                    });
                }

                let delay_ms = if let Some(d) = delay {
                    let dur = crate::parse_duration(d).map_err(|e| Finding {
                        kind: FindingKind::Checker,
                        title: "invalid_duration".to_string(),
                        message: e.to_string(),
                        location: None,
                    })?;
                    dur.as_millis().min(u128::from(u64::MAX)) as u64
                } else {
                    0
                };
                if matches!(self.http_backend, HttpBackend::Host)
                    && !host_http_rule_path_supported(path)
                {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "http_when_host_path".to_string(),
                        message: format!(
                            "http_when path {path:?} is not supported with host backend; use an absolute http(s) url or a path beginning with '/'. examples: \
                             {{\"type\":\"http_when\",\"method\":\"GET\",\"path\":\"https://api.example.com/v1/me\",...}} \
                             or {{\"type\":\"http_when\",\"method\":\"GET\",\"path\":\"/v1/me\",...}}"
                        ),
                        location: None,
                    });
                }

                self.http_rules.push(HttpRule {
                    method: method.clone(),
                    path: path.clone(),
                    status: *status,
                    headers: canonical_headers(headers.as_ref())?,
                    body: body.clone(),
                    json: json.clone(),
                    delay_ms,
                    remaining: times.unwrap_or(u64::MAX),
                });
                Ok(true)
            }

            crate::Step::HttpRequest {
                method,
                path,
                headers,
                body,
                expect_status,
                expect_headers,
                expect_body,
                expect_json,
                save_body_as,
            } => {
                let start_ms = self.clock.now_ms();
                if expect_body.is_some() && expect_json.is_some() {
                    return Err(Finding {
                        kind: FindingKind::Checker,
                        title: "http_request_invalid".to_string(),
                        message: "HttpRequest: cannot set both expect_body and expect_json"
                            .to_string(),
                        location: None,
                    });
                }
                let (status_code, resp_headers, resp_body, backend) = match self
                    .replay_peek()
                    .cloned()
                {
                    Some(Decision::HttpRequest {
                        method: replay_method,
                        path: replay_path,
                        status_code,
                        headers,
                        body,
                        duration_ms,
                    }) => {
                        if replay_method != *method || replay_path != *path {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!(
                                    "replay http drift: expected {replay_method} {replay_path}, got {method} {path}"
                                ),
                                location: None,
                            });
                        }
                        let _ = self.replay_take_if(|d| matches!(d, Decision::HttpRequest { .. }));
                        self.advance_recorded_time(duration_ms);
                        (status_code, headers, body, "replay".to_string())
                    }
                    Some(Decision::HttpRequestTimeout {
                        method: replay_method,
                        path: replay_path,
                        duration_ms,
                    }) => {
                        if replay_method != *method || replay_path != *path {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "replay_drift".to_string(),
                                message: format!(
                                    "replay http timeout drift: expected {replay_method} {replay_path}, got {method} {path}"
                                ),
                                location: None,
                            });
                        }
                        let _ = self
                            .replay_take_if(|d| matches!(d, Decision::HttpRequestTimeout { .. }));
                        self.advance_recorded_time(duration_ms);
                        return Err(Finding {
                            kind: FindingKind::Hang,
                            title: "timeout".to_string(),
                            message: format!("host http request timed out for {method} {path}"),
                            location: self.current_finding_location(),
                        });
                    }
                    _ if matches!(self.http_backend, HttpBackend::Host) => {
                        if !path.starts_with("http://") && !path.starts_with("https://") {
                            return Err(Finding {
                                kind: FindingKind::Checker,
                                title: "http_host_url".to_string(),
                                message: format!(
                                    "host http backend requires absolute http/https url, got {path:?}"
                                ),
                                location: None,
                            });
                        }
                        let host_rule_idx = self.http_rules.iter().position(|r| {
                            r.remaining > 0
                                && r.method == *method
                                && host_http_rule_matches(&r.path, path)
                        });
                        if !self.http_rules.is_empty() && host_rule_idx.is_none() {
                            return Err(Finding {
                                kind: FindingKind::Assertion,
                                title: "http_when_host_unmatched".to_string(),
                                message: format!(
                                    "no http_when matched host request {method} {path}. remediation: \
                                 1) align http_when.method/path with this request (absolute url or '/path'), \
                                 2) run with --http-backend scripted to use mocked responses. example: \
                                 fozzy run <scenario.fozzy.json> --http-backend scripted --json"
                                ),
                                location: None,
                            });
                        }
                        let request_headers = canonical_headers(headers.as_ref())?;
                        let (response, duration_ms) = measure_duration_ms(|| {
                            dispatch_host_http(
                                method,
                                path,
                                &request_headers,
                                body.as_deref(),
                                self.remaining_host_timeout(),
                            )
                            .map_err(|message| Finding {
                                kind: FindingKind::Assertion,
                                title: "http_host_request".to_string(),
                                message,
                                location: None,
                            })
                        })?;
                        let response = match response {
                            HostHttpDispatch::Completed(response) => {
                                self.decisions.push(Decision::HttpRequest {
                                    method: method.clone(),
                                    path: path.clone(),
                                    status_code: response.status,
                                    headers: response.headers.clone(),
                                    body: response.body.clone(),
                                    duration_ms,
                                });
                                self.advance_recorded_time(duration_ms);
                                response
                            }
                            HostHttpDispatch::TimedOut => {
                                self.decisions.push(Decision::HttpRequestTimeout {
                                    method: method.clone(),
                                    path: path.clone(),
                                    duration_ms,
                                });
                                self.advance_recorded_time(duration_ms);
                                return Err(Finding {
                                    kind: FindingKind::Hang,
                                    title: "timeout".to_string(),
                                    message: format!(
                                        "host http request timed out for {method} {path}"
                                    ),
                                    location: self.current_finding_location(),
                                });
                            }
                        };
                        if let Some(idx) = host_rule_idx {
                            let mut rule = self.http_rules[idx].clone();
                            if rule.remaining != u64::MAX {
                                rule.remaining = rule.remaining.saturating_sub(1);
                            }
                            self.http_rules[idx] = rule.clone();
                            assert_http_when_response_matches_host(
                                method,
                                path,
                                rule.status,
                                &rule.headers,
                                rule.body.as_deref(),
                                rule.json.as_ref(),
                                response.status,
                                &response.headers,
                                &response.body,
                            )?;
                        }
                        (
                            response.status,
                            response.headers,
                            response.body,
                            "host".to_string(),
                        )
                    }
                    _ => {
                        let rule_idx = self.http_rules.iter().position(|r| {
                            r.remaining > 0 && r.method == *method && r.path == *path
                        });
                        let Some(idx) = rule_idx else {
                            return Err(Finding {
                                kind: FindingKind::Assertion,
                                title: "http_unmatched".to_string(),
                                message: format!("no http mock matched {method} {path}"),
                                location: None,
                            });
                        };

                        let mut rule = self.http_rules[idx].clone();
                        if rule.remaining != u64::MAX {
                            rule.remaining = rule.remaining.saturating_sub(1);
                        }
                        self.http_rules[idx] = rule.clone();

                        if self.det && rule.delay_ms > 0 {
                            self.clock.advance(Duration::from_millis(rule.delay_ms));
                        } else if !self.det && rule.delay_ms > 0 {
                            std::thread::sleep(Duration::from_millis(rule.delay_ms));
                        }

                        let resp_body = if let Some(j) = &rule.json {
                            serde_json::to_string(j).map_err(|e| Finding {
                                kind: FindingKind::Checker,
                                title: "http_json_serialize".to_string(),
                                message: e.to_string(),
                                location: None,
                            })?
                        } else {
                            rule.body.clone().unwrap_or_default()
                        };
                        (
                            rule.status,
                            rule.headers.clone(),
                            resp_body,
                            "scripted".to_string(),
                        )
                    }
                };

                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "http_request".to_string(),
                    fields: serde_json::Map::from_iter([
                        (
                            "method".to_string(),
                            serde_json::Value::String(method.clone()),
                        ),
                        ("path".to_string(), serde_json::Value::String(path.clone())),
                        ("backend".to_string(), serde_json::Value::String(backend)),
                        (
                            "status".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(status_code)),
                        ),
                        (
                            "has_body".to_string(),
                            serde_json::Value::Bool(!resp_body.is_empty()),
                        ),
                        (
                            "header_count".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(
                                resp_headers.len() as u64
                            )),
                        ),
                        (
                            "request_payload_bytes".to_string(),
                            serde_json::json!(body.as_ref().map(|s| s.len() as u64).unwrap_or(0)),
                        ),
                        (
                            "response_payload_bytes".to_string(),
                            serde_json::json!(resp_body.len() as u64),
                        ),
                        (
                            "duration_ms".to_string(),
                            serde_json::json!(self.clock.now_ms().saturating_sub(start_ms)),
                        ),
                    ]),
                });
                self.events.push(TraceEvent {
                    time_ms: self.clock.now_ms(),
                    name: "capability_http".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("op".to_string(), serde_json::json!("request")),
                        ("method".to_string(), serde_json::json!(method)),
                        ("path".to_string(), serde_json::json!(path)),
                        (
                            "payload_bytes".to_string(),
                            serde_json::json!(resp_body.len() as u64),
                        ),
                        (
                            "duration_ms".to_string(),
                            serde_json::json!(self.clock.now_ms().saturating_sub(start_ms)),
                        ),
                    ]),
                });

                if let Some(expected) = expect_status
                    && status_code != *expected
                {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "http_status".to_string(),
                        message: format!("expected status {expected}, got {}", status_code),
                        location: None,
                    });
                }

                if let Some(expected) = expect_body
                    && resp_body != *expected
                {
                    return Err(Finding {
                        kind: FindingKind::Assertion,
                        title: "http_body".to_string(),
                        message: "http response body mismatch".to_string(),
                        location: None,
                    });
                }

                if let Some(expected) = expect_json {
                    let got: serde_json::Value =
                        serde_json::from_str(&resp_body).map_err(|e| Finding {
                            kind: FindingKind::Assertion,
                            title: "http_json_parse".to_string(),
                            message: e.to_string(),
                            location: None,
                        })?;
                    if got != *expected {
                        return Err(Finding {
                            kind: FindingKind::Assertion,
                            title: "http_json".to_string(),
                            message: "http response json mismatch".to_string(),
                            location: None,
                        });
                    }
                }

                if let Some(expected_headers) = expect_headers {
                    let expected = canonical_headers(Some(expected_headers))?;
                    for (k, v) in expected {
                        let got = resp_headers.get(&k);
                        if got != Some(&v) {
                            return Err(Finding {
                                kind: FindingKind::Assertion,
                                title: "http_headers".to_string(),
                                message: format!(
                                    "http response header mismatch for {k:?}: expected {v:?}, got {got:?}"
                                ),
                                location: None,
                            });
                        }
                    }
                }

                if let Some(key) = save_body_as {
                    self.kv.insert(key.clone(), resp_body);
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
