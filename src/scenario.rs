//! Scenario file parsing and a minimal deterministic step DSL for v0.1.

use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};

use crate::{parse_duration, FozzyError, FozzyResult};

#[derive(Debug, Clone)]
pub struct ScenarioPath {
    path: PathBuf,
}

impl ScenarioPath {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScenarioFile {
    Steps(ScenarioV1Steps),
    Suites(ScenarioV1Suites),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioV1Steps {
    pub version: u32,
    pub name: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioV1Suites {
    pub version: u32,
    pub name: String,
    pub suites: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Step {
    TraceEvent { name: String, #[serde(default)] fields: serde_json::Map<String, serde_json::Value> },
    RandU64 { #[serde(default)] key: Option<String> },
    AssertEqInt { a: i64, b: i64, #[serde(default)] msg: Option<String> },
    AssertEqStr { a: String, b: String, #[serde(default)] msg: Option<String> },
    Sleep { duration: String },
    Advance { duration: String },
    Freeze { #[serde(default)] at_ms: Option<u64> },
    Unfreeze,
    SetKv { key: String, value: String },
    GetKvAssert { key: String, equals: Option<String>, is_null: Option<bool> },
    FsWrite { path: String, data: String },
    FsReadAssert { path: String, equals: String },
    FsSnapshot { name: String },
    FsRestore { name: String },
    Fail { message: String },
    Panic { message: String },
}

#[derive(Debug, Clone)]
pub struct Scenario {
    pub name: String,
    pub steps: Vec<Step>,
}

impl Scenario {
    pub fn load(path: &ScenarioPath) -> FozzyResult<Self> {
        let bytes = std::fs::read(path.as_path())?;
        let file: ScenarioFile = serde_json::from_slice(&bytes)?;
        match file {
            ScenarioFile::Steps(s) => {
                if s.version != 1 {
                    return Err(FozzyError::Scenario(format!(
                        "unsupported scenario version {} (expected 1)",
                        s.version
                    )));
                }
                Ok(Self { name: s.name, steps: s.steps })
            }
            ScenarioFile::Suites(_s) => Err(FozzyError::Scenario(format!(
                "scenario file {} uses `suites` without an executable step DSL (v0.1 only supports `steps`)",
                path.as_path().display()
            ))),
        }
    }

    pub fn validate(&self) -> FozzyResult<()> {
        for step in &self.steps {
            match step {
                Step::Sleep { duration } | Step::Advance { duration } => {
                    parse_duration(duration)?;
                }
                Step::GetKvAssert { equals: Some(_), is_null: Some(true), .. } => {
                    return Err(FozzyError::Scenario(
                        "GetKvAssert: cannot set both equals and is_null=true".to_string(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn example() -> ScenarioV1Steps {
        ScenarioV1Steps {
            version: 1,
            name: "example".to_string(),
            steps: vec![
                Step::TraceEvent {
                    name: "setup".to_string(),
                    fields: serde_json::Map::new(),
                },
                Step::RandU64 { key: Some("rand".to_string()) },
                Step::Sleep { duration: "10ms".to_string() },
                Step::AssertEqInt { a: 1, b: 1, msg: None },
            ],
        }
    }
}
