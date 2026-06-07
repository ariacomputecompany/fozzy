use crate::cli_workflows::*;
use crate::FullStepStatus;

#[test]
fn profile_diff_status_rejects_regressions() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "minor_regression",
            "regressionCount": 1,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": "cpu_time_ms"
        },
        "regressions": [{
            "domain": "cpu",
            "metric": "cpu_time_ms",
            "timeDomain": "host_monotonic_time",
            "classification": "regression",
            "isRegression": true,
            "isSignificant": false
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("verdict=minor_regression"));
}

#[test]
fn profile_diff_status_requires_stable_when_requested() {
    let value = serde_json::json!({
        "domains": ["heap"],
        "summary": {
            "verdict": "improvement",
            "regressionCount": 0,
            "improvementCount": 1,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "heap",
            "metric": "alloc_bytes",
            "timeDomain": "virtual_time",
            "classification": "improvement",
            "isRegression": false,
            "isSignificant": false
        }]
    });
    let (status, _) = profile_diff_status(&value, true);
    assert!(matches!(status, FullStepStatus::Failed));

    let (status_no_stable, _) = profile_diff_status(&value, false);
    assert!(matches!(status_no_stable, FullStepStatus::Passed));
}

#[test]
fn profile_diff_status_rejects_unknown_verdicts() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "unknown",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": []
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("verdict=unknown"));
}

#[test]
fn profile_diff_status_rejects_inconsistent_summary_counts() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "cpu",
            "metric": "cpu_time_ms",
            "timeDomain": "host_monotonic_time",
            "classification": "regression",
            "isRegression": true,
            "isSignificant": true
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn profile_diff_status_rejects_duplicate_regression_rows() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [
            {
                "domain": "cpu",
                "metric": "cpu_time_ms",
                "timeDomain": "host_monotonic_time",
                "classification": "regression",
                "isRegression": true,
                "isSignificant": false
            },
            {
                "domain": "cpu",
                "metric": "cpu_time_ms",
                "timeDomain": "host_monotonic_time",
                "classification": "regression",
                "isRegression": true,
                "isSignificant": false
            }
        ]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_rows=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn profile_diff_status_rejects_invalid_regression_rows() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "",
            "metric": "",
            "timeDomain": "",
            "classification": ""
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_rows=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn profile_diff_status_rejects_unknown_domains() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "mystery",
            "metric": "cpu_time_ms",
            "timeDomain": "host_monotonic_time",
            "classification": "stable",
            "isRegression": false,
            "isSignificant": false
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_rows=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn profile_diff_status_rejects_unknown_time_domains() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "cpu",
            "metric": "cpu_time_ms",
            "timeDomain": "cpu",
            "classification": "stable",
            "isRegression": false,
            "isSignificant": false
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_rows=1"));
}

#[test]
fn profile_diff_status_rejects_metric_time_domain_mismatch() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "cpu",
            "metric": "cpu_time_ms",
            "timeDomain": "virtual_time",
            "classification": "stable",
            "isRegression": false,
            "isSignificant": false
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_rows=1"));
}

#[test]
fn profile_diff_status_rejects_unknown_metric_for_domain() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "io",
            "metric": "cpu_time_ms",
            "timeDomain": "host_monotonic_time",
            "classification": "stable",
            "isRegression": false,
            "isSignificant": false
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_rows=1"));
}

#[test]
fn profile_diff_status_rejects_semantically_inconsistent_rows() {
    let value = serde_json::json!({
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "cpu",
            "metric": "cpu_time_ms",
            "timeDomain": "host_monotonic_time",
            "classification": "improvement",
            "isRegression": true,
            "isSignificant": false
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_rows=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn profile_diff_status_rejects_unknown_domains_array_entries() {
    let value = serde_json::json!({
        "domains": ["cpu", "mystery"],
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "cpu",
            "metric": "cpu_time_ms",
            "timeDomain": "host_monotonic_time",
            "classification": "stable",
            "isRegression": false,
            "isSignificant": false
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_domains=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn profile_diff_status_rejects_duplicate_domains_array_entries() {
    let value = serde_json::json!({
        "domains": ["cpu", "cpu"],
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "cpu",
            "metric": "cpu_time_ms",
            "timeDomain": "host_monotonic_time",
            "classification": "stable",
            "isRegression": false,
            "isSignificant": false
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_domains=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn profile_diff_status_rejects_domain_array_row_mismatch() {
    let value = serde_json::json!({
        "domains": ["cpu"],
        "summary": {
            "verdict": "stable",
            "regressionCount": 0,
            "improvementCount": 0,
            "significantRegressionCount": 0,
            "topRegressionMetric": null
        },
        "regressions": [{
            "domain": "heap",
            "metric": "alloc_bytes",
            "timeDomain": "virtual_time",
            "classification": "stable",
            "isRegression": false,
            "isSignificant": false
        }]
    });
    let (status, detail) = profile_diff_status(&value, false);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_domains=0"));
    assert!(detail.contains("duplicate_domains=0"));
    assert!(detail.contains("domains_consistent=false"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn profile_top_status_skips_empty_domains() {
    let value = serde_json::json!({
        "warnings": [],
        "emptyDomains": [{"domain": "heap", "empty": true, "reason": "no heap samples in trace"}]
    });
    let (status, detail) = profile_top_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("heap:no heap samples in trace"));
    assert!(detail.contains("has_concrete_domain_data=false"));
    assert!(detail.contains("invalid_empty_domains=0"));
}

#[test]
fn profile_top_status_rejects_warnings() {
    let value = serde_json::json!({
        "warnings": ["cpu domain uses host-time sampling"],
        "emptyDomains": []
    });
    let (status, detail) = profile_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("warnings=cpu domain uses host-time sampling"));
    assert!(detail.contains("invalid_warnings=0"));
}

#[test]
fn profile_top_status_rejects_invalid_empty_domain_rows() {
    let value = serde_json::json!({
        "warnings": [],
        "emptyDomains": [{"domain": "heap", "reason": "no heap samples in trace"}]
    });
    let (status, detail) = profile_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_empty_domains=1"));
}

#[test]
fn profile_top_status_rejects_unknown_empty_domain_rows() {
    let value = serde_json::json!({
        "warnings": [],
        "emptyDomains": [{"domain": "mystery", "empty": true, "reason": "no mystery samples in trace"}]
    });
    let (status, detail) = profile_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_empty_domains=1"));
}

#[test]
fn profile_top_status_rejects_invalid_warning_rows() {
    let value = serde_json::json!({
        "warnings": ["", null],
        "emptyDomains": []
    });
    let (status, detail) = profile_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_warnings=2"));
}

#[test]
fn profile_top_status_rejects_duplicate_warning_rows() {
    let value = serde_json::json!({
        "warnings": ["cpu domain uses host-time sampling", "cpu domain uses host-time sampling"],
        "emptyDomains": []
    });
    let (status, detail) = profile_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_warnings=1"));
}

#[test]
fn profile_top_status_rejects_duplicate_empty_domain_rows() {
    let value = serde_json::json!({
        "warnings": [],
        "emptyDomains": [
            {"domain": "heap", "empty": true, "reason": "no heap samples in trace"},
            {"domain": "heap", "empty": true, "reason": "no heap samples in trace"}
        ]
    });
    let (status, detail) = profile_top_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_empty_domains=1"));
}

#[test]
fn profile_explain_status_skips_non_diagnostic_results() {
    let value = serde_json::json!({
        "regressionStatement": "no measurable regression shift found",
        "likelyCauseDomain": "unknown",
        "topShiftedPath": "n/a",
        "evidencePointers": ["profile.metrics.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("cause_domain=unknown"));
    assert!(detail.contains("shifted_path=n/a"));
}

#[test]
fn profile_explain_status_skips_missing_regression_statement() {
    let value = serde_json::json!({
        "likelyCauseDomain": "latency",
        "topShiftedPath": "metric::p99_ms",
        "evidencePointers": ["profile.metrics.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("cause_domain=latency"));
    assert!(detail.contains("shifted_path=metric::p99_ms"));
}

#[test]
fn profile_explain_status_skips_unknown_cause_domain() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "mystery",
        "topShiftedPath": "metric::p99_latency_ms",
        "evidencePointers": ["profile.metrics.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("cause_domain=mystery"));
}

#[test]
fn profile_explain_status_skips_non_metric_shifted_path() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "latency",
        "topShiftedPath": "critical_path",
        "evidencePointers": ["profile.metrics.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("shifted_path=critical_path"));
}

#[test]
fn profile_explain_status_skips_domain_metric_mismatch() {
    let value = serde_json::json!({
        "regressionStatement": "io ops changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "io",
        "topShiftedPath": "metric::cpu_time_ms",
        "evidencePointers": ["profile.metrics.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("cause_domain=io"));
    assert!(detail.contains("shifted_path=metric::cpu_time_ms"));
}

#[test]
fn profile_explain_status_skips_single_run_observational_summary() {
    let value = serde_json::json!({
        "regressionStatement": "run abc123 shows p50/p95/p99/max=0/0/0/0ms, alloc_bytes=128",
        "likelyCauseDomain": "heap",
        "topShiftedPath": "root -> step-0 (0ms)",
        "evidencePointers": ["profile.metrics.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("cause_domain=heap"));
    assert!(detail.contains("shifted_path=root -> step-0 (0ms)"));
}

#[test]
fn profile_explain_status_accepts_real_diagnosis() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "latency",
        "topShiftedPath": "metric::p99_latency_ms",
        "evidencePointers": ["profile.metrics.json", "profile.latency.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Passed));
    assert!(detail.contains("cause_domain=latency"));
    assert!(detail.contains("shifted_path=metric::p99_latency_ms"));
    assert!(detail.contains("evidence_pointers=2"));
}

#[test]
fn profile_explain_status_skips_missing_evidence_pointers() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "latency",
        "topShiftedPath": "metric::p99_latency_ms",
        "evidencePointers": []
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("evidence_pointers=0"));
}

#[test]
fn profile_explain_status_skips_invalid_evidence_pointer_rows() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "latency",
        "topShiftedPath": "metric::p99_latency_ms",
        "evidencePointers": ["", null]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("invalid_evidence_pointers=2"));
}

#[test]
fn profile_explain_status_skips_unknown_evidence_pointer_files() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "latency",
        "topShiftedPath": "metric::p99_latency_ms",
        "evidencePointers": ["report.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("invalid_evidence_pointers=1"));
}

#[test]
fn profile_explain_status_accepts_absolute_known_evidence_pointer_files() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "latency",
        "topShiftedPath": "metric::p99_latency_ms",
        "evidencePointers": ["/tmp/run/profile.metrics.json", "/tmp/run/profile.latency.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Passed));
    assert!(detail.contains("invalid_evidence_pointers=0"));
}

#[test]
fn profile_explain_status_skips_missing_metrics_evidence_pointer() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "latency",
        "topShiftedPath": "metric::p99_latency_ms",
        "evidencePointers": ["profile.latency.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("metrics_pointer_present=false"));
}

#[test]
fn profile_explain_status_skips_missing_domain_specific_evidence_pointer() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "latency",
        "topShiftedPath": "metric::p99_latency_ms",
        "evidencePointers": ["profile.metrics.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("domain_pointer_required=true"));
    assert!(detail.contains("domain_pointer_present=false"));
}

#[test]
fn profile_explain_status_skips_duplicate_evidence_pointers() {
    let value = serde_json::json!({
        "regressionStatement": "latency p99 changed from 10.0 to 25.0 (+150.0%)",
        "likelyCauseDomain": "latency",
        "topShiftedPath": "metric::p99_latency_ms",
        "evidencePointers": ["profile.metrics.json", "profile.metrics.json"]
    });
    let (status, detail) = profile_explain_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("duplicate_evidence_pointers=1"));
}
