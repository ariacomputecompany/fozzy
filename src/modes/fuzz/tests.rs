use super::{
    FuzzTarget, crash_trace_output_path, execute_target, replay_fuzz_trace, with_numeric_suffix,
};
use crate::{
    CURRENT_TRACE_VERSION, Config, MemoryOptions, ProfileCaptureLevel, Reporter, RunIdentity,
    RunMode, RunSummary, TRACE_FORMAT, TraceFile,
};
use std::path::{Path, PathBuf};

fn temp_workspace(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("fozzy-fuzz-test-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp workspace");
    root
}

fn test_config(root: &Path) -> Config {
    Config {
        base_dir: root.join(".fozzy"),
        reporter: Reporter::Json,
        proc_backend: crate::ProcBackend::Scripted,
        fs_backend: crate::FsBackend::Virtual,
        http_backend: crate::HttpBackend::Scripted,
        mem_track: false,
        mem_limit_mb: None,
        mem_fail_after: None,
        fail_on_leak: false,
        leak_budget: None,
        mem_artifacts: false,
        profile_heap_alloc_budget: None,
        profile_heap_in_use_budget: None,
        mem_fragmentation_seed: None,
        mem_pressure_wave: None,
    }
}

fn write_memory_leak_scenario(root: &Path) -> PathBuf {
    let path = root.join("memory.leak.fozzy.json");
    std::fs::write(
        &path,
        r#"{
  "version": 1,
  "name": "memory-leak",
  "steps": [
    { "type": "memory_alloc", "bytes": 256, "key": "leak", "tag": "leak-test" }
  ]
}"#,
    )
    .expect("write scenario");
    path
}

#[test]
fn crash_trace_output_path_uses_base_then_numbered_suffixes() {
    let artifacts_dir = Path::new("/tmp/fozzy-run");
    let first = crash_trace_output_path(None, artifacts_dir, 1);
    let second = crash_trace_output_path(None, artifacts_dir, 2);
    let third = crash_trace_output_path(None, artifacts_dir, 3);
    assert_eq!(first, artifacts_dir.join("trace.fozzy"));
    assert_eq!(second, artifacts_dir.join("trace.1.fozzy"));
    assert_eq!(third, artifacts_dir.join("trace.2.fozzy"));
}

#[test]
fn with_numeric_suffix_handles_paths_without_extension() {
    let out = with_numeric_suffix(Path::new("artifacts/trace"), 4);
    assert_eq!(out, Path::new("artifacts/trace.4"));
}

#[test]
fn fuzz_target_parses_scenario_prefix_and_path_form() {
    let a: FuzzTarget = "scenario:tests/example.fozzy.json".parse().expect("prefix");
    let b: FuzzTarget = "tests/example.fozzy.json".parse().expect("path form");
    assert!(matches!(a, FuzzTarget::Scenario { .. }));
    assert!(matches!(b, FuzzTarget::Scenario { .. }));
}

#[test]
fn scenario_fuzz_target_preserves_structured_memory() {
    let root = temp_workspace("scenario-memory");
    let scenario = write_memory_leak_scenario(&root);
    let cfg = test_config(&root);
    let target = FuzzTarget::Scenario { path: scenario };

    let exec = execute_target(
        &cfg,
        &target,
        &[1, 2, 3],
        &MemoryOptions {
            track: false,
            artifacts: false,
            ..MemoryOptions::default()
        },
    )
    .expect("execute target");

    assert_eq!(exec.status, crate::ExitStatus::Fail);
    assert_eq!(
        exec.memory
            .as_ref()
            .map(|memory| memory.summary.leaked_bytes),
        Some(256)
    );
}

#[test]
fn replay_fuzz_trace_uses_replayed_memory_summary() {
    let root = temp_workspace("scenario-replay");
    let scenario = write_memory_leak_scenario(&root);
    let cfg = test_config(&root);
    let target = FuzzTarget::Scenario {
        path: scenario.clone(),
    };
    let exec = execute_target(
        &cfg,
        &target,
        &[7],
        &MemoryOptions {
            track: false,
            artifacts: false,
            ..MemoryOptions::default()
        },
    )
    .expect("execute target");
    let trace_path = root.join("trace.fozzy");
    let trace = TraceFile {
        format: TRACE_FORMAT.to_string(),
        version: CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Fuzz,
        scenario_path: None,
        scenario: None,
        fuzz: Some(crate::FuzzTrace {
            target: format!("scenario:{}", scenario.display()),
            input_hex: "07".to_string(),
        }),
        explore: None,
        memory: exec.memory.clone(),
        decisions: Vec::new(),
        events: exec.events.clone(),
        summary: RunSummary {
            status: exec.status,
            mode: RunMode::Fuzz,
            identity: RunIdentity {
                run_id: "r1".to_string(),
                seed: 1,
                trace_path: Some(trace_path.to_string_lossy().to_string()),
                report_path: None,
                artifacts_dir: None,
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: exec.memory.as_ref().map(|memory| memory.summary.clone()),
            findings: exec.findings.clone(),
        },
        checksum: None,
    };
    trace.write_json(&trace_path).expect("write trace");
    let replayed = replay_fuzz_trace(
        &cfg,
        &trace,
        &trace_path,
        &crate::ReplayOptions {
            until: None,
            step: false,
            dump_events: false,
            profile_capture: ProfileCaptureLevel::Baseline,
            reporter: Reporter::Json,
        },
    )
    .expect("replay fuzz trace");
    assert_eq!(
        replayed
            .summary
            .memory
            .as_ref()
            .map(|memory| memory.leaked_bytes),
        Some(256)
    );
}
