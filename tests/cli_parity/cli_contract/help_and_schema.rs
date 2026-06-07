use super::*;

#[test]
fn artifacts_help_uses_run_or_trace_value_name() {
    for sub in ["pack", "export"] {
        let out = run_cli(&["artifacts".into(), sub.to_string(), "--help".into()]);
        assert_eq!(out.status.code(), Some(0), "help should exit 0");
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("RUN_OR_TRACE"),
            "help should show RUN_OR_TRACE for artifacts {sub}; got: {stdout}"
        );
    }
}

#[test]
fn steps_alias_matches_schema_output() {
    let schema = run_cli(&["schema".into(), "--json".into()]);
    assert_eq!(
        schema.status.code(),
        Some(0),
        "schema stderr={}",
        String::from_utf8_lossy(&schema.stderr)
    );
    let steps = run_cli(&["steps".into(), "--json".into()]);
    assert_eq!(
        steps.status.code(),
        Some(0),
        "steps alias stderr={}",
        String::from_utf8_lossy(&steps.stderr)
    );
    assert_eq!(parse_json_stdout(&schema), parse_json_stdout(&steps));
}

#[test]
fn map_hotspots_services_and_suites_emit_expected_schema() {
    let ws = temp_workspace("map-schema");
    let services_dir = ws.join("services").join("payments");
    let tests_dir = ws.join("tests");
    std::fs::create_dir_all(&services_dir).expect("services dir");
    std::fs::create_dir_all(&tests_dir).expect("tests dir");
    std::fs::write(
        services_dir.join("handler.rs"),
        r#"
        async fn handle_payment() {
            if retry { tokio::spawn(async move {}); }
            let _ = std::fs::read("config.toml");
            if timeout { panic!("failed"); }
        }
        "#,
    )
    .expect("write source");
    std::fs::write(
        tests_dir.join("handler.fozzy.json"),
        r#"{"version":1,"name":"handler","steps":[{"type":"trace_event","name":"x"}]}"#,
    )
    .expect("write scenario");

    let root = ws.to_string_lossy().to_string();
    let scenario_root = tests_dir.to_string_lossy().to_string();

    let hotspots = run_cli(&[
        "map".into(),
        "hotspots".into(),
        "--root".into(),
        root.clone(),
        "--min-risk".into(),
        "1".into(),
        "--limit".into(),
        "20".into(),
        "--json".into(),
    ]);
    assert_eq!(
        hotspots.status.code(),
        Some(0),
        "map hotspots stderr={}",
        String::from_utf8_lossy(&hotspots.stderr)
    );
    let hotspots_doc = parse_json_stdout(&hotspots);
    assert_eq!(
        hotspots_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.map_hotspots.v2"
    );
    assert!(
        hotspots_doc
            .get("hotspots")
            .and_then(|v| v.as_array())
            .is_some_and(|v| !v.is_empty())
    );

    let services = run_cli(&[
        "map".into(),
        "services".into(),
        "--root".into(),
        root.clone(),
        "--json".into(),
    ]);
    assert_eq!(
        services.status.code(),
        Some(0),
        "map services stderr={}",
        String::from_utf8_lossy(&services.stderr)
    );
    let services_doc = parse_json_stdout(&services);
    assert_eq!(
        services_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.map_services.v2"
    );

    let suites = run_cli(&[
        "map".into(),
        "suites".into(),
        "--root".into(),
        root,
        "--scenario-root".into(),
        scenario_root,
        "--min-risk".into(),
        "1".into(),
        "--json".into(),
    ]);
    assert_eq!(
        suites.status.code(),
        Some(0),
        "map suites stderr={}",
        String::from_utf8_lossy(&suites.stderr)
    );
    let suites_doc = parse_json_stdout(&suites);
    assert_eq!(
        suites_doc
            .get("schemaVersion")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "fozzy.map_suites.v5"
    );
    assert!(
        suites_doc
            .get("suites")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|s| s.get("coverageEvidence"))
            .is_some(),
        "map suites should emit explainable coverage evidence"
    );
    assert_eq!(
        suites_doc
            .get("profile")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "pedantic"
    );
    assert_eq!(
        suites_doc
            .get("shrinkPolicy")
            .and_then(|v| v.as_str())
            .unwrap_or_default(),
        "no_known_failures"
    );
    assert!(
        suites_doc
            .get("requiredHotspotCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= suites_doc
                .get("coveredHotspotCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
    );
}
