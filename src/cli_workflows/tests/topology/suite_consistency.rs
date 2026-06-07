use super::*;

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
