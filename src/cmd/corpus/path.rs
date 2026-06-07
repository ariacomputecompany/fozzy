use std::path::{Path, PathBuf};

use crate::{FozzyError, FozzyResult};

pub(super) fn validate_output_file_path_secure(out_file: &Path) -> FozzyResult<()> {
    if out_file.exists() {
        let md = std::fs::symlink_metadata(out_file)?;
        if md.file_type().is_symlink() {
            return Err(FozzyError::InvalidArgument(format!(
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
                return Err(FozzyError::InvalidArgument(format!(
                    "refusing to write through symlinked output path: {}",
                    cur.display()
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn portable_rel_key(rel: &Path) -> String {
    let mut out = String::new();
    for (idx, comp) in rel.components().enumerate() {
        use std::path::Component;
        if let Component::Normal(seg) = comp {
            if idx > 0 {
                out.push('/');
            }
            out.push_str(&seg.to_string_lossy().to_lowercase());
        }
    }
    out
}

pub(super) fn normalize_zip_entry_rel_path(name: &str) -> FozzyResult<PathBuf> {
    if name.starts_with("//")
        || name.starts_with("\\\\")
        || name.contains('\\')
        || is_windows_drive_prefixed(name)
    {
        return Err(FozzyError::InvalidArgument(format!(
            "unsafe archive entry path rejected: {name}"
        )));
    }

    let path = Path::new(name);
    let mut rel = PathBuf::new();
    for comp in path.components() {
        use std::path::Component;
        match comp {
            Component::Normal(seg) => {
                let seg = seg.to_str().ok_or_else(|| {
                    FozzyError::InvalidArgument(format!(
                        "unsafe archive entry path rejected: {name}"
                    ))
                })?;
                validate_archive_path_segment(seg, name)?;
                rel.push(seg);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(FozzyError::InvalidArgument(format!(
                    "unsafe archive entry path rejected: {name}"
                )));
            }
        }
    }
    if rel.as_os_str().is_empty() {
        return Err(FozzyError::InvalidArgument(format!(
            "unsafe archive entry path rejected: {name}"
        )));
    }
    Ok(rel)
}

pub(super) fn validate_zip_entry_name_raw(raw_name: &[u8], display_name: &str) -> FozzyResult<()> {
    if raw_name.contains(&0) {
        return Err(FozzyError::InvalidArgument(format!(
            "unsafe archive entry path rejected: {display_name}"
        )));
    }
    Ok(())
}

fn is_windows_drive_prefixed(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

fn validate_archive_path_segment(seg: &str, original_name: &str) -> FozzyResult<()> {
    if seg.is_empty() {
        return Err(FozzyError::InvalidArgument(format!(
            "unsafe archive entry path rejected: {original_name}"
        )));
    }

    if seg.ends_with('.') || seg.ends_with(' ') {
        return Err(FozzyError::InvalidArgument(format!(
            "unsafe archive entry path rejected: {original_name}"
        )));
    }

    if seg
        .chars()
        .any(|c| c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
    {
        return Err(FozzyError::InvalidArgument(format!(
            "unsafe archive entry path rejected: {original_name}"
        )));
    }

    if is_windows_reserved_name(seg) {
        return Err(FozzyError::InvalidArgument(format!(
            "unsafe archive entry path rejected: {original_name}"
        )));
    }

    Ok(())
}

fn is_windows_reserved_name(seg: &str) -> bool {
    let trimmed = seg.trim_end_matches(['.', ' ']);
    let stem = trimmed.split('.').next().unwrap_or(trimmed);
    let upper = stem.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}
