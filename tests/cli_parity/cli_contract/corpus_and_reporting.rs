use super::*;

#[test]
fn corpus_import_rejects_raw_duplicate_entries_in_strict_and_non_strict() {
    let ws = temp_workspace("corpus-dup-raw");
    let zip = ws.join("dup.zip");
    let out = ws.join("out");
    std::fs::create_dir_all(&out).expect("out");
    std::fs::write(
        &zip,
        build_zip_with_raw_entries(&[(b"same.txt", b"A"), (b"same.txt", b"B")]),
    )
    .expect("zip");

    for strict in [false, true] {
        let mut args = vec![
            "corpus".into(),
            "import".into(),
            zip.to_string_lossy().to_string(),
            "--out".into(),
            out.to_string_lossy().to_string(),
            "--json".into(),
        ];
        if strict {
            args.insert(0, "--strict".into());
        }
        let outp = run_cli(&args);
        assert_eq!(outp.status.code(), Some(2), "duplicate import must fail");
        let doc = parse_json_stdout(&outp);
        assert_eq!(doc.get("code").and_then(|v| v.as_str()), Some("error"));
        assert!(
            doc.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .contains("duplicate output file in archive is not allowed")
        );
    }
}

#[test]
fn corpus_import_rejects_raw_nul_collision_in_strict_and_non_strict() {
    let ws = temp_workspace("corpus-nul-raw");
    let zip = ws.join("nuldup.zip");
    let out = ws.join("out");
    std::fs::create_dir_all(&out).expect("out");
    std::fs::write(
        &zip,
        build_zip_with_raw_entries(&[(b"bad\0a.txt", b"A"), (b"bad", b"B")]),
    )
    .expect("zip");

    for strict in [false, true] {
        let mut args = vec![
            "corpus".into(),
            "import".into(),
            zip.to_string_lossy().to_string(),
            "--out".into(),
            out.to_string_lossy().to_string(),
            "--json".into(),
        ];
        if strict {
            args.insert(0, "--strict".into());
        }
        let outp = run_cli(&args);
        assert_eq!(
            outp.status.code(),
            Some(2),
            "nul collision import must fail"
        );
        let doc = parse_json_stdout(&outp);
        assert_eq!(doc.get("code").and_then(|v| v.as_str()), Some("error"));
        assert!(
            doc.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .contains("unsafe archive entry path rejected")
        );
    }
}

#[test]
fn ci_rejects_flake_budget_without_flake_runs() {
    let ws = temp_workspace("ci-budget");
    let trace = ws.join("trace.fozzy");
    let raw = r#"{
      "format":"fozzy-trace",
      "version":2,
      "engine":{"version":"0.1.0"},
      "mode":"run",
      "scenario_path":null,
      "scenario":{"version":1,"name":"x","steps":[]},
      "decisions":[],
      "events":[],
      "summary":{
        "status":"pass",
        "mode":"run",
        "identity":{"runId":"r1","seed":1},
        "startedAt":"2026-01-01T00:00:00Z",
        "finishedAt":"2026-01-01T00:00:00Z",
        "durationMs":0
      }
    }"#;
    std::fs::write(&trace, raw).expect("write trace");
    let trace_arg = trace.to_string_lossy().to_string();

    let normal = run_cli(&[
        "ci".into(),
        trace_arg.clone(),
        "--flake-budget".into(),
        "5".into(),
        "--json".into(),
    ]);
    assert_eq!(
        normal.status.code(),
        Some(2),
        "normal mode should reject misconfig"
    );

    let strict = run_cli(&[
        "--strict".into(),
        "ci".into(),
        trace_arg,
        "--flake-budget".into(),
        "5".into(),
        "--json".into(),
    ]);
    assert_eq!(
        strict.status.code(),
        Some(2),
        "strict mode should reject misconfig"
    );
}

#[test]
fn report_flaky_rejects_duplicate_inputs() {
    let ws = temp_workspace("flake-dup");
    let runs = ws.join(".fozzy").join("runs");
    std::fs::create_dir_all(&runs).expect("mkdir");

    let mk_report = |id: &str, status: &str| {
        let dir = runs.join(id);
        std::fs::create_dir_all(&dir).expect("run dir");
        let body = format!(
            r#"{{
  "status":"{status}",
  "mode":"run",
  "identity":{{"runId":"{id}","seed":1}},
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0
}}"#
        );
        std::fs::write(dir.join("report.json"), body).expect("write report");
    };
    mk_report("r1", "pass");
    mk_report("r2", "fail");

    std::fs::write(ws.join("fozzy.toml"), "base_dir = \".fozzy\"\n").expect("write config");
    let cfg = ws.join("fozzy.toml").to_string_lossy().to_string();
    let cwd = ws.to_string_lossy().to_string();

    let out = run_cli(&[
        "report".into(),
        "flaky".into(),
        "r1".into(),
        "r1".into(),
        "r2".into(),
        "--flake-budget".into(),
        "10".into(),
        "--cwd".into(),
        cwd,
        "--config".into(),
        cfg,
    ]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "duplicate runs should be rejected"
    );
}
