//! Fuzz corpus management.

use clap::Subcommand;
use std::path::PathBuf;

use crate::{Config, FozzyResult};

#[path = "corpus/ops.rs"]
mod ops;
#[path = "corpus/path.rs"]
mod path;
#[path = "corpus/zip.rs"]
mod zip;

use ops::{export_zip, minimize_corpus};
use zip::import_zip;

#[derive(Debug, Subcommand)]
pub enum CorpusCommand {
    List {
        dir: PathBuf,
    },
    Add {
        dir: PathBuf,
        file: PathBuf,
    },
    Minimize {
        dir: PathBuf,
        #[arg(long)]
        budget: Option<crate::FozzyDuration>,
    },
    Export {
        dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Import {
        zip: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

pub fn corpus_command(_config: &Config, command: &CorpusCommand) -> FozzyResult<serde_json::Value> {
    match command {
        CorpusCommand::List { dir } => {
            let mut files = Vec::new();
            if dir.exists() {
                for entry in walkdir::WalkDir::new(dir).min_depth(1).max_depth(1) {
                    let entry = entry.map_err(|e| {
                        let msg = e.to_string();
                        crate::FozzyError::Io(
                            e.into_io_error()
                                .unwrap_or_else(|| std::io::Error::other(msg)),
                        )
                    })?;
                    if entry.file_type().is_file() {
                        files.push(entry.path().to_string_lossy().to_string());
                    }
                }
            }
            files.sort();
            Ok(serde_json::to_value(files)?)
        }

        CorpusCommand::Add { dir, file } => {
            std::fs::create_dir_all(dir)?;
            let bytes = std::fs::read(file)?;
            let name = format!("input-{}.bin", blake3::hash(&bytes).to_hex());
            let out_path = dir.join(name);
            std::fs::write(&out_path, bytes)?;
            Ok(serde_json::json!({"added": out_path.to_string_lossy().to_string()}))
        }

        CorpusCommand::Minimize { dir, budget } => minimize_corpus(dir, *budget),

        CorpusCommand::Export { dir, out } => {
            export_zip(dir, out)?;
            Ok(serde_json::json!({"ok": true, "zip": out.to_string_lossy().to_string()}))
        }

        CorpusCommand::Import { zip, out } => {
            import_zip(zip, out)?;
            Ok(serde_json::json!({"ok": true, "dir": out.to_string_lossy().to_string()}))
        }
    }
}

#[cfg(test)]
#[path = "corpus/tests.rs"]
mod tests;
