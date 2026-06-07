use super::super::*;
#[allow(unused_imports)]
use crate::{
    ExitStatus, MemoryGraphNode, MemoryOptions, MemorySummary, RunIdentity, RunMode, RunSummary,
};
#[allow(unused_imports)]
use std::path::Path;

#[test]
fn top_sorts_descending_by_bytes() {
    let mut leaks = vec![
        MemoryLeak {
            alloc_id: 1,
            bytes: 10,
            callsite_hash: "a".to_string(),
            tag: None,
        },
        MemoryLeak {
            alloc_id: 2,
            bytes: 50,
            callsite_hash: "b".to_string(),
            tag: None,
        },
        MemoryLeak {
            alloc_id: 3,
            bytes: 20,
            callsite_hash: "c".to_string(),
            tag: None,
        },
    ];
    leaks.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    assert_eq!(leaks[0].bytes, 50);
    assert_eq!(leaks[1].bytes, 20);
    assert_eq!(leaks[2].bytes, 10);
}

#[test]
fn memory_diff_from_trace_inputs() {
    let root = std::env::temp_dir().join(format!("fozzy-memory-cmd-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("mkdir");
    let mk_trace = |path: &Path, leaked: u64| {
        let trace = crate::TraceFile {
            format: crate::TRACE_FORMAT.to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(crate::ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            fuzz: None,
            explore: None,
            memory: Some(crate::MemoryTrace {
                options: MemoryOptions::default(),
                summary: MemorySummary {
                    leaked_bytes: leaked,
                    leaked_allocs: if leaked > 0 { 1 } else { 0 },
                    ..MemorySummary::default()
                },
                leaks: Vec::new(),
                graph: MemoryGraph::default(),
            }),
            decisions: Vec::new(),
            events: Vec::new(),
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 1,
                    trace_path: None,
                    report_path: None,
                    artifacts_dir: None,
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 0,
                duration_ns: 0,
                tests: None,
                memory: None,
                findings: Vec::new(),
            },
            checksum: None,
        };
        trace.write_json(path).expect("write trace");
    };
    let left = root.join("left.fozzy");
    let right = root.join("right.fozzy");
    mk_trace(&left, 10);
    mk_trace(&right, 30);

    let cfg = Config::default();
    let out = memory_command(
        &cfg,
        &MemoryCommand::Diff {
            left: left.display().to_string(),
            right: right.display().to_string(),
        },
    )
    .expect("diff");
    let obj = out.as_object().expect("object");
    assert_eq!(
        obj.get("deltaLeakedBytes").and_then(|v| v.as_i64()),
        Some(20)
    );
}
