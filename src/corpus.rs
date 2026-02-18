//! Fuzz corpus management.

use clap::Subcommand;
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::{Config, FozzyError, FozzyResult};

#[derive(Debug, Subcommand)]
pub enum CorpusCommand {
    List { dir: PathBuf },
    Add { dir: PathBuf, file: PathBuf },
    Minimize {
        dir: PathBuf,
        #[arg(long)]
        budget: Option<crate::FozzyDuration>,
    },
    Export { dir: PathBuf, #[arg(long)] out: PathBuf },
    Import { zip: PathBuf, #[arg(long)] out: PathBuf },
}

pub fn corpus_command(_config: &Config, command: &CorpusCommand) -> FozzyResult<serde_json::Value> {
    match command {
        CorpusCommand::List { dir } => {
            let mut files = Vec::new();
            if dir.exists() {
                for entry in WalkDir::new(dir).min_depth(1).max_depth(1) {
                    let entry = entry.map_err(|e| {
                        let msg = e.to_string();
                        FozzyError::Io(
                            e.into_io_error()
                                .unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, msg)),
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

        CorpusCommand::Minimize { dir, budget: _ } => {
            // Placeholder: true corpus minimization depends on the target + coverage signals.
            Ok(serde_json::json!({"ok": true, "dir": dir.to_string_lossy().to_string()}))
        }

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

fn export_zip(dir: &Path, out_zip: &Path) -> FozzyResult<()> {
    if let Some(parent) = out_zip.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(out_zip)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    if dir.exists() {
        for entry in WalkDir::new(dir).min_depth(1) {
            let entry = entry.map_err(|e| {
                let msg = e.to_string();
                FozzyError::Io(
                    e.into_io_error()
                        .unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, msg)),
                )
            })?;
            if !entry.file_type().is_file() {
                continue;
            }

            let rel = entry.path().strip_prefix(dir).unwrap_or(entry.path());
            let name = rel.to_string_lossy().replace('\\', "/");
            zip.start_file(name, options)?;
            let bytes = std::fs::read(entry.path())?;
            zip.write_all(&bytes)?;
        }
    }

    zip.finish()?;
    Ok(())
}

fn import_zip(zip_path: &Path, out_dir: &Path) -> FozzyResult<()> {
    std::fs::create_dir_all(out_dir)?;

    let file = File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| FozzyError::InvalidArgument(format!("invalid zip: {e}")))?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| FozzyError::InvalidArgument(format!("zip read error: {e}")))?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().to_string();
        let out_path = out_dir.join(name);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes)?;
        std::fs::write(out_path, bytes)?;
    }
    Ok(())
}
