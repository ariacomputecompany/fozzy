use std::time::Duration;

use crate::{
    DistributedInvariant, DistributedStep, ExitStatus, Finding, FozzyError, FozzyResult,
    ScenarioFile, ScenarioPath, ScenarioV1Distributed, TraceEvent,
};

use super::exec::run_explore_inner;
use super::types::{ScenarioV1Explore, ScheduleStrategy};

fn is_shrinkable_setup_step(step: &DistributedStep) -> bool {
    matches!(
        step,
        DistributedStep::Partition { .. }
            | DistributedStep::Heal { .. }
            | DistributedStep::Crash { .. }
            | DistributedStep::Restart { .. }
            | DistributedStep::Tick { .. }
    )
}

pub(super) fn shrinkable_setup_step(step: &DistributedStep) -> bool {
    is_shrinkable_setup_step(step)
}

pub(super) fn load_explore_scenario(
    path: &ScenarioPath,
    nodes_override: Option<usize>,
) -> FozzyResult<ScenarioV1Explore> {
    let bytes = std::fs::read(path.as_path())?;
    let file: ScenarioFile = serde_json::from_slice(&bytes)?;
    let ScenarioFile::Distributed(d) = file else {
        return Err(FozzyError::Scenario(format!(
            "scenario file {} is not a distributed scenario (use `distributed` section)",
            path.as_path().display()
        )));
    };
    d.validate()?;
    distributed_to_explore(d, nodes_override)
}

pub(crate) fn distributed_to_explore(
    d: ScenarioV1Distributed,
    nodes_override: Option<usize>,
) -> FozzyResult<ScenarioV1Explore> {
    d.validate()?;
    let nodes = if let Some(n) = nodes_override {
        (0..n).map(|i| format!("n{i}")).collect()
    } else if let Some(nodes) = d.distributed.nodes.clone() {
        nodes
    } else if let Some(n) = d.distributed.node_count {
        (0..n).map(|i| format!("n{i}")).collect()
    } else {
        unreachable!("distributed validation requires nodes or node_count")
    };

    Ok(ScenarioV1Explore {
        version: 1,
        name: d.name,
        nodes,
        steps: d.distributed.steps,
        invariants: d.distributed.invariants,
    })
}

pub(crate) fn execute_explore_for_fuzz(
    scenario: &ScenarioV1Explore,
    seed: u64,
) -> FozzyResult<(ExitStatus, Vec<Finding>, Vec<TraceEvent>)> {
    let (status, findings, events, _, _) = run_explore_inner(
        scenario,
        seed,
        ScheduleStrategy::CoverageGuided,
        Some(200),
        None,
    )?;
    Ok((status, findings, events))
}

pub(super) fn apply_faults_preset(
    scenario: &mut ScenarioV1Explore,
    faults: Option<&str>,
) -> FozzyResult<()> {
    let Some(faults) = faults else {
        return Ok(());
    };
    let mut injected = Vec::new();
    for token in faults.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        match token {
            "none" => {}
            "partition-first-two" => {
                if scenario.nodes.len() < 2 {
                    return Err(FozzyError::Scenario(
                        "fault preset partition-first-two requires at least 2 nodes".to_string(),
                    ));
                }
                injected.push(DistributedStep::Partition {
                    a: scenario.nodes[0].clone(),
                    b: scenario.nodes[1].clone(),
                });
            }
            "heal-first-two" => {
                if scenario.nodes.len() < 2 {
                    return Err(FozzyError::Scenario(
                        "fault preset heal-first-two requires at least 2 nodes".to_string(),
                    ));
                }
                injected.push(DistributedStep::Heal {
                    a: scenario.nodes[0].clone(),
                    b: scenario.nodes[1].clone(),
                });
            }
            "crash-first" => {
                if scenario.nodes.is_empty() {
                    return Err(FozzyError::Scenario(
                        "fault preset crash-first requires at least 1 node".to_string(),
                    ));
                }
                injected.push(DistributedStep::Crash {
                    node: scenario.nodes[0].clone(),
                });
            }
            "restart-first" => {
                if scenario.nodes.is_empty() {
                    return Err(FozzyError::Scenario(
                        "fault preset restart-first requires at least 1 node".to_string(),
                    ));
                }
                injected.push(DistributedStep::Restart {
                    node: scenario.nodes[0].clone(),
                });
            }
            other => {
                return Err(FozzyError::InvalidArgument(format!(
                    "unknown --faults preset {other:?} (supported: none,partition-first-two,heal-first-two,crash-first,restart-first)"
                )));
            }
        }
    }
    if !injected.is_empty() {
        let mut merged = injected;
        merged.extend(std::mem::take(&mut scenario.steps));
        scenario.steps = merged;
    }
    Ok(())
}

pub(super) fn apply_checker_override(
    scenario: &mut ScenarioV1Explore,
    checker: Option<&str>,
) -> FozzyResult<()> {
    let Some(checker) = checker else {
        return Ok(());
    };
    let mut parsed = Vec::new();
    for token in checker.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        parsed.push(parse_checker_token(token)?);
    }
    if parsed.is_empty() {
        return Err(FozzyError::InvalidArgument(
            "empty --checker override; provide at least one checker token".to_string(),
        ));
    }
    scenario.invariants = parsed;
    Ok(())
}

fn parse_checker_token(token: &str) -> FozzyResult<DistributedInvariant> {
    if let Some(key) = token.strip_prefix("kv_all_equal:") {
        return Ok(DistributedInvariant::KvAllEqual {
            key: key.to_string(),
        });
    }
    if let Some(key) = token.strip_prefix("kv_present_on_all:") {
        return Ok(DistributedInvariant::KvPresentOnAll {
            key: key.to_string(),
        });
    }
    if let Some(rest) = token.strip_prefix("kv_node_equals:") {
        let mut parts = rest.splitn(3, ':');
        let node = parts.next().unwrap_or_default().trim();
        let key = parts.next().unwrap_or_default().trim();
        let equals = parts.next().unwrap_or_default().trim();
        if node.is_empty() || key.is_empty() || equals.is_empty() {
            return Err(FozzyError::InvalidArgument(
                "invalid --checker kv_node_equals syntax; expected kv_node_equals:<node>:<key>:<value>".to_string(),
            ));
        }
        return Ok(DistributedInvariant::KvNodeEquals {
            node: node.to_string(),
            key: key.to_string(),
            equals: equals.to_string(),
        });
    }

    Err(FozzyError::InvalidArgument(format!(
        "unknown --checker {token:?} (supported: kv_all_equal:<key>, kv_present_on_all:<key>, kv_node_equals:<node>:<key>:<value>)"
    )))
}

pub(super) fn shrink_trial_duration() -> Duration {
    Duration::from_secs(2)
}
