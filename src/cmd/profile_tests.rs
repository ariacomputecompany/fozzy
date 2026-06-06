use super::*;
#[allow(unused_imports)]
use crate::{ExitStatus, RunIdentity, RunMode, RunSummary, ScenarioV1Steps, Step, TraceEvent};
#[allow(unused_imports)]
use std::path::PathBuf;

fn sample_trace() -> TraceFile {
    TraceFile {
        format: "fozzy-trace".to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Run,
        scenario_path: None,
        scenario: Some(ScenarioV1Steps {
            version: 1,
            name: "no-heap".to_string(),
            steps: vec![
                Step::TraceEvent {
                    name: "setup".to_string(),
                    fields: serde_json::Map::new(),
                },
                Step::TraceEvent {
                    name: "teardown".to_string(),
                    fields: serde_json::Map::new(),
                },
            ],
        }),
        fuzz: None,
        explore: None,
        memory: None,
        decisions: Vec::new(),
        events: vec![
            TraceEvent {
                time_ms: 1,
                name: "http_request".to_string(),
                fields: serde_json::Map::new(),
            },
            TraceEvent {
                time_ms: 4,
                name: "memory_alloc".to_string(),
                fields: serde_json::Map::from_iter([
                    ("alloc_id".to_string(), serde_json::json!(1)),
                    ("bytes".to_string(), serde_json::json!(64)),
                    (
                        "callsite_hash".to_string(),
                        serde_json::json!("step:memory_alloc"),
                    ),
                ]),
            },
            TraceEvent {
                time_ms: 8,
                name: "memory_free".to_string(),
                fields: serde_json::Map::from_iter([(
                    "alloc_id".to_string(),
                    serde_json::json!(1),
                )]),
            },
        ],
        summary: RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "r1".to_string(),
                seed: 7,
                trace_path: None,
                report_path: None,
                artifacts_dir: None,
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 10,
            duration_ns: 10_000_000,
            tests: None,
            memory: None,
            findings: Vec::new(),
        },
        checksum: None,
    }
}

#[allow(dead_code)]
fn sample_trace_without_heap() -> TraceFile {
    TraceFile {
        format: "fozzy-trace".to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Run,
        scenario_path: None,
        scenario: Some(ScenarioV1Steps {
            version: 1,
            name: "no-heap".to_string(),
            steps: vec![
                Step::TraceEvent {
                    name: "setup".to_string(),
                    fields: serde_json::Map::new(),
                },
                Step::TraceEvent {
                    name: "teardown".to_string(),
                    fields: serde_json::Map::new(),
                },
            ],
        }),
        fuzz: None,
        explore: None,
        memory: None,
        decisions: Vec::new(),
        events: vec![
            TraceEvent {
                time_ms: 1,
                name: "setup".to_string(),
                fields: serde_json::Map::new(),
            },
            TraceEvent {
                time_ms: 5,
                name: "work".to_string(),
                fields: serde_json::Map::new(),
            },
            TraceEvent {
                time_ms: 9,
                name: "teardown".to_string(),
                fields: serde_json::Map::new(),
            },
        ],
        summary: RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "r-no-heap".to_string(),
                seed: 9,
                trace_path: None,
                report_path: None,
                artifacts_dir: None,
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 1,
            duration_ns: 1_000_000,
            tests: None,
            memory: None,
            findings: Vec::new(),
        },
        checksum: None,
    }
}

#[allow(dead_code)]
fn sample_trace_with_cpu_samples() -> TraceFile {
    let mut trace = sample_trace();
    trace.events = vec![
        TraceEvent {
            time_ms: 1,
            name: "sample".to_string(),
            fields: serde_json::Map::from_iter([
                (
                    "stack".to_string(),
                    serde_json::json!("fozzy::runtime;step::cpu"),
                ),
                ("weight_ms".to_string(), serde_json::json!(4)),
            ]),
        },
        TraceEvent {
            time_ms: 2,
            name: "sample".to_string(),
            fields: serde_json::Map::from_iter([
                (
                    "stack".to_string(),
                    serde_json::json!("fozzy::runtime;step::cpu"),
                ),
                ("weight_ms".to_string(), serde_json::json!(6)),
            ]),
        },
    ];
    trace
}

#[allow(dead_code)]
fn sample_trace_with_full_profile_support() -> TraceFile {
    let mut trace = sample_trace();
    let mut sample_events = vec![
        TraceEvent {
            time_ms: 1,
            name: "sample".to_string(),
            fields: serde_json::Map::from_iter([
                (
                    "stack".to_string(),
                    serde_json::json!("fozzy::runtime;step::cpu"),
                ),
                ("weight_ms".to_string(), serde_json::json!(4)),
            ]),
        },
        TraceEvent {
            time_ms: 2,
            name: "sample".to_string(),
            fields: serde_json::Map::from_iter([
                (
                    "stack".to_string(),
                    serde_json::json!("fozzy::runtime;step::cpu"),
                ),
                ("weight_ms".to_string(), serde_json::json!(6)),
            ]),
        },
    ];
    sample_events.extend(trace.events);
    trace.events = sample_events;
    trace
}

#[path = "profile_tests/analytics.rs"]
mod analytics;
#[path = "profile_tests/resolution.rs"]
mod resolution;
#[path = "profile_tests/shrink.rs"]
mod shrink;
