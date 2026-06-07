use super::*;

#[test]
fn profile_shrink_trace_resolves_detached_artifact_directory() {
    let ws = temp_workspace("profile-shrink-artifacts");
    let cfg = ws.join("fozzy.toml");
    std::fs::write(&cfg, "base_dir = \".fozzy\"\n").expect("write config");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("scenario");
    let trace = ws.join("record.trace.fozzy");

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

    let shrink = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "shrink".into(),
            trace.to_string_lossy().to_string(),
            "--metric".into(),
            "alloc_bytes".into(),
            "--direction".into(),
            "increase".into(),
            "--minimize".into(),
            "all".into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        shrink.status.code(),
        Some(0),
        "profile shrink stderr={}",
        String::from_utf8_lossy(&shrink.stderr)
    );
    let shrink_doc = parse_json_stdout(&shrink);
    assert_eq!(
        shrink_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.profile_shrink.v2"
    );
    let out_trace = shrink_doc
        .get("outTrace")
        .and_then(|v| v.as_str())
        .expect("out trace");
    let artifacts_dir = shrink_doc
        .get("artifactsDir")
        .and_then(|v| v.as_str())
        .expect("artifacts dir");
    assert_ne!(
        Path::new(out_trace).parent().expect("trace parent"),
        Path::new(artifacts_dir)
    );

    let ls = run_cli_in(
        &ws,
        &[
            "artifacts".into(),
            "ls".into(),
            out_trace.into(),
            "--config".into(),
            cfg.to_string_lossy().to_string(),
            "--json".into(),
        ],
    );
    assert_eq!(
        ls.status.code(),
        Some(0),
        "artifacts ls stderr={}",
        String::from_utf8_lossy(&ls.stderr)
    );
    let ls_doc = parse_json_stdout(&ls);
    let entries = ls_doc
        .get("entries")
        .and_then(|v| v.as_array())
        .expect("entries");
    assert!(entries.iter().any(|entry| {
        entry.get("path").and_then(|v| v.as_str())
            == Some(&format!("{artifacts_dir}/profile.metrics.json"))
    }));
}
