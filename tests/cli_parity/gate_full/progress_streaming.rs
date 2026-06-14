use super::*;

fn write_basic_step_scenario(path: &Path) {
    std::fs::write(
        path,
        r#"{
  "version": 1,
  "name": "full-progress-pass",
  "steps": [
    { "type": "trace_event", "name": "start" },
    { "type": "sleep", "duration": "1ms" }
  ]
}"#,
    )
    .expect("write step scenario");
}

fn init_git_repo(path: &Path) {
    Command::new("git")
        .current_dir(path)
        .args(["init"])
        .output()
        .expect("git init");
    Command::new("git")
        .current_dir(path)
        .args(["config", "user.email", "codex@example.com"])
        .output()
        .expect("git config email");
    Command::new("git")
        .current_dir(path)
        .args(["config", "user.name", "Codex"])
        .output()
        .expect("git config name");
}

#[test]
fn full_json_streams_progress_events_to_stderr() {
    let ws = temp_workspace("full-progress-events");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    write_basic_step_scenario(&scenario_root.join("example.fozzy.json"));

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "full",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--seed",
            "7",
            "--doctor-runs",
            "2",
            "--fuzz-time",
            "10ms",
            "--json",
        ])
        .output()
        .expect("run full");

    let stderr_docs = parse_json_stderr_docs(&out);
    assert!(
        stderr_docs.iter().any(|doc| {
            doc.get("schemaVersion").and_then(|v| v.as_str()) == Some("fozzy.full_progress.v1")
                && doc.get("event").and_then(|v| v.as_str()) == Some("phase_started")
                && doc.get("step").and_then(|v| v.as_str()) == Some("prepare")
        }),
        "expected prepare phase progress event, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stderr_docs.iter().any(|doc| {
            doc.get("schemaVersion").and_then(|v| v.as_str()) == Some("fozzy.full_progress.v1")
                && doc.get("event").and_then(|v| v.as_str()) == Some("step_started")
                && doc.get("step").and_then(|v| v.as_str()) == Some("discover_scenarios")
        }),
        "expected discover_scenarios start event, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stderr_docs.iter().any(|doc| {
            doc.get("schemaVersion").and_then(|v| v.as_str()) == Some("fozzy.full_progress.v1")
                && doc.get("event").and_then(|v| v.as_str()) == Some("step_finished")
                && doc.get("step").and_then(|v| v.as_str()) == Some("discover_scenarios")
        }),
        "expected discover_scenarios completion event, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let final_doc = parse_json_stdout(&out);
    assert_eq!(
        final_doc.get("schemaVersion").and_then(|v| v.as_str()),
        Some("fozzy.full_report.v1")
    );
}

#[test]
fn full_uses_isolated_temp_fuzz_corpus_instead_of_workspace_default() {
    let ws = temp_workspace("full-isolated-fuzz-corpus");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    write_basic_step_scenario(&scenario_root.join("example.fozzy.json"));

    let poison_dir = ws.join(".fozzy/corpora/default");
    std::fs::create_dir_all(&poison_dir).expect("create poison corpus dir");
    std::fs::write(
        &poison_dir.join("poison.bin"),
        b"workspace-corpus-should-not-be-used",
    )
    .expect("write poison corpus file");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "full",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--seed",
            "7",
            "--doctor-runs",
            "2",
            "--fuzz-time",
            "10ms",
            "--json",
        ])
        .output()
        .expect("run full");

    let doc = parse_json_stdout(&out);
    let fuzz_detail = full_step_detail(&doc, "fuzz").unwrap_or_default();
    assert!(
        !fuzz_detail.contains("workspace-corpus-should-not-be-used")
            && !fuzz_detail.contains(".fozzy/corpora/default"),
        "fuzz detail should not reference workspace default corpus: {fuzz_detail}"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("fozzy.full_progress.v1"),
        "stderr should contain progress events for visibility"
    );
}

#[test]
fn full_without_explicit_seed_does_not_fail_on_seed_mismatch() {
    let ws = temp_workspace("full-default-seed");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    write_basic_step_scenario(&scenario_root.join("example.fozzy.json"));

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "full",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--doctor-runs",
            "2",
            "--fuzz-time",
            "10ms",
            "--required-steps",
            "doctor_deep,test_det,run_record_trace,replay,fuzz",
            "--json",
        ])
        .output()
        .expect("run full");

    let doc = parse_json_stdout(&out);
    for step in [
        "doctor_deep",
        "test_det",
        "run_record_trace",
        "replay",
        "fuzz",
    ] {
        assert_ne!(
            full_step_status(&doc, step).as_deref(),
            Some("failed"),
            "{step} should not fail under default workflow seed: {}",
            full_step_detail(&doc, step).unwrap_or_default()
        );
        assert!(
            !full_step_detail(&doc, step)
                .unwrap_or_default()
                .contains("seed_matches=false"),
            "{step} should not report seed mismatch"
        );
    }
}

#[test]
fn gate_without_explicit_seed_does_not_fail_on_seed_mismatch() {
    let ws = temp_workspace("gate-default-seed");
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    write_basic_step_scenario(&scenario_root.join("example.fozzy.json"));

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");

    let doc = parse_json_stdout(&out);
    for step in [
        "doctor_deep",
        "test_det_strict",
        "run_record_trace",
        "replay",
    ] {
        assert_ne!(
            full_step_status(&doc, step).as_deref(),
            Some("failed"),
            "{step} should not fail under default workflow seed: {}",
            full_step_detail(&doc, step).unwrap_or_default()
        );
        assert!(
            !full_step_detail(&doc, step)
                .unwrap_or_default()
                .contains("seed_matches=false"),
            "{step} should not report seed mismatch"
        );
    }
}

#[test]
fn full_dirty_git_repo_surfaces_advisory_not_failure() {
    let ws = temp_workspace("full-dirty-git-advisory");
    init_git_repo(&ws);
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    write_basic_step_scenario(&scenario_root.join("example.fozzy.json"));
    std::fs::write(ws.join("scratch.txt"), b"dirty").expect("write dirty file");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "full",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--doctor-runs",
            "2",
            "--fuzz-time",
            "10ms",
            "--required-steps",
            "clean_tree,doctor_deep,test_det,run_record_trace,replay,fuzz",
            "--json",
        ])
        .output()
        .expect("run full");

    assert_eq!(
        out.status.code(),
        Some(0),
        "dirty worktree should be advisory only, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "clean_tree").as_deref(),
        Some("advisory")
    );
    assert!(
        full_step_detail(&doc, "clean_tree")
            .unwrap_or_default()
            .contains("advisory only")
    );
}

#[test]
fn gate_dirty_git_repo_surfaces_advisory_not_failure() {
    let ws = temp_workspace("gate-dirty-git-advisory");
    init_git_repo(&ws);
    let scenario_root = ws.join("tests");
    std::fs::create_dir_all(&scenario_root).expect("create tests dir");
    write_basic_step_scenario(&scenario_root.join("example.fozzy.json"));
    std::fs::write(ws.join("scratch.txt"), b"dirty").expect("write dirty file");

    let out = Command::new(env!("CARGO_BIN_EXE_fozzy"))
        .args([
            "--cwd",
            ws.to_str().expect("ws str"),
            "gate",
            "--profile",
            "targeted",
            "--scenario-root",
            scenario_root.to_str().expect("tests str"),
            "--doctor-runs",
            "2",
            "--json",
        ])
        .output()
        .expect("run gate");

    assert_eq!(
        out.status.code(),
        Some(0),
        "dirty worktree should be advisory only"
    );
    let doc = parse_json_stdout(&out);
    assert_eq!(
        full_step_status(&doc, "clean_tree").as_deref(),
        Some("advisory")
    );
}
