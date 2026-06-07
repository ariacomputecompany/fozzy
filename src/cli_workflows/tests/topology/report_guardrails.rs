use super::*;

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
    assert!(detail
        .contains("warnings=map scan skipped 1 source file(s); hotspot coverage is incomplete"));
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
