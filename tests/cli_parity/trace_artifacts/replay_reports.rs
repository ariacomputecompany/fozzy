use super::*;

#[test]
fn replay_emits_requested_html_report_artifact() {
    let ws = temp_workspace("replay-reporter-html");
    let scenario = ws.join("example.fozzy.json");
    std::fs::write(&scenario, fixture("example.fozzy.json")).expect("write scenario");
    let trace = ws.join("trace.fozzy");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--record".into(),
            trace.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(run.status.code(), Some(0), "run should succeed");

    let replay = run_cli_in(
        &ws,
        &[
            "replay".into(),
            trace.display().to_string(),
            "--reporter".into(),
            "html".into(),
            "--json".into(),
        ],
    );
    assert_eq!(replay.status.code(), Some(0), "replay should succeed");
    let out = parse_json_stdout(&replay);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    assert!(
        artifacts_dir.join("report.html").exists(),
        "replay should emit report.html when reporter=html"
    );
}

#[test]
fn replay_fuzz_emits_requested_html_report_artifact() {
    let ws = temp_workspace("replay-fuzz-reporter-html");
    let trace = ws.join("fuzz.trace.fozzy");

    let fuzz = run_cli_in(
        &ws,
        &[
            "fuzz".into(),
            format!(
                "scenario:{}",
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/memory.pass.fozzy.json")
                    .display()
            ),
            "--det".into(),
            "--runs".into(),
            "1".into(),
            "--seed".into(),
            "7".into(),
            "--record".into(),
            trace.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(fuzz.status.code(), Some(0), "fuzz should succeed");

    let replay = run_cli_in(
        &ws,
        &[
            "replay".into(),
            trace.display().to_string(),
            "--reporter".into(),
            "html".into(),
            "--json".into(),
        ],
    );
    assert_eq!(replay.status.code(), Some(0), "replay should succeed");
    let out = parse_json_stdout(&replay);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    assert!(
        artifacts_dir.join("report.html").exists(),
        "fuzz replay should emit report.html when reporter=html"
    );
}

#[test]
fn replay_explore_emits_requested_html_report_artifact() {
    let ws = temp_workspace("replay-explore-reporter-html");
    let trace = ws.join("explore.trace.fozzy");

    let explore = run_cli_in(
        &ws,
        &[
            "explore".into(),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/kv.explore.fozzy.json")
                .display()
                .to_string(),
            "--steps".into(),
            "10".into(),
            "--seed".into(),
            "7".into(),
            "--record".into(),
            trace.display().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(explore.status.code(), Some(0), "explore should succeed");

    let replay = run_cli_in(
        &ws,
        &[
            "replay".into(),
            trace.display().to_string(),
            "--reporter".into(),
            "html".into(),
            "--json".into(),
        ],
    );
    assert_eq!(replay.status.code(), Some(0), "replay should succeed");
    let out = parse_json_stdout(&replay);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    assert!(
        artifacts_dir.join("report.html").exists(),
        "explore replay should emit report.html when reporter=html"
    );
}
