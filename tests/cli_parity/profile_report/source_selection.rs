use super::*;

#[test]
fn profile_direct_trace_prefers_declared_artifacts_dir_over_profile_cache() {
    let ws = temp_workspace("profile-direct-artifacts-dir");
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
    let out_trace = shrink_doc
        .get("outTrace")
        .and_then(|v| v.as_str())
        .expect("out trace");

    let top = run_cli_in(
        &ws,
        &[
            "profile".into(),
            "top".into(),
            out_trace.into(),
            "--heap".into(),
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

    let cache_dir = ws.join(".fozzy").join("profile-cache");
    let cache_entries = if cache_dir.exists() {
        std::fs::read_dir(&cache_dir)
            .expect("read cache dir")
            .filter_map(Result::ok)
            .count()
    } else {
        0
    };
    assert_eq!(
        cache_entries, 0,
        "direct trace with declared artifacts dir should not synthesize duplicate profile-cache entries"
    );
}
