use super::*;

#[test]
fn timeline_builds_required_fields() {
    let trace = sample_trace();
    let timeline = build_profile_timeline(&trace);
    assert_eq!(timeline.len(), 3);
    assert_eq!(timeline[0].run_id, "r1");
    assert_eq!(timeline[0].seed, 7);
}

#[test]
fn diff_is_deterministic() {
    let trace = sample_trace();
    let timeline = build_profile_timeline(&trace);
    let cpu = build_cpu_profile(&trace, &timeline);
    let heap = build_heap_profile(&trace, &timeline);
    let latency = build_latency_profile(&trace, &timeline);
    let metrics = build_profile_metrics(&trace, &timeline, &cpu, &heap, &latency);
    let diff_a = compute_diff(
        "a",
        "b",
        &["latency".to_string(), "heap".to_string()],
        &metrics,
        &metrics,
        Some(&heap),
        Some(&heap),
        &HashMap::new(),
        &HashMap::new(),
        1,
        1,
    );
    let diff_b = compute_diff(
        "a",
        "b",
        &["latency".to_string(), "heap".to_string()],
        &metrics,
        &metrics,
        Some(&heap),
        Some(&heap),
        &HashMap::new(),
        &HashMap::new(),
        1,
        1,
    );
    assert_eq!(
        serde_json::to_string(&diff_a).expect("json"),
        serde_json::to_string(&diff_b).expect("json")
    );
}

#[test]
fn profile_event_schema_roundtrip_and_compatibility() {
    let trace = sample_trace();
    let timeline = build_profile_timeline(&trace);
    let event = timeline.first().expect("event");
    let encoded = serde_json::to_vec(event).expect("encode");
    let decoded: ProfileEvent = serde_json::from_slice(&encoded).expect("decode");
    assert_eq!(decoded.t_virtual, event.t_virtual);
    assert_eq!(decoded.kind, event.kind);
    assert_eq!(decoded.run_id, event.run_id);
    assert_eq!(decoded.seed, event.seed);
    assert_eq!(decoded.thread, event.thread);
    assert_eq!(decoded.span_id, event.span_id);

    let compat_json = serde_json::json!({
        "t_virtual": 42,
        "kind": "io",
        "run_id": "compat-run",
        "seed": 99,
        "thread": "main",
        "span_id": "s-1",
        "cost": {"duration_ms": 3},
        "unknown_field": "ignored"
    });
    let compat: ProfileEvent = serde_json::from_value(compat_json).expect("compat decode");
    assert_eq!(compat.t_virtual, 42);
    assert_eq!(compat.kind, ProfileEventKind::Io);
    assert_eq!(compat.t_mono, None);
    assert_eq!(compat.task, None);
    assert!(compat.tags.is_empty());
    assert_eq!(compat.cost.duration_ms, Some(3));
}

#[test]
fn folded_stack_aggregation_is_correct_and_stable() {
    let mut trace = sample_trace();
    trace.events = vec![
        TraceEvent {
            time_ms: 1,
            name: "sample".to_string(),
            fields: serde_json::Map::from_iter([
                (
                    "stack".to_string(),
                    serde_json::json!("fozzy::runtime;step::a"),
                ),
                ("weight_ms".to_string(), serde_json::json!(3)),
            ]),
        },
        TraceEvent {
            time_ms: 2,
            name: "sample".to_string(),
            fields: serde_json::Map::from_iter([
                (
                    "stack".to_string(),
                    serde_json::json!("fozzy::runtime;step::a"),
                ),
                ("weight_ms".to_string(), serde_json::json!(2)),
            ]),
        },
        TraceEvent {
            time_ms: 3,
            name: "sample".to_string(),
            fields: serde_json::Map::from_iter([
                (
                    "stack".to_string(),
                    serde_json::json!("fozzy::runtime;step::b"),
                ),
                ("weight_ms".to_string(), serde_json::json!(5)),
            ]),
        },
    ];
    let timeline = build_profile_timeline(&trace);
    let cpu = build_cpu_profile(&trace, &timeline);
    assert_eq!(cpu.folded_stacks.len(), 2);
    assert_eq!(cpu.folded_stacks[0].stack, "fozzy::runtime;step::a");
    assert_eq!(cpu.folded_stacks[0].weight, 5);
    assert_eq!(cpu.folded_stacks[1].stack, "fozzy::runtime;step::b");
    assert_eq!(cpu.folded_stacks[1].weight, 5);
}

#[test]
fn latency_critical_path_extraction_is_correct() {
    let mut trace = sample_trace();
    trace.events = vec![
        TraceEvent {
            time_ms: 0,
            name: "span_start".to_string(),
            fields: serde_json::Map::from_iter([("span".to_string(), serde_json::json!("root"))]),
        },
        TraceEvent {
            time_ms: 1,
            name: "span_start".to_string(),
            fields: serde_json::Map::from_iter([
                ("span".to_string(), serde_json::json!("db")),
                ("parent_span".to_string(), serde_json::json!("root")),
            ]),
        },
        TraceEvent {
            time_ms: 4,
            name: "http_request".to_string(),
            fields: serde_json::Map::from_iter([
                ("span".to_string(), serde_json::json!("io-1")),
                ("parent_span".to_string(), serde_json::json!("root")),
            ]),
        },
        TraceEvent {
            time_ms: 7,
            name: "span_end".to_string(),
            fields: serde_json::Map::from_iter([("span".to_string(), serde_json::json!("db"))]),
        },
        TraceEvent {
            time_ms: 10,
            name: "span_end".to_string(),
            fields: serde_json::Map::from_iter([("span".to_string(), serde_json::json!("root"))]),
        },
    ];
    let timeline = build_profile_timeline(&trace);
    let latency = build_latency_profile(&trace, &timeline);
    assert!(
        !latency.critical_path.is_empty(),
        "expected critical path edges"
    );
    assert_eq!(latency.critical_path[0].to_span, "root");
    assert_eq!(latency.critical_path[0].reason, "io");
}

#[test]
fn heap_callsite_and_lifetime_histogram_aggregation_is_correct() {
    let mut trace = sample_trace();
    trace.events = vec![
        TraceEvent {
            time_ms: 1,
            name: "memory_alloc".to_string(),
            fields: serde_json::Map::from_iter([
                ("alloc_id".to_string(), serde_json::json!(1)),
                ("bytes".to_string(), serde_json::json!(64)),
                ("callsite_hash".to_string(), serde_json::json!("cs:A")),
            ]),
        },
        TraceEvent {
            time_ms: 2,
            name: "memory_alloc".to_string(),
            fields: serde_json::Map::from_iter([
                ("alloc_id".to_string(), serde_json::json!(2)),
                ("bytes".to_string(), serde_json::json!(32)),
                ("callsite_hash".to_string(), serde_json::json!("cs:A")),
            ]),
        },
        TraceEvent {
            time_ms: 4,
            name: "memory_free".to_string(),
            fields: serde_json::Map::from_iter([("alloc_id".to_string(), serde_json::json!(1))]),
        },
        TraceEvent {
            time_ms: 20,
            name: "memory_free".to_string(),
            fields: serde_json::Map::from_iter([("alloc_id".to_string(), serde_json::json!(2))]),
        },
        TraceEvent {
            time_ms: 30,
            name: "memory_alloc".to_string(),
            fields: serde_json::Map::from_iter([
                ("alloc_id".to_string(), serde_json::json!(3)),
                ("bytes".to_string(), serde_json::json!(16)),
                ("callsite_hash".to_string(), serde_json::json!("cs:B")),
            ]),
        },
    ];
    trace.summary.duration_ms = 30;
    let timeline = build_profile_timeline(&trace);
    let heap = build_heap_profile(&trace, &timeline);
    let cs_a = heap
        .hotspots
        .iter()
        .find(|h| h.callsite_hash == "cs:A")
        .expect("cs:A");
    let cs_b = heap
        .hotspots
        .iter()
        .find(|h| h.callsite_hash == "cs:B")
        .expect("cs:B");
    assert_eq!(cs_a.alloc_count, 2);
    assert_eq!(cs_a.alloc_bytes, 96);
    assert_eq!(cs_a.in_use_bytes, 0);
    assert_eq!(cs_b.alloc_count, 1);
    assert_eq!(cs_b.in_use_bytes, 16);
    assert!(
        heap.lifetime_histogram
            .iter()
            .any(|b| b.bucket == "2-10ms" && b.count == 1)
    );
    assert!(
        heap.lifetime_histogram
            .iter()
            .any(|b| b.bucket == "11-100ms" && b.count == 1)
    );
}

#[test]
fn heap_profile_prefers_effective_alloc_bytes() {
    let mut trace = sample_trace();
    trace.events = vec![
        TraceEvent {
            time_ms: 1,
            name: "memory_alloc".to_string(),
            fields: serde_json::Map::from_iter([
                ("alloc_id".to_string(), serde_json::json!(1)),
                ("bytes".to_string(), serde_json::json!(64)),
                ("effective_bytes".to_string(), serde_json::json!(96)),
                ("callsite_hash".to_string(), serde_json::json!("cs:A")),
            ]),
        },
        TraceEvent {
            time_ms: 2,
            name: "memory_alloc".to_string(),
            fields: serde_json::Map::from_iter([
                ("alloc_id".to_string(), serde_json::json!(2)),
                ("bytes".to_string(), serde_json::json!(32)),
                ("effective_bytes".to_string(), serde_json::json!(48)),
                ("callsite_hash".to_string(), serde_json::json!("cs:A")),
            ]),
        },
    ];
    trace.memory = Some(crate::MemoryTrace {
        options: crate::MemoryOptions {
            track: true,
            artifacts: true,
            ..crate::MemoryOptions::default()
        },
        summary: crate::MemorySummary {
            alloc_count: 2,
            free_count: 0,
            failed_alloc_count: 0,
            in_use_bytes: 144,
            peak_bytes: 144,
            leaked_bytes: 144,
            leaked_allocs: 2,
        },
        leaks: Vec::new(),
        graph: crate::MemoryGraph::default(),
    });
    let timeline = build_profile_timeline(&trace);
    let heap = build_heap_profile(&trace, &timeline);
    let cs_a = heap
        .hotspots
        .iter()
        .find(|h| h.callsite_hash == "cs:A")
        .expect("cs:A");
    assert_eq!(heap.total_alloc_bytes, 144);
    assert_eq!(heap.in_use_bytes, 144);
    assert_eq!(cs_a.alloc_bytes, 144);
    assert_eq!(cs_a.in_use_bytes, 144);
}

#[test]
fn diff_tie_breaking_is_deterministic() {
    let trace = sample_trace();
    let timeline = build_profile_timeline(&trace);
    let cpu = build_cpu_profile(&trace, &timeline);
    let heap = build_heap_profile(&trace, &timeline);
    let latency = build_latency_profile(&trace, &timeline);
    let mut left = build_profile_metrics(&trace, &timeline, &cpu, &heap, &latency);
    let mut right = left.clone();
    left.io_ops = 0;
    left.sched_ops = 0;
    right.io_ops = 1;
    right.sched_ops = 1;
    let diff = compute_diff(
        "left",
        "right",
        &["io".to_string(), "sched".to_string()],
        &left,
        &right,
        None,
        None,
        &HashMap::new(),
        &HashMap::new(),
        1,
        1,
    );
    let metrics = diff
        .regressions
        .iter()
        .map(|r| r.metric.clone())
        .collect::<Vec<_>>();
    assert_eq!(metrics, vec!["io_ops".to_string(), "sched_ops".to_string()]);
}
