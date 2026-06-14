use super::state::{FullRunState, ScenarioSelection};
use super::*;

pub(super) fn prepare_scenarios(
    state: &mut FullRunState,
    config: &Config,
    scenario_root: &Path,
    seed: Option<u64>,
    doctor_runs: u32,
    scenario_filter: Option<&str>,
    skip_steps: &[String],
    required_steps: &[String],
    require_topology_coverage: Option<&Path>,
    topology_min_risk: u8,
    topology_profile: TopologyProfile,
    topology_shrink_policy: ShrinkCoveragePolicy,
) -> Option<ScenarioSelection> {
    if state.strict {
        match git_clean_tree_check() {
            Ok(check) => {
                let (status, detail) = clean_tree_step_status(&check);
                state.push("clean_tree", status, detail);
            }
            Err(err) => state.push("clean_tree", FullStepStatus::Failed, err.to_string()),
        }
    } else {
        state.push_skipped("clean_tree", "strict disabled; git worktree check skipped");
    }

    if let Some(conflict) = full_policy_conflict_details(
        skip_steps,
        required_steps,
        require_topology_coverage.is_some(),
    ) {
        state.push("policy_conflict", FullStepStatus::Failed, conflict);
    }

    let usage = fozzy::usage_doc();
    state.push(
        "usage",
        if usage.items.is_empty() {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        format!("items={}", usage.items.len()),
    );
    let version = fozzy::version_info();
    state.push(
        "version",
        FullStepStatus::Passed,
        format!("version={}", version.version),
    );

    run_init_check(state, config, doctor_runs, seed);

    state.start_step(
        "discover_scenarios",
        format!("scanning {}", scenario_root.display()),
    );
    let mut discovered = match run_with_timeout("discover_scenarios", DISCOVERY_TIMEOUT, {
        let scenario_root = scenario_root.to_path_buf();
        move || Ok(discover_scenarios(&scenario_root))
    }) {
        Ok(discovered) => discovered,
        Err(err) => {
            state.abort_due_to_timeout(
                "discover_scenarios",
                err.to_string(),
                format!(
                    "Scenario discovery did not complete under {}; `fozzy full --json` aborted with a structured failure instead of hanging silently.",
                    scenario_root.display()
                ),
            );
            return None;
        }
    };
    if let Some(filter) = scenario_filter {
        if !filter.is_empty() {
            discovered
                .steps
                .retain(|path| path.to_string_lossy().contains(filter));
            discovered
                .distributed
                .retain(|path| path.to_string_lossy().contains(filter));
        }
    }

    let parse_error_count = discovered.parse_errors.len();
    state.push(
        "discover_scenarios",
        if parse_error_count > 0 || discovered.steps.is_empty() {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        format!(
            "discovered step_scenarios={} distributed_scenarios={} parse_errors={}",
            discovered.steps.len(),
            discovered.distributed.len(),
            parse_error_count
        ),
    );
    if parse_error_count > 0 {
        state.guidance.push(format!(
            "Fix malformed scenarios before trusting `fozzy full` coverage: {}",
            discovered.parse_errors.join(" | ")
        ));
    } else if discovered.steps.is_empty() {
        state.guidance.push(
            "Add at least one executable step scenario under the selected scenario root before trusting `fozzy full` coverage; distributed-only roots cannot exercise the deterministic run/test/trace surface.".to_string(),
        );
    }

    push_topology_step(
        state,
        scenario_root,
        &discovered,
        require_topology_coverage,
        topology_min_risk,
        topology_profile,
        topology_shrink_policy,
    );
    if state.should_abort() {
        return None;
    }

    let step = discovered
        .steps
        .iter()
        .find(|path| is_preferred_step_scenario(path))
        .cloned()
        .or_else(|| discovered.steps.first().cloned());
    let host_step = discovered
        .steps
        .iter()
        .find(|path| is_preferred_host_step_scenario(path))
        .cloned();
    let distributed = discovered
        .distributed
        .iter()
        .find(|path| is_preferred_distributed_scenario(path))
        .cloned()
        .or_else(|| discovered.distributed.first().cloned());

    Some(ScenarioSelection {
        discovered,
        step,
        host_step,
        distributed,
        memory: MemoryOptions {
            track: true,
            limit_mb: config.mem_limit_mb,
            fail_after_allocs: config.mem_fail_after,
            fail_on_leak: config.fail_on_leak,
            leak_budget_bytes: config.leak_budget,
            artifacts: true,
            fragmentation_seed: config.mem_fragmentation_seed,
            pressure_wave: config.mem_pressure_wave.clone(),
        },
    })
}

fn run_init_check(
    state: &mut FullRunState,
    config: &Config,
    _doctor_runs: u32,
    _seed: Option<u64>,
) {
    let init_tmp = state.register_temp(
        std::env::temp_dir().join(format!("fozzy-full-init-{}", uuid::Uuid::new_v4())),
    );
    let init_status = (|| -> anyhow::Result<String> {
        std::fs::create_dir_all(&init_tmp)?;
        let prev = std::env::current_dir()?;
        std::env::set_current_dir(&init_tmp)?;
        let cfg = Config::load_optional_checked(Path::new("fozzy.toml"))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let init_res = fozzy::init_project(
            &cfg,
            Path::new("fozzy.toml"),
            &InitTemplate::Rust,
            true,
            &selected_init_test_types(&[], true),
        );
        let restore_res = std::env::set_current_dir(prev);
        if let Err(err) = restore_res {
            return Err(anyhow::anyhow!(
                "failed to restore cwd after init check: {err}"
            ));
        }
        let _ = config;
        init_res?;
        let example = init_tmp.join("tests/example.fozzy.json");
        if !example.exists() {
            return Err(anyhow::anyhow!(
                "expected init scaffold missing: {}",
                example.display()
            ));
        }
        Ok(format!("workspace={}", init_tmp.display()))
    })();
    match init_status {
        Ok(detail) => state.push("init", FullStepStatus::Passed, detail),
        Err(err) => state.push("init", FullStepStatus::Failed, err.to_string()),
    }
}

fn push_topology_step(
    state: &mut FullRunState,
    scenario_root: &Path,
    _discovered: &FullScenarioDiscovery,
    require_topology_coverage: Option<&Path>,
    topology_min_risk: u8,
    topology_profile: TopologyProfile,
    topology_shrink_policy: ShrinkCoveragePolicy,
) {
    if let Some(root) = require_topology_coverage {
        state.start_step(
            "topology_coverage",
            format!(
                "mapping suites for root={} scenario_root={}",
                root.display(),
                scenario_root.display()
            ),
        );
        let options = MapSuitesOptions {
            root: root.to_path_buf(),
            scenario_root: scenario_root.to_path_buf(),
            min_risk: topology_min_risk,
            profile: topology_profile,
            shrink_policy: topology_shrink_policy,
            limit: 200,
            offset: 0,
            max_matched_scenarios: 25,
        };
        match run_with_timeout("topology_coverage", TOPOLOGY_TIMEOUT, move || {
            Ok(fozzy::map_suites(&options)?)
        }) {
            Ok(report) => {
                let (status, detail) = topology_coverage_status(
                    &report,
                    root,
                    scenario_root,
                    topology_profile,
                    topology_shrink_policy,
                    topology_min_risk,
                );
                state.push("topology_coverage", status, detail);
            }
            Err(err) => state.abort_due_to_timeout(
                "topology_coverage",
                err.to_string(),
                "Suite-map coverage did not finish in time; investigate repo scan size, scenario parsing cost, or disable topology coverage intentionally."
                    .to_string(),
            ),
        }
    } else {
        state.push_skipped(
            "topology_coverage",
            "not requested (use --require-topology-coverage <repo_root>)",
        );
    }
}
