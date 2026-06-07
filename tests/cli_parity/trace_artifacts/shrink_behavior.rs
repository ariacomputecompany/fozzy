use super::*;

#[cfg(unix)]
#[test]
fn shrink_preserves_real_duration_in_output_trace() {
    let ws = temp_workspace("host-proc-shrink-duration");
    let scenario = ws.join("host-proc-shrink-duration.fozzy.json");
    let trace = ws.join("host-proc-shrink-duration.fozzy");
    let shrunk = ws.join("host-proc-shrink-duration.min.fozzy");
    let raw = r#"{
      "version":1,
      "name":"host-proc-shrink-duration",
      "steps":[
        {"type":"proc_spawn","cmd":"/bin/sh","args":["-lc","sleep 1; echo done"],"expect_exit":0,"expect_stdout":"done\n"}
      ]
    }"#;
    std::fs::write(&scenario, raw).expect("write scenario");

    let run = run_cli(&[
        "--proc-backend".into(),
        "host".into(),
        "run".into(),
        scenario.to_string_lossy().to_string(),
        "--det".into(),
        "--record".into(),
        trace.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(
        run.status.code(),
        Some(0),
        "host shrink source run should pass"
    );

    let shrink = run_cli(&[
        "shrink".into(),
        trace.to_string_lossy().to_string(),
        "--minimize".into(),
        "all".into(),
        "--out".into(),
        shrunk.to_string_lossy().to_string(),
        "--json".into(),
    ]);
    assert_eq!(shrink.status.code(), Some(0), "shrink should pass");

    let shrunk_doc = read_trace_json(&shrunk);
    let summary_ms = shrunk_doc
        .get("summary")
        .and_then(|v| v.get("durationMs"))
        .and_then(|v| v.as_u64())
        .expect("shrunk trace summary duration");
    assert!(
        summary_ms >= 900,
        "expected shrunk trace summary duration to preserve real runtime evidence, got {summary_ms}"
    );
}

#[test]
fn shrink_rejects_unsupported_reporter_flag() {
    let ws = temp_workspace("shrink-reporter-reject");
    let scenario = ws.join("memory.pass.fozzy.json");
    std::fs::write(&scenario, fixture("memory.pass.fozzy.json")).expect("write scenario");
    let trace = ws.join("trace.fozzy");
    let shrunk = ws.join("trace.min.fozzy");

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

    let shrink = run_cli_in(
        &ws,
        &[
            "shrink".into(),
            trace.display().to_string(),
            "--out".into(),
            shrunk.display().to_string(),
            "--reporter".into(),
            "html".into(),
            "--json".into(),
        ],
    );
    assert_ne!(
        shrink.status.code(),
        Some(0),
        "shrink with unsupported reporter must fail"
    );
    let stdout = String::from_utf8_lossy(&shrink.stdout);
    assert!(
        stdout.contains("invalid value 'html' for '--reporter <REPORTER>'")
            || stdout.contains("possible values: pretty"),
        "stdout: {stdout}"
    );
}
