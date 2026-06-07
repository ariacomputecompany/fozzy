use super::*;

#[test]
fn run_manifest_refreshes_profile_capabilities_after_artifact_emit() {
    let ws = temp_workspace("run-manifest-profile-capabilities");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("write scenario");

    let output = run_cli_in(
        &ws,
        &[
            "run".into(),
            scenario.display().to_string(),
            "--det".into(),
            "--seed".into(),
            "7".into(),
            "--profile-capture".into(),
            "full".into(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "run should succeed");
    let out = parse_json_stdout(&output);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts_dir.join("manifest.json")).expect("read manifest"),
    )
    .expect("manifest json");
    let caps = manifest
        .get("profileCapabilities")
        .and_then(|v| v.as_array())
        .expect("profile capabilities");
    assert!(
        caps.iter().any(|v| v.as_str() == Some("metrics")),
        "manifest should include metrics capability after profile artifact emit"
    );
}

#[test]
fn fuzz_manifest_refreshes_profile_capabilities_after_artifact_emit() {
    let ws = temp_workspace("fuzz-manifest-profile-capabilities");
    let output = run_cli_in(
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
            "--profile-capture".into(),
            "full".into(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "fuzz should succeed");
    let out = parse_json_stdout(&output);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts_dir.join("manifest.json")).expect("read manifest"),
    )
    .expect("manifest json");
    let caps = manifest
        .get("profileCapabilities")
        .and_then(|v| v.as_array())
        .expect("profile capabilities");
    assert!(
        caps.iter().any(|v| v.as_str() == Some("metrics")),
        "manifest should include metrics capability after profile artifact emit"
    );
}

#[test]
fn explore_manifest_refreshes_profile_capabilities_after_artifact_emit() {
    let ws = temp_workspace("explore-manifest-profile-capabilities");
    let output = run_cli_in(
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
            "--profile-capture".into(),
            "full".into(),
            "--json".into(),
        ],
    );
    assert_eq!(output.status.code(), Some(0), "explore should succeed");
    let out = parse_json_stdout(&output);
    let artifacts_dir = resolve_identity_artifacts_dir(&ws, &out);
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(artifacts_dir.join("manifest.json")).expect("read manifest"),
    )
    .expect("manifest json");
    let caps = manifest
        .get("profileCapabilities")
        .and_then(|v| v.as_array())
        .expect("profile capabilities");
    assert!(
        caps.iter().any(|v| v.as_str() == Some("metrics")),
        "manifest should include metrics capability after profile artifact emit"
    );
}
