use std::collections::BTreeSet;

use super::{HotspotSignals, ShrinkCoveragePolicy, TopologyProfile};

pub(crate) const SUITE_TEST_DET: &str = "test_det";
pub(crate) const SUITE_RUN_REPLAY_CI: &str = "run_record_replay_ci";
pub(crate) const SUITE_FUZZ: &str = "fuzz_inputs";
pub(crate) const SUITE_EXPLORE: &str = "explore_schedule_faults";
pub(crate) const SUITE_HOST: &str = "host_backends_run";
pub(crate) const SUITE_MEMORY: &str = "memory_graph_diff_top";
pub(crate) const SUITE_SHRINK_EXERCISED: &str = "shrink_exercised";
pub(crate) const SUITE_SHRINK_FAILURE: &str = "shrink_failure_trace";

pub(crate) fn effective_min_risk(base: u8, profile: TopologyProfile) -> u8 {
    match profile {
        TopologyProfile::Balanced => base.saturating_add(15).min(100),
        TopologyProfile::Pedantic => base.saturating_sub(5),
        TopologyProfile::Overkill => base.saturating_sub(15),
    }
}

pub(crate) fn required_suites_for_hotspot(
    profile: TopologyProfile,
    shrink_policy: ShrinkCoveragePolicy,
    signals: &HotspotSignals,
    has_known_shrink_failure: bool,
) -> Vec<String> {
    let mut out = BTreeSet::<String>::new();
    out.insert(SUITE_TEST_DET.to_string());
    out.insert(SUITE_RUN_REPLAY_CI.to_string());
    let mut require_shrink_exercised = false;
    let mut require_shrink_failure = false;

    match profile {
        TopologyProfile::Balanced => {
            if signals.concurrency_signals > 0 {
                out.insert(SUITE_EXPLORE.to_string());
            }
            if signals.external_signals > 0 {
                out.insert(SUITE_HOST.to_string());
            }
            if signals.failure_signals > 0 || signals.branch_signals > 25 {
                require_shrink_exercised = true;
            }
            if signals.failure_signals > 0 {
                require_shrink_failure = true;
            }
            if signals.memory_signals > 2 {
                out.insert(SUITE_MEMORY.to_string());
            }
            if signals.branch_signals > 20 {
                out.insert(SUITE_FUZZ.to_string());
            }
        }
        TopologyProfile::Pedantic => {
            require_shrink_exercised = true;
            if signals.failure_signals > 0 {
                require_shrink_failure = true;
            }
            if signals.concurrency_signals > 0 || signals.failure_signals >= 4 {
                out.insert(SUITE_EXPLORE.to_string());
            }
            if signals.external_signals > 0 || signals.entrypoint_signals > 0 {
                out.insert(SUITE_HOST.to_string());
            }
            if signals.memory_signals > 0 {
                out.insert(SUITE_MEMORY.to_string());
            }
            if signals.branch_signals > 6 || signals.failure_signals > 0 {
                out.insert(SUITE_FUZZ.to_string());
            }
        }
        TopologyProfile::Overkill => {
            out.insert(SUITE_FUZZ.to_string());
            out.insert(SUITE_EXPLORE.to_string());
            out.insert(SUITE_HOST.to_string());
            out.insert(SUITE_MEMORY.to_string());
            require_shrink_exercised = true;
            require_shrink_failure = true;
        }
    }
    if require_shrink_failure {
        require_shrink_exercised = true;
    }
    if require_shrink_exercised {
        out.insert(SUITE_SHRINK_EXERCISED.to_string());
    }
    if require_shrink_failure {
        match shrink_policy {
            ShrinkCoveragePolicy::FailureOnly => {
                out.insert(SUITE_SHRINK_FAILURE.to_string());
            }
            ShrinkCoveragePolicy::ExercisedOk => {}
            ShrinkCoveragePolicy::NoKnownFailures => {
                if has_known_shrink_failure {
                    out.insert(SUITE_SHRINK_FAILURE.to_string());
                }
            }
        }
    }

    out.into_iter().collect()
}

pub(crate) fn recommended_suites_for_hotspot(signals: &HotspotSignals) -> Vec<String> {
    let mut out = BTreeSet::<String>::new();
    out.insert(SUITE_TEST_DET.to_string());
    out.insert(SUITE_RUN_REPLAY_CI.to_string());
    if signals.concurrency_signals > 0 {
        out.insert(SUITE_EXPLORE.to_string());
    }
    if signals.external_signals > 0 {
        out.insert(SUITE_HOST.to_string());
    }
    if signals.failure_signals > 0 {
        out.insert(SUITE_SHRINK_FAILURE.to_string());
    }
    if signals.failure_signals > 0 || signals.branch_signals > 20 {
        out.insert(SUITE_SHRINK_EXERCISED.to_string());
    }
    if signals.memory_signals > 0 {
        out.insert(SUITE_MEMORY.to_string());
    }
    if signals.branch_signals > 8 {
        out.insert(SUITE_FUZZ.to_string());
    }
    out.into_iter().collect()
}

pub(crate) fn why_required(risk: u8, threshold: u8, signals: &HotspotSignals) -> Vec<String> {
    let mut out = Vec::<String>::new();
    if risk >= threshold {
        out.push(format!("risk_score {} >= threshold {}", risk, threshold));
    }
    if signals.concurrency_signals > 0 {
        out.push("concurrency hotspot".to_string());
    }
    if signals.external_signals > 0 {
        out.push("external side-effects present".to_string());
    }
    if signals.failure_signals > 0 {
        out.push("failure/retry/timeout behavior present".to_string());
    }
    if signals.memory_signals > 0 {
        out.push("memory behavior present".to_string());
    }
    out
}
