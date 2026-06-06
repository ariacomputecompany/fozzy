    use super::*;
    use fozzy::{RunIdentity, RunMode};

    fn topology_status_for_report(report: &fozzy::MapSuitesReport) -> (FullStepStatus, String) {
        topology_coverage_status(
            report,
            Path::new(&report.root),
            Path::new(&report.scenario_root),
            report.profile,
            report.shrink_policy,
            report.base_min_risk,
        )
    }

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

    fn sample_run_summary(status: ExitStatus) -> RunSummary {
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

    fn write_trace_fixture(path: &Path, summary: &RunSummary) {
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
        let dir =
            std::env::temp_dir().join(format!("fozzy-corpus-list-dup-{}", uuid::Uuid::new_v4()));
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

    #[test]
    fn memory_graph_status_skips_empty_graph() {
        let value = serde_json::json!({"graph": {"nodes": [], "edges": []}});
        let (status, detail) = memory_graph_status(&value);
        assert!(matches!(status, FullStepStatus::Skipped));
        assert!(detail.contains("nodes=0"));
        assert!(detail.contains("edges=0"));
    }

    #[test]
    fn memory_graph_status_rejects_invalid_edge_references() {
        let value = serde_json::json!({
            "graph": {
                "nodes": [{"id": "alloc:1", "kind": "alloc", "label": "a"}],
                "edges": [{"from": "alloc:1", "to": "alloc:2", "kind": "owns"}]
            }
        });
        let (status, detail) = memory_graph_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_edges=1"));
        assert!(detail.contains("consistent=false"));
    }

    #[test]
    fn memory_graph_status_rejects_invalid_node_ids() {
        let value = serde_json::json!({
            "graph": {
                "nodes": [
                    {"id": "", "kind": "alloc", "label": "blank"},
                    {"kind": "alloc", "label": "missing"}
                ],
                "edges": []
            }
        });
        let (status, detail) = memory_graph_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_nodes=2"));
        assert!(detail.contains("consistent=false"));
    }

    #[test]
    fn memory_graph_status_rejects_blank_edge_kind() {
        let value = serde_json::json!({
            "graph": {
                "nodes": [
                    {"id": "alloc:1", "kind": "alloc", "label": "a"},
                    {"id": "alloc:2", "kind": "alloc", "label": "b"}
                ],
                "edges": [{"from": "alloc:1", "to": "alloc:2", "kind": ""}]
            }
        });
        let (status, detail) = memory_graph_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_edges=1"));
        assert!(detail.contains("consistent=false"));
    }

    #[test]
    fn memory_graph_status_rejects_duplicate_node_ids() {
        let value = serde_json::json!({
            "graph": {
                "nodes": [
                    {"id": "alloc:1", "kind": "alloc", "label": "a"},
                    {"id": "alloc:1", "kind": "alloc", "label": "a-dup"}
                ],
                "edges": []
            }
        });
        let (status, detail) = memory_graph_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("duplicate_nodes=1"));
        assert!(detail.contains("consistent=false"));
    }

    #[test]
    fn memory_graph_status_rejects_duplicate_edges() {
        let value = serde_json::json!({
            "graph": {
                "nodes": [
                    {"id": "alloc:1", "kind": "alloc", "label": "a"},
                    {"id": "alloc:2", "kind": "alloc", "label": "b"}
                ],
                "edges": [
                    {"from": "alloc:1", "to": "alloc:2", "kind": "owns"},
                    {"from": "alloc:1", "to": "alloc:2", "kind": "owns"}
                ]
            }
        });
        let (status, detail) = memory_graph_status(&value);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("duplicate_edges=1"));
        assert!(detail.contains("consistent=false"));
    }

    #[test]
    fn artifacts_list_status_rejects_empty_entries() {
        let output = fozzy::ArtifactOutput::List {
            entries: Vec::new(),
        };
        let path = PathBuf::from("/tmp/example.trace.fozzy");
        let (status, detail) = artifacts_list_status(&output, &path);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("entries=0"));
    }

    #[test]
    fn artifacts_list_status_rejects_missing_entry_file() {
        let output = fozzy::ArtifactOutput::List {
            entries: vec![fozzy::ArtifactEntry {
                kind: fozzy::ArtifactKind::Trace,
                path: "/tmp/definitely-missing-fozzy-artifact".to_string(),
                size_bytes: Some(10),
            }],
        };
        let path = PathBuf::from("/tmp/example.trace.fozzy");
        let (status, detail) = artifacts_list_status(&output, &path);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid="));
        assert!(detail.contains("definitely-missing-fozzy-artifact"));
    }

    #[test]
    fn artifacts_list_status_rejects_duplicate_entry_paths() {
        let dir =
            std::env::temp_dir().join(format!("fozzy-artifacts-list-dup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create artifact dir");
        let artifact = dir.join("trace.fozzy");
        std::fs::write(&artifact, b"trace").expect("write artifact");
        let path_str = artifact.to_string_lossy().to_string();
        let output = fozzy::ArtifactOutput::List {
            entries: vec![
                fozzy::ArtifactEntry {
                    kind: fozzy::ArtifactKind::Trace,
                    path: path_str.clone(),
                    size_bytes: Some(5),
                },
                fozzy::ArtifactEntry {
                    kind: fozzy::ArtifactKind::Trace,
                    path: path_str,
                    size_bytes: Some(5),
                },
            ],
        };
        let path = PathBuf::from("/tmp/example.trace.fozzy");
        let (status, detail) = artifacts_list_status(&output, &path);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("entries=2"));
        assert!(detail.contains("duplicate artifact path"));
    }

    #[test]
    fn artifacts_list_status_rejects_blank_entry_path() {
        let output = fozzy::ArtifactOutput::List {
            entries: vec![fozzy::ArtifactEntry {
                kind: fozzy::ArtifactKind::Trace,
                path: "   ".to_string(),
                size_bytes: Some(5),
            }],
        };
        let path = PathBuf::from("/tmp/example.trace.fozzy");
        let (status, detail) = artifacts_list_status(&output, &path);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("entries=1"));
        assert!(detail.contains("blank artifact path"));
    }

    #[test]
    fn artifacts_diff_status_rejects_inconsistent_file_delta() {
        let output = fozzy::ArtifactOutput::Diff {
            diff: Box::new(fozzy::ArtifactDiff {
                left: "left".to_string(),
                right: "right".to_string(),
                files: vec![fozzy::ArtifactFileDelta {
                    key: "Trace:trace.fozzy".to_string(),
                    left_path: Some("/tmp/left.trace.fozzy".to_string()),
                    right_path: Some("/tmp/right.trace.fozzy".to_string()),
                    left_size_bytes: Some(10),
                    right_size_bytes: Some(11),
                    changed: false,
                }],
                report: None,
                trace: None,
            }),
        };
        let (status, detail) = artifacts_diff_status(&output);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid=1"));
    }

    #[test]
    fn artifacts_diff_status_allows_same_size_changed_file_delta() {
        let output = fozzy::ArtifactOutput::Diff {
            diff: Box::new(fozzy::ArtifactDiff {
                left: "left".to_string(),
                right: "right".to_string(),
                files: vec![fozzy::ArtifactFileDelta {
                    key: "Trace:trace.fozzy".to_string(),
                    left_path: Some("/tmp/left.trace.fozzy".to_string()),
                    right_path: Some("/tmp/right.trace.fozzy".to_string()),
                    left_size_bytes: Some(10),
                    right_size_bytes: Some(10),
                    changed: true,
                }],
                report: None,
                trace: None,
            }),
        };
        let (status, detail) = artifacts_diff_status(&output);
        assert!(matches!(status, FullStepStatus::Passed));
        assert!(detail.contains("invalid=0"));
    }

    #[test]
    fn artifacts_diff_status_rejects_duplicate_file_delta_keys() {
        let output = fozzy::ArtifactOutput::Diff {
            diff: Box::new(fozzy::ArtifactDiff {
                left: "left".to_string(),
                right: "right".to_string(),
                files: vec![
                    fozzy::ArtifactFileDelta {
                        key: "Trace:trace.fozzy".to_string(),
                        left_path: Some("/tmp/left.trace.fozzy".to_string()),
                        right_path: Some("/tmp/right.trace.fozzy".to_string()),
                        left_size_bytes: Some(10),
                        right_size_bytes: Some(11),
                        changed: true,
                    },
                    fozzy::ArtifactFileDelta {
                        key: "Trace:trace.fozzy".to_string(),
                        left_path: Some("/tmp/left.trace.fozzy".to_string()),
                        right_path: Some("/tmp/right.trace.fozzy".to_string()),
                        left_size_bytes: Some(10),
                        right_size_bytes: Some(11),
                        changed: true,
                    },
                ],
                report: None,
                trace: None,
            }),
        };
        let (status, detail) = artifacts_diff_status(&output);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid=1"));
    }

    #[test]
    fn artifacts_diff_status_rejects_blank_diff_identities() {
        let output = fozzy::ArtifactOutput::Diff {
            diff: Box::new(fozzy::ArtifactDiff {
                left: "   ".to_string(),
                right: "".to_string(),
                files: vec![fozzy::ArtifactFileDelta {
                    key: "Trace:trace.fozzy".to_string(),
                    left_path: Some("/tmp/left.trace.fozzy".to_string()),
                    right_path: Some("/tmp/right.trace.fozzy".to_string()),
                    left_size_bytes: Some(10),
                    right_size_bytes: Some(11),
                    changed: true,
                }],
                report: None,
                trace: None,
            }),
        };
        let (status, detail) = artifacts_diff_status(&output);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("left_ok=false"));
        assert!(detail.contains("right_ok=false"));
    }

    #[test]
    fn env_step_status_rejects_unknown_backends() {
        let env = fozzy::EnvInfo {
            os: "macos".to_string(),
            arch: "aarch64".to_string(),
            fozzy: fozzy::version_info(),
            capabilities: std::collections::BTreeMap::new(),
        };
        let (status, detail) = env_step_status(&env);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("proc=unknown"));
        assert!(detail.contains("known_proc=false"));
    }

    #[test]
    fn env_step_status_rejects_invalid_backend_names() {
        let mut capabilities = std::collections::BTreeMap::new();
        capabilities.insert(
            "proc".to_string(),
            fozzy::CapabilityInfo {
                backend: "sandboxed".to_string(),
                deterministic: true,
            },
        );
        capabilities.insert(
            "fs".to_string(),
            fozzy::CapabilityInfo {
                backend: "overlay".to_string(),
                deterministic: true,
            },
        );
        capabilities.insert(
            "http".to_string(),
            fozzy::CapabilityInfo {
                backend: "mock".to_string(),
                deterministic: true,
            },
        );
        let env = fozzy::EnvInfo {
            os: "macos".to_string(),
            arch: "aarch64".to_string(),
            fozzy: fozzy::version_info(),
            capabilities,
        };
        let (status, detail) = env_step_status(&env);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("proc=sandboxed"));
        assert!(detail.contains("known_proc=false"));
        assert!(detail.contains("known_fs=false"));
        assert!(detail.contains("known_http=false"));
    }

    #[test]
    fn zip_artifact_status_rejects_invalid_zip_payload() {
        let path =
            std::env::temp_dir().join(format!("fozzy-invalid-zip-{}.zip", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"not a zip").expect("write invalid zip");
        let (status, detail) = zip_artifact_status(&path);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("zip_parse_error="));
    }

    #[test]
    fn zip_artifact_status_rejects_empty_zip_archive() {
        let path =
            std::env::temp_dir().join(format!("fozzy-empty-zip-{}.zip", uuid::Uuid::new_v4()));
        {
            let file = std::fs::File::create(&path).expect("create empty zip");
            let zip = zip::ZipWriter::new(file);
            zip.finish().expect("finish empty zip");
        }
        let (status, detail) = zip_artifact_status(&path);
        let _ = std::fs::remove_file(&path);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("zip_entries=0"));
    }

    #[test]
    fn ci_report_status_surfaces_failing_check_detail() {
        let report = fozzy::CiReport {
            schema_version: "fozzy.ci_report.v1".to_string(),
            ok: false,
            checks: vec![
                fozzy::CiCheck {
                    name: "trace_verify".to_string(),
                    ok: true,
                    detail: Some(
                        "checksum_present=true checksum_valid=true warnings=<none>".to_string(),
                    ),
                },
                fozzy::CiCheck {
                    name: "strict_warning_policy".to_string(),
                    ok: false,
                    detail: Some(
                        "strict=true warnings=[\"detected 1 leaked allocation(s)\"]".to_string(),
                    ),
                },
            ],
        };
        let (status, detail) = ci_report_status(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("checks=2"));
        assert!(detail.contains("strict_warning_policy: strict=true warnings="));
    }

    #[test]
    fn ci_report_status_rejects_inconsistent_ok_summary() {
        let report = fozzy::CiReport {
            schema_version: "fozzy.ci_report.v1".to_string(),
            ok: true,
            checks: vec![fozzy::CiCheck {
                name: "trace_verify".to_string(),
                ok: false,
                detail: Some("checksum_valid=false".to_string()),
            }],
        };
        let (status, detail) = ci_report_status(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("reported_ok=true"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn ci_report_status_rejects_invalid_check_names() {
        let report = fozzy::CiReport {
            schema_version: "fozzy.ci_report.v1".to_string(),
            ok: true,
            checks: vec![fozzy::CiCheck {
                name: "   ".to_string(),
                ok: true,
                detail: Some("ok".to_string()),
            }],
        };
        let (status, detail) = ci_report_status(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid=1"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn ci_report_status_rejects_unknown_check_names() {
        let report = fozzy::CiReport {
            schema_version: "fozzy.ci_report.v1".to_string(),
            ok: true,
            checks: vec![fozzy::CiCheck {
                name: "mystery_check".to_string(),
                ok: true,
                detail: Some("ok".to_string()),
            }],
        };
        let (status, detail) = ci_report_status(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid=1"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn ci_report_status_rejects_duplicate_check_names() {
        let report = fozzy::CiReport {
            schema_version: "fozzy.ci_report.v1".to_string(),
            ok: true,
            checks: vec![
                fozzy::CiCheck {
                    name: "trace_verify".to_string(),
                    ok: true,
                    detail: Some("ok".to_string()),
                },
                fozzy::CiCheck {
                    name: "trace_verify".to_string(),
                    ok: true,
                    detail: Some("ok again".to_string()),
                },
            ],
        };
        let (status, detail) = ci_report_status(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("duplicate=1"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn ci_report_status_rejects_empty_check_set() {
        let report = fozzy::CiReport {
            schema_version: "fozzy.ci_report.v1".to_string(),
            ok: true,
            checks: vec![],
        };
        let (status, detail) = ci_report_status(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("checks=0"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_surfaces_issue_and_hint() {
        let report = fozzy::DoctorReport {
            ok: false,
            issues: vec![fozzy::DoctorIssue {
                code: "proc_unmatched_preflight".to_string(),
                message: "strict proc backend preflight found an undeclared subprocess".to_string(),
                hint: Some("Add a `proc_when` step".to_string()),
            }],
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: true,
                signatures: vec!["abc".to_string(), "abc".to_string()],
                first_mismatch_run: None,
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("issues=1"));
        assert!(detail.contains(
            "proc_unmatched_preflight: strict proc backend preflight found an undeclared subprocess"
        ));
        assert!(detail.contains("Add a `proc_when` step"));
    }

    #[test]
    fn doctor_report_status_rejects_inconsistent_ok_summary() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: vec![fozzy::DoctorIssue {
                code: "determinism_audit_mismatch".to_string(),
                message: "mismatch".to_string(),
                hint: None,
            }],
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: false,
                signatures: vec!["abc".to_string(), "def".to_string()],
                first_mismatch_run: Some(2),
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("reported_ok=true"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_zero_run_count() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: Vec::new(),
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 0,
                seed: 7,
                consistent: true,
                signatures: Vec::new(),
                first_mismatch_run: None,
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 0, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("runs=0"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_invalid_issue_rows() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: vec![fozzy::DoctorIssue {
                code: "".to_string(),
                message: " ".to_string(),
                hint: None,
            }],
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: true,
                signatures: vec!["abc".to_string(), "abc".to_string()],
                first_mismatch_run: None,
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_issues=1"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_unknown_issue_codes() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: vec![fozzy::DoctorIssue {
                code: "mystery_issue".to_string(),
                message: "unexpected".to_string(),
                hint: None,
            }],
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: true,
                signatures: vec!["abc".to_string(), "abc".to_string()],
                first_mismatch_run: None,
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_issues=1"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_duplicate_issue_rows() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: vec![
                fozzy::DoctorIssue {
                    code: "determinism_audit_mismatch".to_string(),
                    message: "mismatch".to_string(),
                    hint: None,
                },
                fozzy::DoctorIssue {
                    code: "determinism_audit_mismatch".to_string(),
                    message: "mismatch".to_string(),
                    hint: Some("same issue repeated".to_string()),
                },
            ],
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: false,
                signatures: vec!["abc".to_string(), "def".to_string()],
                first_mismatch_run: Some(2),
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("duplicate_issues=1"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_invalid_signal_rows() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: Vec::new(),
            nondeterminism_signals: Some(vec![fozzy::NondeterminismSignal {
                source: "".to_string(),
                detail: "".to_string(),
            }]),
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: true,
                signatures: vec!["abc".to_string(), "abc".to_string()],
                first_mismatch_run: None,
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_signals=1"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_unknown_signal_sources() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: Vec::new(),
            nondeterminism_signals: Some(vec![fozzy::NondeterminismSignal {
                source: "stdout".to_string(),
                detail: "line ordering drift".to_string(),
            }]),
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: true,
                signatures: vec!["abc".to_string(), "abc".to_string()],
                first_mismatch_run: None,
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_signals=1"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_duplicate_signal_rows() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: Vec::new(),
            nondeterminism_signals: Some(vec![
                fozzy::NondeterminismSignal {
                    source: "env".to_string(),
                    detail: "line ordering drift".to_string(),
                },
                fozzy::NondeterminismSignal {
                    source: "env".to_string(),
                    detail: "line ordering drift".to_string(),
                },
            ]),
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: true,
                signatures: vec!["abc".to_string(), "abc".to_string()],
                first_mismatch_run: None,
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("duplicate_signals=1"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_missing_determinism_audit() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: Vec::new(),
            nondeterminism_signals: None,
            determinism_audit: None,
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("audit_present=false"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_incoherent_determinism_audit() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: Vec::new(),
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/other.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: true,
                signatures: vec!["abc".to_string()],
                first_mismatch_run: Some(2),
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("audit_present=true"));
        assert!(detail.contains("audit_valid=false"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_mismatched_determinism_audit_seed() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: Vec::new(),
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 99,
                consistent: true,
                signatures: vec!["abc".to_string(), "abc".to_string()],
                first_mismatch_run: None,
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("audit_valid=false"));
        assert!(detail.contains("seed=7"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_missing_mismatch_issue_for_inconsistent_audit() {
        let report = fozzy::DoctorReport {
            ok: true,
            issues: Vec::new(),
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: false,
                signatures: vec!["abc".to_string(), "def".to_string()],
                first_mismatch_run: Some(2),
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("audit_issue_consistent=false"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn doctor_report_status_rejects_spurious_mismatch_issue_for_consistent_audit() {
        let report = fozzy::DoctorReport {
            ok: false,
            issues: vec![fozzy::DoctorIssue {
                code: "determinism_audit_mismatch".to_string(),
                message: "mismatch".to_string(),
                hint: None,
            }],
            nondeterminism_signals: None,
            determinism_audit: Some(fozzy::DeterminismAudit {
                scenario: "tests/repro.fozzy.json".to_string(),
                runs: 2,
                seed: 7,
                consistent: true,
                signatures: vec!["abc".to_string(), "abc".to_string()],
                first_mismatch_run: None,
            }),
        };
        let scenario = Path::new("tests/repro.fozzy.json");
        let (status, detail) = doctor_report_status(&report, true, scenario, 2, 7);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("audit_issue_consistent=false"));
        assert!(detail.contains("derived_ok=false"));
    }

    #[test]
    fn topology_coverage_status_rejects_degraded_confidence_warnings() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: vec!["/repo/src/broken.rs: failed to open".to_string()],
            unreadable_scenarios: Vec::new(),
            warnings: vec![
                "map scan skipped 1 source file(s); hotspot coverage is incomplete".to_string(),
            ],
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: Vec::new(),
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("uncovered=0"));
        assert!(detail.contains(
            "warnings=map scan skipped 1 source file(s); hotspot coverage is incomplete"
        ));
    }

    #[test]
    fn topology_coverage_status_rejects_mismatched_report_identities() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "run_record_replay_ci".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_coverage_status(
            &report,
            Path::new("/other"),
            Path::new("/other/tests"),
            TopologyProfile::Balanced,
            ShrinkCoveragePolicy::FailureOnly,
            50,
        );
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("root_ok=false"));
        assert!(detail.contains("scenario_root_ok=false"));
        assert!(detail.contains("profile_ok=false"));
        assert!(detail.contains("shrink_policy_ok=false"));
        assert!(detail.contains("base_min_risk_ok=false"));
    }

    #[test]
    fn topology_coverage_status_rejects_empty_returned_suites() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 0,
            returned_suites: 0,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: Vec::new(),
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("returned_suites=0"));
    }

    #[test]
    fn topology_coverage_status_rejects_zero_required_hotspots() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 0,
            covered_hotspot_count: 0,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "run_record_replay_ci".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("required_hotspots=0"));
    }

    #[test]
    fn topology_coverage_status_rejects_inconsistent_hotspot_math() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 2,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "run_record_replay_ci".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["high risk".to_string()],
                reasons: vec!["host proc".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("hotspot_math_ok=false"));
    }

    #[test]
    fn topology_coverage_status_rejects_duplicate_returned_suite_hotspots() {
        let suite = fozzy::SuiteRecommendation {
            hotspot_id: "hs-1".to_string(),
            component: "runtime".to_string(),
            path: "src/runtime.rs".to_string(),
            risk_score: 90,
            required_by_policy: true,
            covered: true,
            coverage_hints: vec!["run_record_replay_ci".to_string()],
            required_suites: vec!["run_record_replay_ci".to_string()],
            covered_suites: vec!["run_record_replay_ci".to_string()],
            coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                suite: "run_record_replay_ci".to_string(),
                matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                reason: "matched hotspot token".to_string(),
            }],
            missing_required_suites: Vec::new(),
            why_required: vec!["high risk".to_string()],
            reasons: vec!["host proc".to_string()],
            recommended_suites: vec!["run_record_replay_ci".to_string()],
        };
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 2,
            covered_hotspot_count: 2,
            uncovered_hotspot_count: 0,
            total_suites: 2,
            returned_suites: 2,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![suite.clone(), suite],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_rejects_blank_suite_component() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "   ".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "run_record_replay_ci".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_rejects_invalid_coverage_evidence_rows() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "".to_string(),
                    matched_scenarios: vec!["   ".to_string()],
                    reason: "".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_rejects_duplicate_coverage_evidence_rows() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![
                    fozzy::SuiteCoverageEvidence {
                        suite: "run_record_replay_ci".to_string(),
                        matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                        reason: "matched hotspot token".to_string(),
                    },
                    fozzy::SuiteCoverageEvidence {
                        suite: "run_record_replay_ci".to_string(),
                        matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                        reason: "matched hotspot token".to_string(),
                    },
                ],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_rejects_duplicate_matched_scenarios_within_coverage_evidence() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "run_record_replay_ci".to_string(),
                    matched_scenarios: vec![
                        "tests/example.fozzy.json".to_string(),
                        "tests/example.fozzy.json".to_string(),
                    ],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_rejects_covered_suites_without_matching_evidence() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "host_backends_run".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec![
                    "run_record_replay_ci".to_string(),
                    "host_backends_run".to_string(),
                ],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_rejects_unknown_required_suite_names() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["unknown_suite".to_string()],
                covered_suites: vec!["unknown_suite".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "unknown_suite".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["unknown_suite".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_rejects_blank_or_duplicate_coverage_hints() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["host proc".to_string(), "host proc".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "run_record_replay_ci".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_rejects_recommended_suites_missing_required_entries() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "run_record_replay_ci".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["host_backends_run".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_rejects_inconsistent_suite_coverage_sets() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: vec!["host_backends_run".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "host_backends_run".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: vec!["run_record_replay_ci".to_string()],
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

    #[test]
    fn topology_coverage_status_allows_non_required_suite_with_missing_coverage() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 40,
                required_by_policy: false,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec!["run_record_replay_ci".to_string()],
                covered_suites: Vec::new(),
                coverage_evidence: Vec::new(),
                missing_required_suites: vec!["run_record_replay_ci".to_string()],
                why_required: vec!["below threshold".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Passed));
        assert!(detail.contains("invalid_suites=0"));
    }

    #[test]
    fn topology_coverage_status_rejects_duplicate_suite_list_entries() {
        let report = fozzy::MapSuitesReport {
            schema_version: "fozzy.map_suites.v5".to_string(),
            root: "/repo".to_string(),
            scenario_root: "/repo/tests".to_string(),
            scanned_files: 10,
            profile: TopologyProfile::Pedantic,
            shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
            base_min_risk: 60,
            effective_min_risk: 55,
            scenario_count: 1,
            skipped_source_files: Vec::new(),
            unreadable_scenarios: Vec::new(),
            warnings: Vec::new(),
            required_hotspot_count: 1,
            covered_hotspot_count: 1,
            uncovered_hotspot_count: 0,
            total_suites: 1,
            returned_suites: 1,
            offset: 0,
            limit: 25,
            truncated: false,
            suites: vec![fozzy::SuiteRecommendation {
                hotspot_id: "hs-1".to_string(),
                component: "runtime".to_string(),
                path: "src/runtime.rs".to_string(),
                risk_score: 90,
                required_by_policy: true,
                covered: true,
                coverage_hints: vec!["run_record_replay_ci".to_string()],
                required_suites: vec![
                    "run_record_replay_ci".to_string(),
                    "run_record_replay_ci".to_string(),
                ],
                covered_suites: vec!["run_record_replay_ci".to_string()],
                coverage_evidence: vec![fozzy::SuiteCoverageEvidence {
                    suite: "run_record_replay_ci".to_string(),
                    matched_scenarios: vec!["tests/example.fozzy.json".to_string()],
                    reason: "matched hotspot token".to_string(),
                }],
                missing_required_suites: Vec::new(),
                why_required: vec!["policy hotspot".to_string()],
                reasons: vec!["runtime risk".to_string()],
                recommended_suites: vec!["run_record_replay_ci".to_string()],
            }],
        };
        let (status, detail) = topology_status_for_report(&report);
        assert!(matches!(status, FullStepStatus::Failed));
        assert!(detail.contains("invalid_suites=1"));
    }

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
        let other_trace =
            std::env::temp_dir().join(format!("other-{}.fozzy", uuid::Uuid::new_v4()));
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
