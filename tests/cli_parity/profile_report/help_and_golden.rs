use super::*;

#[test]
fn profile_help_uses_run_or_trace_value_name() {
    for sub in ["top", "flame", "timeline", "export", "shrink", "doctor"] {
        let out = run_cli(&["profile".into(), sub.to_string(), "--help".into()]);
        assert_eq!(out.status.code(), Some(0), "help should exit 0");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("RUN_OR_TRACE"),
            "help should show RUN_OR_TRACE for profile {sub}; got: {stdout}"
        );
        assert!(
            stdout.contains("latest|last-pass|last-fail")
                || stdout.contains("latest, last-pass, last-fail"),
            "help should describe aliases for profile {sub}; got: {stdout}"
        );
    }
}

#[test]
fn profile_golden_run_top_flame_timeline_export_flow() {
    let ws = temp_workspace("profile-golden-flow");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("write scenario");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let trace = ws.join("golden.trace.fozzy");

    let run = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.to_string_lossy().to_string(),
            "--det".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--record".into(),
            trace.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "run stderr={}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(trace.exists(), "expected recorded trace");

    let top = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "top".into(),
            trace.to_string_lossy().to_string(),
            "--heap".into(),
            "--latency".into(),
            "--io".into(),
            "--sched".into(),
            "--limit".into(),
            "10".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        top.status.code(),
        Some(0),
        "profile top stderr={}",
        String::from_utf8_lossy(&top.stderr)
    );
    let top_doc = parse_json_stdout(&top);
    assert_eq!(
        top_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_top.v1"
    );
    assert!(top_doc.get("heap").is_some());
    assert!(top_doc.get("latency").is_some());

    let folded_out = ws.join("heap.folded.txt");
    let flame = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "flame".into(),
            trace.to_string_lossy().to_string(),
            "--heap".into(),
            "--format".into(),
            "folded".into(),
            "--out".into(),
            folded_out.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        flame.status.code(),
        Some(0),
        "profile flame stderr={}",
        String::from_utf8_lossy(&flame.stderr)
    );
    assert!(folded_out.exists(), "folded output must exist");
    assert!(
        std::fs::metadata(&folded_out)
            .map(|m| m.len() > 0)
            .unwrap_or(false),
        "folded output should be non-empty"
    );

    let timeline_out = ws.join("timeline.json");
    let timeline = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "timeline".into(),
            trace.to_string_lossy().to_string(),
            "--format".into(),
            "json".into(),
            "--out".into(),
            timeline_out.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        timeline.status.code(),
        Some(0),
        "profile timeline stderr={}",
        String::from_utf8_lossy(&timeline.stderr)
    );
    assert!(timeline_out.exists(), "timeline output must exist");

    let export_out = ws.join("profile.otlp.json");
    let export = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "export".into(),
            trace.to_string_lossy().to_string(),
            "--format".into(),
            "otlp".into(),
            "--out".into(),
            export_out.to_string_lossy().to_string(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        export.status.code(),
        Some(0),
        "profile export stderr={}",
        String::from_utf8_lossy(&export.stderr)
    );
    assert!(export_out.exists(), "profile export output must exist");
}
