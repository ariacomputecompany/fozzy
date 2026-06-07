//! Topology and hotspot mapping commands (`fozzy map ...`).

#[path = "map/attribution.rs"]
mod attribution;
#[path = "map/dispatch.rs"]
mod dispatch;
#[path = "map/policy.rs"]
mod policy;
#[path = "map/repo.rs"]
mod repo;
#[path = "map/scenario.rs"]
mod scenario;
#[path = "map/types.rs"]
mod types;

pub use dispatch::{map_command, map_suites};
pub use types::*;

#[allow(unused_imports)]
pub(crate) use attribution::{
    AttributionHints, covered_suites_for_hotspot, suite_allows_attribution_match, tokenize,
};
#[allow(unused_imports)]
pub(crate) use policy::{
    SUITE_EXPLORE, SUITE_FUZZ, SUITE_HOST, SUITE_MEMORY, SUITE_RUN_REPLAY_CI,
    SUITE_SHRINK_EXERCISED, SUITE_SHRINK_FAILURE, SUITE_TEST_DET, effective_min_risk,
    recommended_suites_for_hotspot, required_suites_for_hotspot, why_required,
};
#[allow(unused_imports)]
pub(crate) use repo::{
    accumulate_signals_line, component_for_path, count_hits, hotspot_hints, is_candidate_file,
    scan_repo, score_signals, should_skip_path,
};
#[allow(unused_imports)]
pub(crate) use scenario::{
    ScenarioCoverageIndex, build_scenario_facts, discover_scenarios, matches_suite_signal,
};

#[cfg(test)]
#[path = "map/tests.rs"]
mod tests;
