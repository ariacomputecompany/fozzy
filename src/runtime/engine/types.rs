use serde::{Deserialize, Serialize};

use std::path::PathBuf;
use std::time::Duration;

use crate::{
    ExitStatus, Finding, MemoryOptions, MemoryRunReport, Reporter, RunSummary, ScenarioV1Steps,
    TraceEvent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcBackend {
    Scripted,
    Host,
}

impl clap::ValueEnum for ProcBackend {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Scripted, Self::Host]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Scripted => clap::builder::PossibleValue::new("scripted"),
            Self::Host => clap::builder::PossibleValue::new("host"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FsBackend {
    Virtual,
    Host,
}

impl clap::ValueEnum for FsBackend {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Virtual, Self::Host]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Virtual => clap::builder::PossibleValue::new("virtual"),
            Self::Host => clap::builder::PossibleValue::new("host"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpBackend {
    Scripted,
    Host,
}

impl clap::ValueEnum for HttpBackend {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Scripted, Self::Host]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Scripted => clap::builder::PossibleValue::new("scripted"),
            Self::Host => clap::builder::PossibleValue::new("host"),
        })
    }
}

#[derive(Debug, Clone)]
pub enum InitTemplate {
    Ts,
    Rust,
    Minimal,
}

impl clap::ValueEnum for InitTemplate {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Ts, Self::Rust, Self::Minimal]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Ts => clap::builder::PossibleValue::new("ts"),
            Self::Rust => clap::builder::PossibleValue::new("rust"),
            Self::Minimal => clap::builder::PossibleValue::new("minimal"),
        })
    }
}

impl InitTemplate {
    pub fn from_option(opt: Option<&InitTemplate>) -> Self {
        opt.cloned().unwrap_or(Self::Minimal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitTestType {
    Run,
    Fuzz,
    Explore,
    Memory,
    Host,
    All,
}

impl clap::ValueEnum for InitTestType {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::Run,
            Self::Fuzz,
            Self::Explore,
            Self::Memory,
            Self::Host,
            Self::All,
        ]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Run => clap::builder::PossibleValue::new("run"),
            Self::Fuzz => clap::builder::PossibleValue::new("fuzz"),
            Self::Explore => clap::builder::PossibleValue::new("explore"),
            Self::Memory => clap::builder::PossibleValue::new("memory"),
            Self::Host => clap::builder::PossibleValue::new("host"),
            Self::All => clap::builder::PossibleValue::new("all"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub det: bool,
    pub seed: Option<u64>,
    pub timeout: Option<Duration>,
    pub reporter: Reporter,
    pub record_trace_to: Option<PathBuf>,
    pub filter: Option<String>,
    pub jobs: Option<usize>,
    pub fail_fast: bool,
    pub record_collision: RecordCollisionPolicy,
    pub profile_capture: ProfileCaptureLevel,
    pub proc_backend: ProcBackend,
    pub fs_backend: FsBackend,
    pub http_backend: HttpBackend,
    pub memory: MemoryOptions,
}

#[derive(Debug, Clone)]
pub struct ReplayOptions {
    pub step: bool,
    pub until: Option<Duration>,
    pub dump_events: bool,
    pub profile_capture: ProfileCaptureLevel,
    pub reporter: Reporter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileCaptureLevel {
    Baseline,
    Full,
}

impl clap::ValueEnum for ProfileCaptureLevel {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Baseline, Self::Full]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Baseline => clap::builder::PossibleValue::new("baseline"),
            Self::Full => clap::builder::PossibleValue::new("full"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrinkMinimize {
    Input,
    Schedule,
    Faults,
    All,
}

impl clap::ValueEnum for ShrinkMinimize {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Input, Self::Schedule, Self::Faults, Self::All]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Input => clap::builder::PossibleValue::new("input"),
            Self::Schedule => clap::builder::PossibleValue::new("schedule"),
            Self::Faults => clap::builder::PossibleValue::new("faults"),
            Self::All => clap::builder::PossibleValue::new("all"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct ShrinkOptions {
    pub out_trace_path: Option<PathBuf>,
    pub budget: Option<Duration>,
    pub aggressive: bool,
    pub minimize: ShrinkMinimize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordCollisionPolicy {
    Error,
    Overwrite,
    Append,
}

impl clap::ValueEnum for RecordCollisionPolicy {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Error, Self::Overwrite, Self::Append]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Error => clap::builder::PossibleValue::new("error"),
            Self::Overwrite => clap::builder::PossibleValue::new("overwrite"),
            Self::Append => clap::builder::PossibleValue::new("append"),
        })
    }
}

#[derive(Debug, Clone)]
pub struct RunResult {
    pub summary: RunSummary,
}

#[derive(Debug, Clone)]
pub struct ShrinkResult {
    pub out_trace_path: String,
    pub result: RunResult,
}

#[derive(Debug, Clone)]
pub(crate) struct ScenarioRun {
    pub(crate) status: ExitStatus,
    pub(crate) findings: Vec<Finding>,
    pub(crate) memory: Option<MemoryRunReport>,
    pub(crate) decisions: crate::DecisionLog,
    pub(crate) events: Vec<TraceEvent>,
    pub(crate) scenario_path: PathBuf,
    pub(crate) scenario_embedded: ScenarioV1Steps,
    pub(crate) started_at: String,
    pub(crate) finished_at: String,
    pub(crate) duration_ms: u64,
    pub(crate) duration_ns: u64,
}
