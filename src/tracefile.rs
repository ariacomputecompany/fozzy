//! Trace file format (.fozzy) read/write.

use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};

use crate::{Decision, RunMode, RunSummary, ScenarioV1Steps, VersionInfo};

#[derive(Debug, Clone)]
pub struct TracePath {
    path: PathBuf,
}

impl TracePath {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceFile {
    pub format: String,
    pub version: u32,
    pub engine: VersionInfo,
    pub mode: RunMode,
    pub scenario_path: Option<String>,
    pub scenario: Option<ScenarioV1Steps>,
    pub decisions: Vec<Decision>,
    pub events: Vec<TraceEvent>,
    pub summary: RunSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub time_ms: u64,
    pub name: String,
    #[serde(default)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

impl TraceFile {
    pub fn new(
        mode: RunMode,
        scenario_path: Option<String>,
        scenario: Option<ScenarioV1Steps>,
        decisions: Vec<Decision>,
        events: Vec<TraceEvent>,
        summary: RunSummary,
    ) -> Self {
        Self {
            format: "fozzy-trace".to_string(),
            version: 1,
            engine: crate::version_info(),
            mode,
            scenario_path,
            scenario,
            decisions,
            events,
            summary,
        }
    }

    pub fn write_json(&self, path: &Path) -> crate::FozzyResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub fn read_json(path: &Path) -> crate::FozzyResult<Self> {
        let bytes = std::fs::read(path)?;
        let t: TraceFile = serde_json::from_slice(&bytes)?;
        Ok(t)
    }
}

