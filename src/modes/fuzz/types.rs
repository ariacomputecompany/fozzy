use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::{FozzyError, MemoryOptions, ProfileCaptureLevel, RecordCollisionPolicy, Reporter};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FuzzMode {
    Coverage,
    Property,
}

impl clap::ValueEnum for FuzzMode {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Coverage, Self::Property]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Coverage => clap::builder::PossibleValue::new("coverage"),
            Self::Property => clap::builder::PossibleValue::new("property"),
        })
    }
}

#[derive(Debug, Clone)]
pub enum FuzzTarget {
    Scenario { path: PathBuf },
}

impl std::str::FromStr for FuzzTarget {
    type Err = FozzyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if let Some(rest) = s.strip_prefix("scenario:") {
            let path = PathBuf::from(rest.trim());
            if path.as_os_str().is_empty() {
                return Err(FozzyError::InvalidArgument(
                    "fuzz target scenario: requires a path".to_string(),
                ));
            }
            return Ok(Self::Scenario { path });
        }
        if s.ends_with(".fozzy.json") {
            return Ok(Self::Scenario {
                path: PathBuf::from(s),
            });
        }

        Err(FozzyError::InvalidArgument(format!(
            "unsupported fuzz target {s:?} (expected scenario:<path.fozzy.json> or <path.fozzy.json>)"
        )))
    }
}

#[derive(Debug, Clone)]
pub struct FuzzOptions {
    pub det: bool,
    pub mode: FuzzMode,
    pub seed: Option<u64>,
    pub time: Option<Duration>,
    pub runs: Option<u64>,
    pub max_input_bytes: usize,
    pub corpus_dir: Option<PathBuf>,
    pub mutator: Option<String>,
    pub shrink: bool,
    pub record_trace_to: Option<PathBuf>,
    pub reporter: Reporter,
    pub crash_only: bool,
    pub minimize: bool,
    pub record_collision: RecordCollisionPolicy,
    pub profile_capture: ProfileCaptureLevel,
    pub memory: MemoryOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzTrace {
    pub target: String,
    pub input_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzCoverageStats {
    pub target: String,
    pub executed: u64,
    pub crashes: u64,
    pub unique_edges: usize,
    pub discovered_edges_total: u64,
    pub max_new_edges_per_input: u64,
    pub corpus_entries: usize,
}
