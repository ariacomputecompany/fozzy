use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

use crate::{DistributedInvariant, DistributedStep, ExitStatus, Finding, Reporter, TraceEvent};

pub(super) type ExploreExecResult = (
    ExitStatus,
    Vec<Finding>,
    Vec<TraceEvent>,
    u64,
    Vec<crate::Decision>,
);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleStrategy {
    Fifo,
    Bfs,
    Dfs,
    Random,
    Pct,
    CoverageGuided,
}

impl clap::ValueEnum for ScheduleStrategy {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::Fifo,
            Self::Bfs,
            Self::Dfs,
            Self::Random,
            Self::Pct,
            Self::CoverageGuided,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Fifo => clap::builder::PossibleValue::new("fifo"),
            Self::Bfs => clap::builder::PossibleValue::new("bfs"),
            Self::Dfs => clap::builder::PossibleValue::new("dfs"),
            Self::Random => clap::builder::PossibleValue::new("random"),
            Self::Pct => clap::builder::PossibleValue::new("pct"),
            Self::CoverageGuided => clap::builder::PossibleValue::new("coverage_guided"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExploreOptions {
    pub seed: Option<u64>,
    pub time: Option<Duration>,
    pub steps: Option<u64>,
    pub nodes: Option<usize>,
    pub faults: Option<String>,
    pub schedule: ScheduleStrategy,
    pub checker: Option<String>,
    pub record_trace_to: Option<PathBuf>,
    pub shrink: bool,
    pub minimize: bool,
    pub reporter: Reporter,
    pub record_collision: crate::RecordCollisionPolicy,
    pub profile_capture: crate::ProfileCaptureLevel,
    pub memory: crate::MemoryOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExploreTrace {
    pub scenario_path: String,
    pub scenario: ScenarioV1Explore,
    pub schedule: ScheduleStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioV1Explore {
    pub version: u32,
    pub name: String,
    pub nodes: Vec<String>,
    pub steps: Vec<DistributedStep>,
    #[serde(default)]
    pub invariants: Vec<DistributedInvariant>,
}

#[derive(Debug, Clone)]
pub(super) struct Node {
    pub running: bool,
    pub kv: BTreeMap<String, String>,
    pub kv_version: BTreeMap<String, u64>,
}

#[derive(Debug, Clone)]
pub(super) struct Message {
    pub id: u64,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub key: String,
    pub value: String,
    pub version: u64,
}

#[derive(Debug, Clone, Default)]
pub(super) struct NetRules {
    partitions: BTreeSet<(String, String)>,
}

impl NetRules {
    pub fn is_blocked(&self, a: &str, b: &str) -> bool {
        let (x, y) = super::utils::ordered_pair(a, b);
        self.partitions.contains(&(x.to_string(), y.to_string()))
    }

    pub fn partition(&mut self, a: &str, b: &str) {
        let (x, y) = super::utils::ordered_pair(a, b);
        self.partitions.insert((x.to_string(), y.to_string()));
    }

    pub fn heal(&mut self, a: &str, b: &str) {
        let (x, y) = super::utils::ordered_pair(a, b);
        self.partitions.remove(&(x.to_string(), y.to_string()));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InvariantPhase {
    Progress,
    Final,
}

pub(super) type NodeMap = BTreeMap<String, Node>;
pub(super) type MessageQueue = VecDeque<Message>;
