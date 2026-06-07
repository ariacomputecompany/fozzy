use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

use crate::{
    Config, ExitStatus, Finding, FozzyError, FozzyResult, MemoryOptions, ScenarioFile,
    ScenarioPath, TraceEvent, TraceFile, wall_time_iso_utc,
};

use super::{seed_from_input, stable_edge, FuzzTarget};

#[derive(Debug, Clone)]
pub(crate) struct FuzzExec {
    pub(crate) status: ExitStatus,
    pub(crate) findings: Vec<Finding>,
    pub(crate) events: Vec<TraceEvent>,
    pub(crate) coverage: BTreeSet<u64>,
    pub(crate) memory: Option<crate::MemoryTrace>,
    pub(crate) started_at: String,
    pub(crate) finished_at: String,
    pub(crate) duration_ms: u64,
    pub(crate) duration_ns: u64,
}

pub(crate) fn execute_target(
    config: &Config,
    target: &FuzzTarget,
    input: &[u8],
    scenario_memory: &MemoryOptions,
) -> FozzyResult<FuzzExec> {
    let started_at = wall_time_iso_utc();
    let started = Instant::now();
    let mut exec = match target {
        FuzzTarget::Scenario { path } => {
            execute_scenario_target(config, path, input, scenario_memory)
        }
    }?;
    exec.started_at = started_at;
    exec.finished_at = wall_time_iso_utc();
    let (duration_ms, duration_ns) = crate::duration_fields(started.elapsed());
    exec.duration_ms = duration_ms;
    exec.duration_ns = duration_ns;
    Ok(exec)
}

pub(crate) fn target_string(target: &FuzzTarget) -> String {
    match target {
        FuzzTarget::Scenario { path } => format!("scenario:{}", path.display()),
    }
}

pub(crate) fn fuzz_exec_memory(
    exec_memory: Option<&crate::MemoryTrace>,
    harness_memory: Option<&crate::MemoryRunReport>,
) -> Option<crate::MemoryRunReport> {
    exec_memory
        .map(|memory| crate::MemoryRunReport {
            schema_version: "fozzy.memory_report.v1".to_string(),
            options: memory.options.clone(),
            summary: memory.summary.clone(),
            leaks: memory.leaks.clone(),
            timeline: Vec::new(),
            graph: memory.graph.clone(),
        })
        .or_else(|| harness_memory.cloned())
}

pub(crate) fn fuzz_trace_memory_options(trace: &TraceFile) -> MemoryOptions {
    trace
        .memory
        .as_ref()
        .map(|memory| memory.options.clone())
        .unwrap_or(MemoryOptions {
            track: false,
            artifacts: false,
            ..MemoryOptions::default()
        })
}

fn execute_scenario_target(
    _config: &Config,
    path: &Path,
    input: &[u8],
    scenario_memory: &MemoryOptions,
) -> FozzyResult<FuzzExec> {
    let seed = seed_from_input(input);
    let scenario_path = ScenarioPath::new(path.to_path_buf());
    let parsed = crate::Scenario::load_file(&scenario_path)?;
    let parsed = match parsed {
        ScenarioFile::Steps(steps) => ScenarioTarget::Steps(steps),
        ScenarioFile::Distributed(distributed) => {
            ScenarioTarget::Distributed(crate::distributed_to_explore(distributed, None)?)
        }
        ScenarioFile::Suites(_) => ScenarioTarget::Suites,
    };
    let exec = match parsed {
        ScenarioTarget::Steps(scenario) => {
            let run =
                crate::run_embedded_steps_for_fuzz(&scenario, path, seed, scenario_memory.clone())?;
            FuzzExec {
                status: run.status,
                findings: run.findings,
                events: run.events,
                coverage: BTreeSet::new(),
                memory: run
                    .memory
                    .as_ref()
                    .map(|memory: &crate::MemoryRunReport| memory.to_trace()),
                started_at: run.started_at,
                finished_at: run.finished_at,
                duration_ms: run.duration_ms,
                duration_ns: run.duration_ns,
            }
        }
        ScenarioTarget::Distributed(scenario) => {
            let (status, findings, events) = crate::execute_explore_for_fuzz(&scenario, seed)?;
            FuzzExec {
                status,
                findings,
                events,
                coverage: BTreeSet::new(),
                memory: None,
                started_at: String::new(),
                finished_at: String::new(),
                duration_ms: 0,
                duration_ns: 0,
            }
        }
        ScenarioTarget::Suites => {
            return Err(FozzyError::InvalidArgument(format!(
                "scenario fuzz target {} uses suites variant; provide a steps or distributed scenario",
                path.display()
            )));
        }
    };

    let mut coverage = BTreeSet::new();
    coverage.insert(stable_edge(&format!("scenario_path:{}", path.display())));
    coverage.insert(stable_edge(&format!("scenario_status:{:?}", exec.status)));
    coverage.insert(stable_edge(&format!("scenario_seed:{seed}")));
    for finding in &exec.findings {
        coverage.insert(stable_edge(&format!(
            "scenario_finding:{}:{}",
            finding.title, finding.message
        )));
    }

    Ok(FuzzExec { coverage, ..exec })
}

#[derive(Clone)]
enum ScenarioTarget {
    Steps(crate::ScenarioV1Steps),
    Distributed(crate::ScenarioV1Explore),
    Suites,
}
