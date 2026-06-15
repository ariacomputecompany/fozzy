use crate::FullStepStatus;
use crate::cli_workflows::*;
use std::path::Path;

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

mod report_guardrails;
mod suite_consistency;
