//! `fozzy.toml` config loading.

use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Config {
    /// Base directory for fozzy runtime artifacts.
    #[serde(default = "default_base_dir")]
    pub base_dir: PathBuf,

    /// Default reporter for CLI commands.
    #[serde(default = "default_reporter")]
    pub reporter: crate::Reporter,
}

fn default_base_dir() -> PathBuf {
    PathBuf::from(".fozzy")
}

fn default_reporter() -> crate::Reporter {
    crate::Reporter::Pretty
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_dir: default_base_dir(),
            reporter: default_reporter(),
        }
    }
}

impl Config {
    pub fn load_optional(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => match toml::from_str::<Config>(&s) {
                Ok(cfg) => cfg,
                Err(err) => {
                    tracing::warn!("failed to parse config {}: {err}", path.display());
                    Self::default()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(err) => {
                tracing::warn!("failed to read config {}: {err}", path.display());
                Self::default()
            }
        }
    }

    pub fn runs_dir(&self) -> PathBuf {
        self.base_dir.join("runs")
    }

    pub fn corpora_dir(&self) -> PathBuf {
        self.base_dir.join("corpora")
    }
}
