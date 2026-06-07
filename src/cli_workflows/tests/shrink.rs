use super::reports::{sample_run_summary, write_trace_fixture};
use crate::cli_workflows::*;
use crate::FullStepStatus;

#[test]
fn shrink_step_status_rejects_strict_warning_for_pass_summary() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.mode = RunMode::Replay;
    summary.findings = vec![fozzy::Finding {
        kind: fozzy::FindingKind::Checker,
        title: "memory_leak".to_string(),
        message: "detected 1 leaked allocation(s)".to_string(),
        location: None,
    }];
    let out_trace = std::env::temp_dir().join(format!("shrink-{}.fozzy", uuid::Uuid::new_v4()));
    summary.identity.trace_path = Some(out_trace.display().to_string());
    write_trace_fixture(&out_trace, &summary);
    let (status, detail, classification) = shrink_step_status(
        Some(ExitStatus::Pass),
        &summary,
        true,
        7,
        RunMode::Replay,
        false,
        &out_trace,
    );
    let _ = std::fs::remove_file(&out_trace);
    assert!(matches!(status, FullStepStatus::Failed));
    assert_eq!(classification, "strict_policy_rejected");
    assert!(detail.contains("strict_ok=false"));
    assert!(detail.contains("status=Pass"));
}

#[test]
fn shrink_step_status_rejects_missing_run_id() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.mode = RunMode::Replay;
    summary.identity.run_id.clear();
    let out_trace = std::env::temp_dir().join(format!("shrink-{}.fozzy", uuid::Uuid::new_v4()));
    summary.identity.trace_path = Some(out_trace.display().to_string());
    write_trace_fixture(&out_trace, &summary);
    let (status, detail, classification) = shrink_step_status(
        Some(ExitStatus::Pass),
        &summary,
        true,
        7,
        RunMode::Replay,
        false,
        &out_trace,
    );
    let _ = std::fs::remove_file(&out_trace);
    assert!(matches!(status, FullStepStatus::Failed));
    assert_eq!(classification, "run_identity_missing");
    assert!(detail.contains("run_id_present=false"));
}

#[test]
fn shrink_step_status_rejects_missing_out_trace_artifact() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.mode = RunMode::Replay;
    let missing = std::env::temp_dir().join(format!("missing-{}.fozzy", uuid::Uuid::new_v4()));
    summary.identity.trace_path = Some(missing.display().to_string());
    let (status, detail, classification) = shrink_step_status(
        Some(ExitStatus::Pass),
        &summary,
        true,
        7,
        RunMode::Replay,
        false,
        &missing,
    );
    assert!(matches!(status, FullStepStatus::Failed));
    assert_eq!(classification, "out_trace_missing");
    assert!(detail.contains("missing"));
}

#[test]
fn shrink_step_status_rejects_mismatched_reported_trace_path() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.mode = RunMode::Replay;
    let out_trace = std::env::temp_dir().join(format!("shrink-{}.fozzy", uuid::Uuid::new_v4()));
    let other_trace = std::env::temp_dir().join(format!("other-{}.fozzy", uuid::Uuid::new_v4()));
    summary.identity.trace_path = Some(other_trace.display().to_string());
    write_trace_fixture(&out_trace, &summary);
    let (status, detail, classification) = shrink_step_status(
        Some(ExitStatus::Pass),
        &summary,
        true,
        7,
        RunMode::Replay,
        false,
        &out_trace,
    );
    let _ = std::fs::remove_file(&out_trace);
    assert!(matches!(status, FullStepStatus::Failed));
    assert_eq!(classification, "out_trace_identity_mismatch");
    assert!(detail.contains("trace_reported=true"));
    assert!(detail.contains("trace_matches=false"));
}

#[test]
fn shrink_step_status_rejects_seed_mismatch() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.mode = RunMode::Replay;
    summary.identity.seed = 99;
    let out_trace = std::env::temp_dir().join(format!("shrink-{}.fozzy", uuid::Uuid::new_v4()));
    summary.identity.trace_path = Some(out_trace.display().to_string());
    write_trace_fixture(&out_trace, &summary);
    let (status, detail, classification) = shrink_step_status(
        Some(ExitStatus::Pass),
        &summary,
        true,
        7,
        RunMode::Replay,
        false,
        &out_trace,
    );
    let _ = std::fs::remove_file(&out_trace);
    assert!(matches!(status, FullStepStatus::Failed));
    assert_eq!(classification, "seed_mismatch");
    assert!(detail.contains("seed_matches=false"));
    assert!(detail.contains("seed=7"));
}

#[test]
fn shrink_step_status_rejects_mode_mismatch() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    let out_trace = std::env::temp_dir().join(format!("shrink-{}.fozzy", uuid::Uuid::new_v4()));
    summary.mode = RunMode::Run;
    summary.identity.trace_path = Some(out_trace.display().to_string());
    write_trace_fixture(&out_trace, &summary);
    let (status, detail, classification) = shrink_step_status(
        Some(ExitStatus::Pass),
        &summary,
        true,
        7,
        RunMode::Replay,
        false,
        &out_trace,
    );
    let _ = std::fs::remove_file(&out_trace);
    assert!(matches!(status, FullStepStatus::Failed));
    assert_eq!(classification, "mode_mismatch");
    assert!(detail.contains("mode_matches=false"));
    assert!(detail.contains("mode=Replay"));
}

#[test]
fn shrink_step_status_rejects_trace_content_mismatch() {
    let mut summary = sample_run_summary(ExitStatus::Pass);
    summary.mode = RunMode::Replay;
    let out_trace = std::env::temp_dir().join(format!("shrink-{}.fozzy", uuid::Uuid::new_v4()));
    summary.identity.trace_path = Some(out_trace.display().to_string());
    let mut trace_summary = summary.clone();
    trace_summary.identity.run_id = "other-run".to_string();
    write_trace_fixture(&out_trace, &trace_summary);
    let (status, detail, classification) = shrink_step_status(
        Some(ExitStatus::Pass),
        &summary,
        true,
        7,
        RunMode::Replay,
        false,
        &out_trace,
    );
    let _ = std::fs::remove_file(&out_trace);
    assert!(matches!(status, FullStepStatus::Failed));
    assert_eq!(classification, "out_trace_content_mismatch");
    assert!(detail.contains("trace_content_matches=false"));
}

#[test]
fn flaky_report_status_rejects_flaky_results() {
    let value = serde_json::json!({
        "runCount": 2,
        "statusCounts": {"pass": 1, "fail": 1},
        "findingTitleSets": [[], ["boom"]],
        "isFlaky": true,
        "flakeRatePct": 50.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("is_flaky=true"));
}

#[test]
fn flaky_report_status_rejects_zero_run_count() {
    let value = serde_json::json!({
        "runCount": 0,
        "statusCounts": {},
        "findingTitleSets": [],
        "isFlaky": false,
        "flakeRatePct": 0.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("run_count=0"));
}

#[test]
fn flaky_report_status_rejects_inconsistent_payload() {
    let value = serde_json::json!({
        "runCount": 2,
        "statusCounts": {"pass": 2},
        "findingTitleSets": [[]],
        "isFlaky": false,
        "flakeRatePct": 50.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("derived_flaky=false"));
    assert!(detail.contains("flake_rate_pct=50"));
}

#[test]
fn flaky_report_status_rejects_invalid_status_keys() {
    let value = serde_json::json!({
        "runCount": 2,
        "statusCounts": {"": 2},
        "findingTitleSets": [["ok"]],
        "isFlaky": false,
        "flakeRatePct": 0.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_status_keys=1"));
}

#[test]
fn flaky_report_status_rejects_invalid_status_values() {
    let value = serde_json::json!({
        "runCount": 2,
        "statusCounts": {"pass": 0},
        "findingTitleSets": [[]],
        "isFlaky": false,
        "flakeRatePct": 0.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_status_values=1"));
}

#[test]
fn flaky_report_status_rejects_status_total_mismatch() {
    let value = serde_json::json!({
        "runCount": 3,
        "statusCounts": {"pass": 2},
        "findingTitleSets": [[]],
        "isFlaky": false,
        "flakeRatePct": 0.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("status_total=2"));
    assert!(detail.contains("run_count=3"));
}

#[test]
fn flaky_report_status_rejects_invalid_finding_rows() {
    let value = serde_json::json!({
        "runCount": 2,
        "statusCounts": {"pass": 2},
        "findingTitleSets": [[null]],
        "isFlaky": false,
        "flakeRatePct": 0.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_finding_rows=1"));
}

#[test]
fn flaky_report_status_rejects_duplicate_titles_within_row() {
    let value = serde_json::json!({
        "runCount": 2,
        "statusCounts": {"pass": 2},
        "findingTitleSets": [["boom", "boom"]],
        "isFlaky": false,
        "flakeRatePct": 0.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_titles_within_rows=1"));
}

#[test]
fn flaky_report_status_rejects_duplicate_finding_rows() {
    let value = serde_json::json!({
        "runCount": 2,
        "statusCounts": {"pass": 2},
        "findingTitleSets": [["boom"], ["boom"]],
        "isFlaky": false,
        "flakeRatePct": 0.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_finding_rows=1"));
}

#[test]
fn flaky_report_status_allows_empty_finding_rows_for_clean_runs() {
    let value = serde_json::json!({
        "runCount": 2,
        "statusCounts": {"pass": 2},
        "findingTitleSets": [[]],
        "isFlaky": false,
        "flakeRatePct": 0.0
    });
    let (status, detail) = flaky_report_status(&value);
    assert!(matches!(status, FullStepStatus::Passed));
    assert!(detail.contains("invalid_finding_rows=0"));
    assert!(detail.contains("status_total=2"));
}

#[test]
fn memory_top_status_rejects_leaks() {
    let value = serde_json::json!({
        "total": 1,
        "leaks": [{"allocId": 1}]
    });
    let (status, detail) = memory_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("total_leaks=1"));
}

#[test]
fn memory_top_status_rejects_inconsistent_payload() {
    let value = serde_json::json!({
        "total": 0,
        "leaks": [{"allocId": 1}]
    });
    let (status, detail) = memory_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("shown=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn memory_top_status_rejects_duplicate_alloc_ids() {
    let value = serde_json::json!({
        "total": 2,
        "leaks": [
            {"allocId": 7, "bytes": 64, "callsiteHash": "abc"},
            {"allocId": 7, "bytes": 32, "callsiteHash": "def"}
        ]
    });
    let (status, detail) = memory_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_alloc_ids=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn memory_top_status_rejects_invalid_leak_rows() {
    let value = serde_json::json!({
        "total": 1,
        "leaks": [
            {"allocId": 0, "bytes": 0, "callsiteHash": ""}
        ]
    });
    let (status, detail) = memory_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_rows=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn memory_diff_status_rejects_contract_drift() {
    let value = serde_json::json!({
        "leftLeakedBytes": 0,
        "rightLeakedBytes": 64,
        "leftLeakedAllocs": 0,
        "rightLeakedAllocs": 1,
        "leftPeakBytes": 0,
        "rightPeakBytes": 0,
        "deltaLeakedBytes": 64,
        "deltaLeakedAllocs": 1,
        "deltaPeakBytes": 0
    });
    let (status, detail) = memory_diff_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("delta_leaked_bytes=64"));
}

#[test]
fn memory_diff_status_rejects_inconsistent_delta_math() {
    let value = serde_json::json!({
        "leftLeakedBytes": 0,
        "rightLeakedBytes": 64,
        "leftLeakedAllocs": 0,
        "rightLeakedAllocs": 1,
        "leftPeakBytes": 0,
        "rightPeakBytes": 10,
        "deltaLeakedBytes": 0,
        "deltaLeakedAllocs": 0,
        "deltaPeakBytes": 0
    });
    let (status, detail) = memory_diff_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("consistent=false"));
}
