use super::gate::clean_tree_step_status;
use super::*;

mod extended_surface;
mod prepare;
mod state;
mod trace_surface;

use state::FullRunState;

pub(super) const DISCOVERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
pub(super) const TOPOLOGY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
pub(super) const EXECUTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

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
    stream_json_events: bool,
) -> anyhow::Result<FullReport> {
    let seed = Some(resolved_workflow_seed(seed));
    let mut state = FullRunState::new(strict, unsafe_mode, scenario_root, stream_json_events);
    state.start_phase(
        "prepare",
        format!("scenario_root={}", scenario_root.display()),
    );
    let Some(selection) = prepare::prepare_scenarios(
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
    ) else {
        return Ok(state.finish(skip_steps, required_steps));
    };
    if state.should_abort() {
        return Ok(state.finish(skip_steps, required_steps));
    }
    state.start_phase(
        "deterministic_surface",
        "running doctor/test/run trace chain",
    );
    let mut run_state = trace_surface::run_deterministic_surface(
        &mut state,
        config,
        &selection,
        seed,
        doctor_runs,
        strict,
    );
    if state.should_abort() {
        return Ok(state.finish(skip_steps, required_steps));
    }
    state.start_phase(
        "trace_surface",
        "verifying replay, ci, shrink, and artifact surfaces",
    );
    trace_surface::run_trace_surface(
        &mut state,
        config,
        &mut run_state,
        strict,
        allow_expected_failures,
        seed,
    );
    if state.should_abort() {
        return Ok(state.finish(skip_steps, required_steps));
    }
    state.start_phase(
        "extended_surface",
        "running fuzz, explore, corpus, host backend, and env checks",
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

pub(super) fn run_with_timeout<T, F>(
    label: &str,
    timeout: std::time::Duration,
    op: F,
) -> anyhow::Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(op());
    });
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(anyhow::anyhow!(
            "phase `{label}` timed out after {}ms",
            timeout.as_millis()
        )),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(anyhow::anyhow!(
            "phase `{label}` ended without returning a result"
        )),
    }
}
