use super::*;

pub(crate) fn topology_coverage_status(
    report: &fozzy::MapSuitesReport,
    expected_root: &Path,
    expected_scenario_root: &Path,
    expected_profile: TopologyProfile,
    expected_shrink_policy: ShrinkCoveragePolicy,
    expected_base_min_risk: u8,
) -> (FullStepStatus, String) {
    fn known_topology_suite(name: &str) -> bool {
        matches!(
            name,
            "test_det"
                | "run_record_replay_ci"
                | "fuzz_inputs"
                | "explore_schedule_faults"
                | "host_backends_run"
                | "memory_graph_diff_top"
                | "shrink_exercised"
                | "shrink_failure_trace"
        )
    }

    let warnings = if report.warnings.is_empty() {
        "<none>".to_string()
    } else {
        report.warnings.join("; ")
    };
    let root_ok = report.root == expected_root.display().to_string();
    let scenario_root_ok = report.scenario_root == expected_scenario_root.display().to_string();
    let profile_ok = report.profile == expected_profile;
    let shrink_policy_ok = report.shrink_policy == expected_shrink_policy;
    let base_min_risk_ok = report.base_min_risk == expected_base_min_risk;
    let hotspot_math_ok = report.covered_hotspot_count <= report.required_hotspot_count
        && report.uncovered_hotspot_count
            == report
                .required_hotspot_count
                .saturating_sub(report.covered_hotspot_count);
    let pagination_math_ok = report.returned_suites == report.suites.len()
        && report.returned_suites <= report.total_suites
        && report.returned_suites <= report.limit
        && if report.truncated {
            report.offset.saturating_add(report.returned_suites) < report.total_suites
        } else {
            report.offset.saturating_add(report.returned_suites) >= report.total_suites
        };
    let mut seen_hotspots = std::collections::BTreeSet::new();
    let invalid_suites = report
        .suites
        .iter()
        .filter(|suite| {
            let mut seen_coverage_evidence = std::collections::BTreeSet::new();
            let mut evidence_suite_set = std::collections::BTreeSet::new();
            let invalid_coverage_evidence = suite.coverage_evidence.iter().any(|evidence| {
                let suite_name = evidence.suite.trim();
                let reason = evidence.reason.trim();
                let mut seen_matched_scenarios = std::collections::BTreeSet::new();
                let invalid_matched_scenarios = evidence.matched_scenarios.iter().any(|scenario| {
                    let scenario = scenario.trim();
                    scenario.is_empty() || !seen_matched_scenarios.insert(scenario.to_string())
                });
                if !suite_name.is_empty() {
                    evidence_suite_set.insert(suite_name.to_string());
                }
                let duplicate_coverage_evidence = !suite_name.is_empty()
                    && !reason.is_empty()
                    && !invalid_matched_scenarios
                    && !evidence.matched_scenarios.is_empty()
                    && !seen_coverage_evidence.insert(format!(
                        "{suite_name}\u{0}{reason}\u{0}{}",
                        evidence
                            .matched_scenarios
                            .iter()
                            .map(|scenario| scenario.trim())
                            .collect::<Vec<_>>()
                            .join("\u{0}")
                    ));
                suite_name.is_empty()
                    || !known_topology_suite(suite_name)
                    || reason.is_empty()
                    || evidence.matched_scenarios.is_empty()
                    || invalid_matched_scenarios
                    || duplicate_coverage_evidence
            });
            let required_set = suite
                .required_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .collect::<std::collections::BTreeSet<_>>();
            let required_duplicates = suite
                .required_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .count()
                != required_set.len();
            let covered_set = suite
                .covered_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .collect::<std::collections::BTreeSet<_>>();
            let covered_duplicates = suite
                .covered_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .count()
                != covered_set.len();
            let missing_set = suite
                .missing_required_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .collect::<std::collections::BTreeSet<_>>();
            let missing_duplicates = suite
                .missing_required_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .count()
                != missing_set.len();
            let recommended_set = suite
                .recommended_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .collect::<std::collections::BTreeSet<_>>();
            let suite_math_invalid = suite.required_suites.iter().any(|suite| {
                let suite = suite.trim();
                suite.is_empty() || !known_topology_suite(suite)
            }) || suite.covered_suites.iter().any(|suite| {
                let suite = suite.trim();
                suite.is_empty() || !known_topology_suite(suite)
            }) || suite.missing_required_suites.iter().any(|suite| {
                let suite = suite.trim();
                suite.is_empty() || !known_topology_suite(suite)
            }) || required_duplicates
                || covered_duplicates
                || missing_duplicates
                || suite.recommended_suites.iter().any(|suite| {
                    let suite = suite.trim();
                    suite.is_empty() || !known_topology_suite(suite)
                })
                || suite
                    .coverage_hints
                    .iter()
                    .any(|hint| hint.trim().is_empty())
                || suite
                    .coverage_hints
                    .iter()
                    .map(|hint| hint.trim())
                    .filter(|hint| !hint.is_empty())
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != suite
                        .coverage_hints
                        .iter()
                        .map(|hint| hint.trim())
                        .filter(|hint| !hint.is_empty())
                        .count()
                || recommended_set.len() != suite.recommended_suites.len()
                || !covered_set.is_subset(&required_set)
                || !missing_set.is_subset(&required_set)
                || !covered_set.is_disjoint(&missing_set)
                || !required_set.is_subset(&recommended_set)
                || covered_set
                    != evidence_suite_set
                        .iter()
                        .map(|suite| suite.as_str())
                        .collect::<std::collections::BTreeSet<_>>()
                || required_set
                    != covered_set
                        .union(&missing_set)
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                || suite.covered != (!suite.required_by_policy || missing_set.is_empty());
            suite.hotspot_id.trim().is_empty()
                || suite.component.trim().is_empty()
                || suite.path.trim().is_empty()
                || invalid_coverage_evidence
                || suite_math_invalid
                || !seen_hotspots.insert(suite.hotspot_id.trim().to_string())
        })
        .count();
    let ok = report.uncovered_hotspot_count == 0
        && report.required_hotspot_count > 0
        && report.warnings.is_empty()
        && root_ok
        && scenario_root_ok
        && profile_ok
        && shrink_policy_ok
        && base_min_risk_ok
        && hotspot_math_ok
        && pagination_math_ok
        && report.returned_suites > 0
        && invalid_suites == 0;
    (
        if ok {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "required_hotspots={} covered={} uncovered={} root_ok={} scenario_root_ok={} profile_ok={} shrink_policy_ok={} base_min_risk_ok={} hotspot_math_ok={} total_suites={} returned_suites={} offset={} limit={} truncated={} pagination_math_ok={} invalid_suites={} min_risk={} profile={} root={} scenario_root={} warnings={}",
            report.required_hotspot_count,
            report.covered_hotspot_count,
            report.uncovered_hotspot_count,
            root_ok,
            scenario_root_ok,
            profile_ok,
            shrink_policy_ok,
            base_min_risk_ok,
            hotspot_math_ok,
            report.total_suites,
            report.returned_suites,
            report.offset,
            report.limit,
            report.truncated,
            pagination_math_ok,
            invalid_suites,
            report.effective_min_risk,
            format!("{:?}", report.profile).to_lowercase(),
            report.root,
            report.scenario_root,
            warnings
        ),
    )
}
