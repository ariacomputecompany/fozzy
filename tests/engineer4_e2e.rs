use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

fn temp_workspace(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("fozzy-engineer4-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp workspace");
    root
}

fn fixture(name: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(root.join("tests").join(name)).expect("read fixture")
}

fn run_cli(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args(args)
        .output()
        .expect("run cli")
}

fn run_cli_in(cwd: &Path, args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run cli in cwd")
}

fn parse_json_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout json")
}

fn json_message(output: &Output) -> String {
    parse_json_stdout(output)
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn write_config(ws: &Path) -> PathBuf {
    let config = ws.join("fozzy.toml");
    std::fs::write(&config, "base_dir = \".fozzy\"\n").expect("write config");
    config
}

fn prepare_scenario(ws: &Path, name: &str) -> PathBuf {
    let path = ws.join(name);
    std::fs::write(&path, fixture(name)).expect("write scenario");
    path
}

fn run_with_memory_artifacts(ws: &Path, scenario_name: &str, trace_name: &str) -> (PathBuf, Value) {
    let config = write_config(ws);
    let scenario = prepare_scenario(ws, scenario_name);
    let trace_path = ws.join(trace_name);
    let output = run_cli(&[
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--seed".into(),
        "17".into(),
        "--mem-track".into(),
        "--mem-artifacts".into(),
        "--record".into(),
        trace_path.to_string_lossy().to_string(),
        "--json".into(),
        "--cwd".into(),
        ws.to_string_lossy().to_string(),
        "--config".into(),
        config.to_string_lossy().to_string(),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "run stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    (trace_path, parse_json_stdout(&output))
}

fn report_show_json(ws: &Path, selector: &str) -> Value {
    let output = run_cli_in(
        ws,
        &[
            "--config".into(),
            ws.join("fozzy.toml").to_string_lossy().to_string(),
            "--cwd".into(),
            ws.to_string_lossy().to_string(),
            "--json".into(),
            "report".into(),
            "show".into(),
            selector.into(),
            "--format".into(),
            "json".into(),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "report show {selector} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_json_stdout(&output)
}

fn memory_top_json(ws: &Path, selector: &str) -> Value {
    let output = run_cli_in(
        ws,
        &[
            "--config".into(),
            ws.join("fozzy.toml").to_string_lossy().to_string(),
            "--cwd".into(),
            ws.to_string_lossy().to_string(),
            "--json".into(),
            "memory".into(),
            "top".into(),
            selector.into(),
        ],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "memory top {selector} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_json_stdout(&output)
}

fn comparable_memory_top(mut value: Value) -> Value {
    if let Some(obj) = value.as_object_mut() {
        obj.remove("run");
    }
    value
}

#[test]
fn selector_parity_matrix_keeps_report_and_memory_views_in_sync() {
    let ws = temp_workspace("selector-parity");
    let (trace_path, run) =
        run_with_memory_artifacts(&ws, "memory.pass.fozzy.json", "memory.fozzy");
    let run_id = run["identity"]["runId"].as_str().expect("run id");
    let relative_trace = trace_path
        .strip_prefix(&ws)
        .expect("trace under workspace")
        .to_string_lossy()
        .to_string();

    let selectors = [
        run_id.to_string(),
        "latest".to_string(),
        trace_path.to_string_lossy().to_string(),
        relative_trace,
    ];
    let baseline_report = report_show_json(&ws, &selectors[0]);
    let baseline_memory = memory_top_json(&ws, &selectors[0]);

    for selector in selectors {
        assert_eq!(
            report_show_json(&ws, &selector),
            baseline_report,
            "report parity failed for selector {selector}"
        );
        assert_eq!(
            comparable_memory_top(memory_top_json(&ws, &selector)),
            comparable_memory_top(baseline_memory.clone()),
            "memory parity failed for selector {selector}"
        );
    }

    let verify_abs = run_cli_in(
        &ws,
        &[
            "--config".into(),
            ws.join("fozzy.toml").to_string_lossy().to_string(),
            "--cwd".into(),
            ws.to_string_lossy().to_string(),
            "--json".into(),
            "trace".into(),
            "verify".into(),
            trace_path.to_string_lossy().to_string(),
        ],
    );
    let verify_rel = run_cli_in(
        &ws,
        &[
            "--config".into(),
            ws.join("fozzy.toml").to_string_lossy().to_string(),
            "--cwd".into(),
            ws.to_string_lossy().to_string(),
            "--json".into(),
            "trace".into(),
            "verify".into(),
            "memory.fozzy".into(),
        ],
    );
    assert_eq!(verify_abs.status.code(), Some(0));
    assert_eq!(verify_rel.status.code(), Some(0));
    assert_eq!(
        parse_json_stdout(&verify_abs),
        parse_json_stdout(&verify_rel),
        "trace selectors should verify identically"
    );
}

#[test]
fn stale_memory_sidecar_is_detected_without_poisoning_report_paths() {
    let ws = temp_workspace("stale-sidecar");
    let (trace_path, run) =
        run_with_memory_artifacts(&ws, "memory.pass.fozzy.json", "memory.fozzy");
    let run_id = run["identity"]["runId"]
        .as_str()
        .expect("run id")
        .to_string();
    let artifacts_dir = ws.join(
        run["identity"]["artifactsDir"]
            .as_str()
            .expect("artifacts dir"),
    );
    let report_before = report_show_json(&ws, "latest");
    let leaks_path = artifacts_dir.join("memory.leaks.json");
    let mut leaks: Value =
        serde_json::from_slice(&std::fs::read(&leaks_path).expect("read memory leaks"))
            .expect("parse memory leaks");
    match leaks.as_array_mut() {
        Some(items) => {
            items.push(json!({
                "allocId": 999,
                "bytes": 1,
                "callsiteHash": "stale-sidecar"
            }));
        }
        None => panic!("memory.leaks.json should be an array"),
    }
    std::fs::write(
        &leaks_path,
        serde_json::to_vec_pretty(&leaks).expect("encode memory leaks"),
    )
    .expect("write stale leaks");

    for selector in [
        run_id.as_str(),
        "latest",
        trace_path.to_string_lossy().as_ref(),
        "memory.fozzy",
    ] {
        let output = run_cli_in(
            &ws,
            &[
                "--config".into(),
                ws.join("fozzy.toml").to_string_lossy().to_string(),
                "--cwd".into(),
                ws.to_string_lossy().to_string(),
                "--json".into(),
                "memory".into(),
                "top".into(),
                selector.into(),
            ],
        );
        assert_ne!(
            output.status.code(),
            Some(0),
            "stale sidecar should fail for {selector}"
        );
        let message = json_message(&output);
        assert!(
            message.contains("sidecar") || message.contains("memory leak"),
            "stale sidecar error should stay explicit for {selector}; got: {message}"
        );
    }

    let report_after = report_show_json(&ws, "latest");
    assert_eq!(
        report_after["identity"]["runId"], report_before["identity"]["runId"],
        "report resolution should remain stable even when memory sidecars go stale"
    );
}

#[test]
fn report_and_trace_timing_fields_match_across_wrapper_and_direct_trace_selectors() {
    let ws = temp_workspace("timing-parity");
    let config = write_config(&ws);
    let scenario = prepare_scenario(&ws, "example.fozzy.json");
    let trace_path = ws.join("timing.fozzy");
    let output = run_cli(&[
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--seed".into(),
        "7".into(),
        "--record".into(),
        trace_path.to_string_lossy().to_string(),
        "--json".into(),
        "--cwd".into(),
        ws.to_string_lossy().to_string(),
        "--config".into(),
        config.to_string_lossy().to_string(),
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "run stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = parse_json_stdout(&output);
    let trace: Value = serde_json::from_slice(&std::fs::read(&trace_path).expect("read trace"))
        .expect("trace json");
    let trace_summary = trace.get("summary").expect("trace summary");
    let selectors = [
        run["identity"]["runId"]
            .as_str()
            .expect("run id")
            .to_string(),
        "latest".to_string(),
        trace_path.to_string_lossy().to_string(),
        "timing.fozzy".to_string(),
    ];

    for selector in selectors {
        let report = report_show_json(&ws, &selector);
        for field in ["startedAt", "finishedAt", "durationMs", "durationNs"] {
            assert_eq!(
                report.get(field),
                trace_summary.get(field),
                "report timing field {field} should match trace summary for selector {selector}"
            );
        }
    }
}
