//! Deterministic distributed exploration runner (single-host simulation).

#[path = "explore/exec.rs"]
mod exec;
#[path = "explore/flows.rs"]
mod flows;
#[path = "explore/invariants.rs"]
mod invariants;
#[path = "explore/network.rs"]
mod network;
#[path = "explore/scenario.rs"]
mod scenario;
#[path = "explore/types.rs"]
mod types;
#[path = "explore/utils.rs"]
mod utils;

pub use flows::{explore, replay_explore_trace, shrink_explore_trace};
pub(crate) use scenario::{distributed_to_explore, execute_explore_for_fuzz};
pub use types::{ExploreOptions, ExploreTrace, ScenarioV1Explore, ScheduleStrategy};
