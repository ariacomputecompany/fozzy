use crate::cli_workflows::*;
use crate::FullStepStatus;
use fozzy::{RunIdentity, RunMode};

pub(super) fn sample_run_summary(status: ExitStatus) -> RunSummary {
    let run_id = format!("test-run-{}", uuid::Uuid::new_v4());
    let artifacts_dir = std::env::temp_dir().join(format!("fozzy-run-summary-{run_id}"));
    std::fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");
    let report_path = artifacts_dir.join("report.json");
    let summary = RunSummary {
        status,
        mode: RunMode::Run,
        identity: RunIdentity {
            run_id,
            seed: 7,
            trace_path: None,
            report_path: Some(report_path.to_string_lossy().to_string()),
            artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 1,
        duration_ns: 1_000_000,
        tests: None,
        memory: None,
        findings: Vec::new(),
    };
    std::fs::write(
        &report_path,
        serde_json::to_vec(&summary).expect("serialize report"),
    )
    .expect("write report");
    fozzy::write_run_manifest(&summary, &artifacts_dir).expect("write manifest");
    summary
}

pub(super) fn write_trace_fixture(path: &Path, summary: &RunSummary) {
    let trace = serde_json::json!({
        "format": "fozzy-trace",
        "version": 1,
        "engine": {"version": "0.1.0"},
        "mode": summary.mode,
        "scenario_path": "tests/example.fozzy.json",
        "scenario": {"version": 1, "name": "example", "steps": []},
        "decisions": [],
        "events": [],
        "summary": serde_json::to_value(summary).expect("serialize trace summary")
    });
    std::fs::write(path, serde_json::to_vec(&trace).expect("serialize trace"))
        .expect("write trace");
}

#[test]
fn replay_summary_status_rejects_class_mismatch() {
    let summary = sample_run_summary(ExitStatus::Fail);
    let (status, detail) =
        replay_summary_status(Some(ExitStatus::Pass), &summary, true, 7, RunMode::Replay);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("class_ok=false"));
}

#[test]
fn replay_summary_status_rejects_missing_run_id() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.identity.run_id = "".to_string();
    let (status, detail) =
        replay_summary_status(Some(ExitStatus::Pass), &summary, true, 7, RunMode::Replay);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("run_id_present=false"));
}

#[test]
fn replay_summary_status_rejects_seed_mismatch() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.identity.seed = 99;
    let (status, detail) =
        replay_summary_status(Some(ExitStatus::Pass), &summary, true, 7, RunMode::Replay);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("seed_matches=false"));
    assert!(detail.contains("seed=7"));
}

#[test]
fn replay_summary_status_rejects_mode_mismatch() {
    let summary = sample_run_summary(ExitStatus::Pass);
    let (status, detail) =
        replay_summary_status(Some(ExitStatus::Pass), &summary, true, 7, RunMode::Replay);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("mode_matches=false"));
    assert!(detail.contains("mode=Replay"));
}

#[test]
fn file_artifact_status_rejects_missing_output() {
    let path = std::env::temp_dir().join(format!(
        "fozzy-missing-artifact-{}.zip",
        uuid::Uuid::new_v4()
    ));
    let (status, detail) = file_artifact_status(&path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("missing"));
}

#[test]
fn run_summary_pass_status_rejects_non_pass() {
    let summary = sample_run_summary(ExitStatus::Fail);
    let (status, detail) = run_summary_pass_status(&summary, true, 7, RunMode::Run);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("status=Fail"));
}

#[test]
fn run_summary_pass_status_rejects_missing_run_id() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.identity.run_id = "   ".to_string();
    let (status, detail) = run_summary_pass_status(&summary, true, 7, RunMode::Run);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("run_id_present=false"));
}

#[test]
fn run_summary_pass_status_rejects_seed_mismatch() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.identity.seed = 99;
    let (status, detail) = run_summary_pass_status(&summary, true, 7, RunMode::Run);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("seed_matches=false"));
    assert!(detail.contains("seed=7"));
}

#[test]
fn run_summary_pass_status_rejects_mode_mismatch() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.mode = RunMode::Test;
    let (status, detail) = run_summary_pass_status(&summary, true, 7, RunMode::Run);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("mode_matches=false"));
    assert!(detail.contains("mode=Run"));
}

#[test]
fn run_summary_pass_status_rejects_missing_report_path() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.identity.report_path = None;
    let (status, detail) = run_summary_pass_status(&summary, true, 7, RunMode::Run);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("report_present=false"));
}

#[test]
fn run_summary_pass_status_rejects_report_content_mismatch() {
    let summary = sample_run_summary(ExitStatus::Pass);
    let report_path = PathBuf::from(
        summary
            .identity
            .report_path
            .clone()
            .expect("report path present"),
    );
    let mut mismatched = summary.clone();
    mismatched.identity.seed = 99;
    std::fs::write(
        &report_path,
        serde_json::to_vec(&mismatched).expect("serialize mismatch"),
    )
    .expect("rewrite report");
    let (status, detail) = run_summary_pass_status(&summary, true, 7, RunMode::Run);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("report_content_matches=false"));
}

#[test]
fn run_summary_pass_status_rejects_manifest_content_mismatch() {
    let summary = sample_run_summary(ExitStatus::Pass);
    let artifacts_dir = PathBuf::from(
        summary
            .identity
            .artifacts_dir
            .clone()
            .expect("artifacts dir present"),
    );
    let manifest_path = artifacts_dir.join("manifest.json");
    let mut mismatched = fozzy::RunManifest {
        schema_version: "fozzy.run_manifest.v1".to_string(),
        run_id: summary.identity.run_id.clone(),
        mode: summary.mode,
        status: summary.status,
        seed: 99,
        started_at: summary.started_at.clone(),
        finished_at: summary.finished_at.clone(),
        duration_ms: summary.duration_ms,
        duration_ns: summary.duration_ns,
        trace_path: summary.identity.trace_path.clone(),
        report_path: summary.identity.report_path.clone(),
        artifacts_dir: summary.identity.artifacts_dir.clone(),
        findings_count: summary.findings.len(),
        tests_passed: None,
        tests_failed: None,
        tests_skipped: None,
        memory_leaked_bytes: None,
        memory_leaked_allocs: None,
        memory_peak_bytes: None,
        profile_capabilities: Vec::new(),
        profile_artifacts: std::collections::BTreeMap::new(),
        profile_schema_versions: std::collections::BTreeMap::new(),
    };
    mismatched.seed = 99;
    std::fs::write(
        &manifest_path,
        serde_json::to_vec(&mismatched).expect("serialize mismatch"),
    )
    .expect("rewrite manifest");
    let (status, detail) = run_summary_pass_status(&summary, true, 7, RunMode::Run);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("manifest_content_matches=false"));
}

#[test]
fn replay_summary_status_rejects_missing_artifacts_dir() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.mode = RunMode::Replay;
    summary.identity.artifacts_dir = None;
    let (status, detail) =
        replay_summary_status(Some(ExitStatus::Pass), &summary, true, 7, RunMode::Replay);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("artifacts_present=false"));
}

#[test]
fn recorded_trace_status_rejects_missing_trace_file() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.identity.trace_path = Some("/tmp/missing.trace.fozzy".to_string());
    let path = std::env::temp_dir().join(format!(
        "fozzy-missing-trace-{}.fozzy",
        uuid::Uuid::new_v4()
    ));
    let (status, detail) = recorded_trace_status(&summary, true, 7, RunMode::Run, &path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("trace_reported=true"));
    assert!(detail.contains("trace_matches=false"));
    assert!(detail.contains("missing"));
}

#[test]
fn recorded_trace_status_rejects_mismatched_reported_trace_path() {
    let path =
        std::env::temp_dir().join(format!("fozzy-trace-match-{}.fozzy", uuid::Uuid::new_v4()));
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.identity.trace_path = Some("/tmp/other.trace.fozzy".to_string());
    write_trace_fixture(&path, &summary);
    let (status, detail) = recorded_trace_status(&summary, true, 7, RunMode::Run, &path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("trace_reported=true"));
    assert!(detail.contains("trace_matches=false"));
    std::fs::remove_file(path).ok();
}

#[test]
fn recorded_trace_status_rejects_seed_mismatch() {
    let path =
        std::env::temp_dir().join(format!("fozzy-trace-seed-{}.fozzy", uuid::Uuid::new_v4()));
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.identity.seed = 99;
    summary.identity.trace_path = Some(path.display().to_string());
    write_trace_fixture(&path, &summary);
    let (status, detail) = recorded_trace_status(&summary, true, 7, RunMode::Run, &path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("seed_matches=false"));
    assert!(detail.contains("seed=7"));
    std::fs::remove_file(path).ok();
}

#[test]
fn recorded_trace_status_rejects_mode_mismatch() {
    let path =
        std::env::temp_dir().join(format!("fozzy-trace-mode-{}.fozzy", uuid::Uuid::new_v4()));
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.mode = RunMode::Test;
    summary.identity.trace_path = Some(path.display().to_string());
    write_trace_fixture(&path, &summary);
    let (status, detail) = recorded_trace_status(&summary, true, 7, RunMode::Run, &path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("mode_matches=false"));
    assert!(detail.contains("mode=Run"));
    std::fs::remove_file(path).ok();
}

#[test]
fn recorded_trace_status_rejects_trace_content_mismatch() {
    let path = std::env::temp_dir().join(format!(
        "fozzy-trace-content-{}.fozzy",
        uuid::Uuid::new_v4()
    ));
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.identity.trace_path = Some(path.display().to_string());
    let mut trace_summary = summary.clone();
    trace_summary.identity.run_id = "other-run".to_string();
    write_trace_fixture(&path, &trace_summary);
    let (status, detail) = recorded_trace_status(&summary, true, 7, RunMode::Run, &path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("trace_content_matches=false"));
    std::fs::remove_file(path).ok();
}

#[test]
fn report_show_status_rejects_empty_content() {
    let value = serde_json::json!({"format": "pretty", "content": ""});
    let (status, detail) = report_show_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("content_bytes=0"));
    assert!(detail.contains("known_format=true"));
}

#[test]
fn report_show_status_rejects_unknown_format() {
    let value = serde_json::json!({"format": "markdown", "content": "# ok"});
    let (status, detail) = report_show_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("format=markdown"));
    assert!(detail.contains("known_format=false"));
}

#[test]
fn report_show_status_rejects_blank_content() {
    let value = serde_json::json!({"format": "pretty", "content": "   \n\t  "});
    let (status, detail) = report_show_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("non_blank=false"));
}

#[test]
fn report_query_status_rejects_non_pass_status() {
    let value = serde_json::json!("fail");
    let (status, detail) = report_query_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains(".status=fail"));
}

#[test]
fn report_query_paths_status_rejects_invalid_entries() {
    let value = serde_json::json!({
        "paths": ["status", "", null]
    });
    let (status, detail) = report_query_paths_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("paths=3"));
    assert!(detail.contains("invalid=2"));
}

#[test]
fn report_query_paths_status_rejects_duplicate_entries() {
    let value = serde_json::json!({
        "paths": ["status", "status"]
    });
    let (status, detail) = report_query_paths_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("paths=2"));
    assert!(detail.contains("duplicate=1"));
}

#[test]
fn corpus_minimize_status_rejects_empty_result() {
    let value = serde_json::json!({
        "filesBefore": 0,
        "filesAfter": 0,
        "duplicatesRemoved": 0,
        "bytesBefore": 0,
        "bytesAfter": 0,
        "bytesRemoved": 0
    });
    let (status, detail) = corpus_minimize_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("files_before=0"));
}

#[test]
fn corpus_minimize_status_rejects_inconsistent_summary_math() {
    let value = serde_json::json!({
        "filesBefore": 3,
        "filesAfter": 2,
        "duplicatesRemoved": 0,
        "bytesBefore": 30,
        "bytesAfter": 20,
        "bytesRemoved": 1
    });
    let (status, detail) = corpus_minimize_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("files_before=3"));
    assert!(detail.contains("duplicates_removed=0"));
    assert!(detail.contains("bytes_removed=1"));
}

#[test]
fn corpus_add_status_rejects_missing_added_path() {
    let value = serde_json::json!({});
    let (status, detail) = corpus_add_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("missing added path"));
}

#[test]
fn corpus_list_status_rejects_missing_entry_file() {
    let value = serde_json::json!(["/tmp/definitely-missing-fozzy-corpus-entry"]);
    let (status, detail) = corpus_list_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("files=1"));
    assert!(detail.contains("invalid="));
    assert!(detail.contains("definitely-missing-fozzy-corpus-entry"));
}

#[test]
fn corpus_list_status_rejects_duplicate_entry_paths() {
    let dir = std::env::temp_dir().join(format!("fozzy-corpus-list-dup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create corpus dir");
    let entry = dir.join("input.bin");
    std::fs::write(&entry, b"seed").expect("write corpus entry");
    let value = serde_json::json!([
        entry.to_string_lossy().to_string(),
        entry.to_string_lossy().to_string()
    ]);
    let (status, detail) = corpus_list_status(&value);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("files=2"));
    assert!(detail.contains("duplicate entry path"));
}

#[test]
fn corpus_list_status_rejects_blank_entry_path() {
    let value = serde_json::json!(["   "]);
    let (status, detail) = corpus_list_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("files=1"));
    assert!(detail.contains("blank entry path"));
}

#[test]
fn corpus_import_status_rejects_missing_dir_path() {
    let value = serde_json::json!({});
    let (status, detail) = corpus_import_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("missing dir path"));
}

#[test]
fn corpus_import_status_rejects_empty_imported_file() {
    let dir = std::env::temp_dir().join(format!("fozzy-import-empty-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create import dir");
    std::fs::write(dir.join("input-empty.bin"), b"").expect("write empty file");
    let value = serde_json::json!({ "dir": dir.to_string_lossy().to_string() });
    let (status, detail) = corpus_import_status(&value);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("entries=1"));
    assert!(detail.contains("invalid="));
    assert!(detail.contains("input-empty.bin"));
}
