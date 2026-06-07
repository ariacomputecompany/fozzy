use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::FozzyResult;

pub(super) fn export_artifacts_zip(files: &[PathBuf], out_zip: &Path) -> FozzyResult<()> {
    use std::fs::File;
    use std::io::{Read as _, Write as _};

    validate_output_file_path_secure(out_zip)?;
    if let Some(parent) = out_zip.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_name = out_zip
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("artifacts.zip");
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
        let file = File::create(&tmp_path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o644);
        let mut used_names: BTreeSet<String> = BTreeSet::new();

        for src in files {
            let name = zip_entry_name_for_path(src, &mut used_names);
            zip.start_file(name, options)?;
            let mut in_file = File::open(src)?;
            let mut buf = [0u8; 64 * 1024];
            loop {
                let n = in_file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buf[..n])?;
            }
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

fn zip_entry_name_for_path(path: &Path, used: &mut BTreeSet<String>) -> String {
    let base = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "artifact".to_string());

    let mut safe: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    while safe.contains("__") {
        safe = safe.replace("__", "_");
    }
    safe = safe.trim_matches('_').to_string();
    if safe.is_empty() {
        safe = "artifact".to_string();
    }

    let (stem, ext) = match safe.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && !e.is_empty() => (s.to_string(), Some(e.to_string())),
        _ => (safe.clone(), None),
    };

    if used.insert(safe.clone()) {
        return safe;
    }

    for i in 2..=10_000usize {
        let candidate = match &ext {
            Some(ext) => format!("{stem}.{i}.{ext}"),
            None => format!("{stem}.{i}"),
        };
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    "artifact.overflow".to_string()
}

fn copy_file_into_dir_secure(src: &Path, out_dir: &Path) -> FozzyResult<()> {
    if out_dir.exists() {
        let out_md = std::fs::symlink_metadata(out_dir)?;
        if out_md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to write into symlinked output directory: {}",
                out_dir.display()
            )));
        }
    }

    let name = src.file_name().ok_or_else(|| {
        crate::FozzyError::InvalidArgument(format!("invalid artifact path: {}", src.display()))
    })?;
    let dst = out_dir.join(name);
    if dst.exists() {
        let dst_md = std::fs::symlink_metadata(&dst)?;
        if dst_md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to overwrite symlinked output file: {}",
                dst.display()
            )));
        }
        if !dst_md.is_file() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to overwrite non-file output path: {}",
                dst.display()
            )));
        }
        std::fs::remove_file(&dst)?;
    }

    let tmp_name = format!(
        ".{}.{}.{}.tmp",
        dst.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("artifact"),
        std::process::id(),
        uuid::Uuid::new_v4()
    );
    let tmp = out_dir.join(tmp_name);
    std::fs::copy(src, &tmp)?;
    std::fs::rename(&tmp, &dst)?;
    Ok(())
}

fn validate_copy_targets_secure(files: &[PathBuf], out_dir: &Path) -> FozzyResult<()> {
    validate_output_dir_path_secure(out_dir)?;
    if out_dir.exists() {
        let out_md = std::fs::symlink_metadata(out_dir)?;
        if out_md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to write into symlinked output directory: {}",
                out_dir.display()
            )));
        }
    }

    let mut seen = std::collections::BTreeSet::<String>::new();
    for src in files {
        let name = src
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                crate::FozzyError::InvalidArgument(format!(
                    "invalid artifact path: {}",
                    src.display()
                ))
            })?
            .to_string();
        if !seen.insert(name.clone()) {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "duplicate output file target detected: {name}"
            )));
        }
        let dst = out_dir.join(&name);
        if dst.exists() {
            let dst_md = std::fs::symlink_metadata(&dst)?;
            if dst_md.file_type().is_symlink() {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "refusing to overwrite symlinked output file: {}",
                    dst.display()
                )));
            }
            if !dst_md.is_file() {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "refusing to overwrite non-file output path: {}",
                    dst.display()
                )));
            }
        }
    }

    Ok(())
}

pub(super) fn export_artifacts_dir_exact(files: &[PathBuf], out_dir: &Path) -> FozzyResult<()> {
    std::fs::create_dir_all(out_dir)?;
    validate_copy_targets_secure(files, out_dir)?;
    prune_stale_output_entries(files, out_dir)?;
    for src in files {
        copy_file_into_dir_secure(src, out_dir)?;
    }
    Ok(())
}

fn prune_stale_output_entries(files: &[PathBuf], out_dir: &Path) -> FozzyResult<()> {
    let expected: BTreeSet<String> = files
        .iter()
        .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .collect();
    for entry in std::fs::read_dir(out_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if expected.contains(&name) {
            continue;
        }
        let path = entry.path();
        let md = std::fs::symlink_metadata(&path)?;
        if md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to remove symlinked stale output entry: {}",
                path.display()
            )));
        }
        if md.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn validate_output_file_path_secure(out_file: &Path) -> FozzyResult<()> {
    if out_file.exists() {
        let md = std::fs::symlink_metadata(out_file)?;
        if md.file_type().is_symlink() {
            return Err(crate::FozzyError::InvalidArgument(format!(
                "refusing to overwrite symlinked output file: {}",
                out_file.display()
            )));
        }
    }
    validate_output_dir_path_secure(out_file.parent().unwrap_or_else(|| Path::new(".")))
}

fn validate_output_dir_path_secure(path: &Path) -> FozzyResult<()> {
    let is_abs = path.is_absolute();
    let mut cur = if is_abs {
        PathBuf::from(Path::new(std::path::MAIN_SEPARATOR_STR))
    } else {
        std::env::current_dir()?
    };
    let mut normal_seen = 0usize;
    for comp in path.components() {
        use std::path::Component;
        match comp {
            Component::Prefix(prefix) => cur.push(prefix.as_os_str()),
            Component::RootDir => {}
            Component::CurDir => continue,
            Component::ParentDir => cur.push(".."),
            Component::Normal(seg) => {
                normal_seen += 1;
                cur.push(seg);
            }
        }
        if cur.exists() {
            let md = std::fs::symlink_metadata(&cur)?;
            let skip_abs_top_component = is_abs && normal_seen == 1;
            if md.file_type().is_symlink() && !skip_abs_top_component {
                return Err(crate::FozzyError::InvalidArgument(format!(
                    "refusing to write through symlinked output path: {}",
                    cur.display()
                )));
            }
        }
    }
    Ok(())
}
