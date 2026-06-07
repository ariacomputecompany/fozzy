use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use walkdir::WalkDir;

use crate::{FozzyError, FozzyResult};

use super::path::validate_output_file_path_secure;
pub(super) fn minimize_corpus(
    dir: &Path,
    budget: Option<crate::FozzyDuration>,
) -> FozzyResult<serde_json::Value> {
    if !dir.exists() {
        return Err(FozzyError::InvalidArgument(format!(
            "corpus directory not found: {}",
            dir.display()
        )));
    }
    if !dir.is_dir() {
        return Err(FozzyError::InvalidArgument(format!(
            "corpus minimize source is not a directory: {}",
            dir.display()
        )));
    }

    let started = Instant::now();
    let budget_limit = budget.map(|d| d.0);
    let check_budget = |phase: &str| -> FozzyResult<()> {
        if let Some(limit) = budget_limit
            && started.elapsed() > limit
        {
            return Err(FozzyError::InvalidArgument(format!(
                "corpus minimize exceeded budget during {phase}: limit={}ms",
                limit.as_millis()
            )));
        }
        Ok(())
    };

    let mut files = Vec::<PathBuf>::new();
    for entry in WalkDir::new(dir).min_depth(1).max_depth(1) {
        check_budget("scan")?;
        let entry = entry.map_err(|e| {
            let msg = e.to_string();
            FozzyError::Io(
                e.into_io_error()
                    .unwrap_or_else(|| std::io::Error::other(msg)),
            )
        })?;
        if entry.file_type().is_symlink() {
            return Err(FozzyError::InvalidArgument(format!(
                "corpus minimize refuses symlinked input: {}",
                entry.path().display()
            )));
        }
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }
    files.sort();

    if files.is_empty() {
        return Err(FozzyError::InvalidArgument(format!(
            "corpus directory has no files to minimize: {}",
            dir.display()
        )));
    }

    let mut unique_by_hash = BTreeMap::<String, Vec<u8>>::new();
    let mut duplicate_files = 0u64;
    let mut duplicate_bytes = 0u64;
    let mut original_bytes = 0u64;

    for path in &files {
        check_budget("read")?;
        let bytes = std::fs::read(path)?;
        original_bytes = original_bytes.saturating_add(bytes.len() as u64);
        let hash = blake3::hash(&bytes).to_hex().to_string();
        if unique_by_hash.contains_key(&hash) {
            duplicate_files = duplicate_files.saturating_add(1);
            duplicate_bytes = duplicate_bytes.saturating_add(bytes.len() as u64);
        } else {
            unique_by_hash.insert(hash, bytes);
        }
    }

    let parent = dir.parent().unwrap_or_else(|| Path::new("."));
    let staging = parent.join(format!(
        ".corpus-minimize-{}.{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&staging)?;

    let write_result = (|| -> FozzyResult<()> {
        for (hash, bytes) in &unique_by_hash {
            check_budget("write")?;
            let name = format!("input-{hash}.bin");
            std::fs::write(staging.join(name), bytes)?;
        }
        Ok(())
    })();

    if let Err(err) = write_result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(err);
    }

    let swap_result = (|| -> FozzyResult<()> {
        for path in &files {
            check_budget("swap")?;
            std::fs::remove_file(path)?;
        }
        for entry in WalkDir::new(&staging).min_depth(1).max_depth(1) {
            check_budget("swap")?;
            let entry = entry.map_err(|e| {
                let msg = e.to_string();
                FozzyError::Io(
                    e.into_io_error()
                        .unwrap_or_else(|| std::io::Error::other(msg)),
                )
            })?;
            if entry.file_type().is_file() {
                let name = entry.file_name().to_owned();
                std::fs::rename(entry.path(), dir.join(name))?;
            }
        }
        std::fs::remove_dir(&staging)?;
        Ok(())
    })();

    if let Err(err) = swap_result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(err);
    }

    Ok(serde_json::json!({
        "ok": true,
        "dir": dir.to_string_lossy().to_string(),
        "filesBefore": files.len(),
        "filesAfter": unique_by_hash.len(),
        "duplicatesRemoved": duplicate_files,
        "bytesBefore": original_bytes,
        "bytesAfter": original_bytes.saturating_sub(duplicate_bytes),
        "bytesRemoved": duplicate_bytes
    }))
}

pub(super) fn export_zip(dir: &Path, out_zip: &Path) -> FozzyResult<()> {
    if !dir.exists() {
        return Err(FozzyError::InvalidArgument(format!(
            "corpus directory not found: {}",
            dir.display()
        )));
    }
    if !dir.is_dir() {
        return Err(FozzyError::InvalidArgument(format!(
            "corpus export source is not a directory: {}",
            dir.display()
        )));
    }

    validate_output_file_path_secure(out_zip)?;
    if let Some(parent) = out_zip.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_name = out_zip
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("corpus.zip");
    let tmp_name = format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4()
    );
    let tmp_path = out_zip
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(tmp_name);

    let write_result = (|| -> FozzyResult<()> {
        let file = std::fs::File::create(&tmp_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o644);

        let mut wrote_any = false;
        for entry in WalkDir::new(dir).min_depth(1) {
            let entry = entry.map_err(|e| {
                let msg = e.to_string();
                FozzyError::Io(
                    e.into_io_error()
                        .unwrap_or_else(|| std::io::Error::other(msg)),
                )
            })?;
            if !entry.file_type().is_file() {
                continue;
            }

            let rel = entry.path().strip_prefix(dir).unwrap_or(entry.path());
            let name = rel.to_string_lossy().replace('\\', "/");
            zip.start_file(name, options)?;
            let bytes = std::fs::read(entry.path())?;
            use std::io::Write as _;
            zip.write_all(&bytes)?;
            wrote_any = true;
        }

        if !wrote_any {
            return Err(FozzyError::InvalidArgument(format!(
                "corpus directory has no files to export: {}",
                dir.display()
            )));
        }

        zip.finish()?;
        Ok(())
    })();

    match write_result {
        Ok(()) => {
            std::fs::rename(&tmp_path, out_zip)?;
            Ok(())
        }
        Err(err) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(err)
        }
    }
}
