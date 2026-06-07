use super::gate::clean_tree_step_status;
use super::*;

mod extended_surface;
mod prepare;
mod state;
mod trace_surface;

use state::FullRunState;

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_full_command(
    config: &Config,
    scenario_root: &Path,
    seed: Option<u64>,
    doctor_runs: u32,
    fuzz_time: std::time::Duration,
    explore_steps: u64,
    explore_nodes: usize,
    strict: bool,
    unsafe_mode: bool,
    allow_expected_failures: bool,
    scenario_filter: Option<&str>,
    skip_steps: &[String],
    required_steps: &[String],
    require_topology_coverage: Option<&Path>,
    topology_min_risk: u8,
    topology_profile: TopologyProfile,
    topology_shrink_policy: ShrinkCoveragePolicy,
) -> anyhow::Result<FullReport> {
    let mut state = FullRunState::new(strict, unsafe_mode, scenario_root);
    let selection = prepare::prepare_scenarios(
        &mut state,
        config,
        scenario_root,
        seed,
        doctor_runs,
        scenario_filter,
        skip_steps,
        required_steps,
        require_topology_coverage,
        topology_min_risk,
        topology_profile,
        topology_shrink_policy,
    );
    let mut run_state = trace_surface::run_deterministic_surface(
        &mut state,
        config,
        &selection,
        seed,
        doctor_runs,
        strict,
    );
    trace_surface::run_trace_surface(
        &mut state,
        config,
        &mut run_state,
        strict,
        allow_expected_failures,
        seed,
    );
    extended_surface::run_extended_surface(
        &mut state,
        config,
        &selection,
        seed,
        fuzz_time,
        explore_steps,
        explore_nodes,
        strict,
    );
    Ok(state.finish(skip_steps, required_steps))
}
