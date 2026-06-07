use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

use crate::{FozzyError, FozzyResult};

use super::path::{normalize_zip_entry_rel_path, portable_rel_key, validate_zip_entry_name_raw};

pub(super) fn import_zip(zip_path: &Path, out_dir: &Path) -> FozzyResult<()> {
    std::fs::create_dir_all(out_dir)?;
    if std::fs::symlink_metadata(out_dir)?.file_type().is_symlink() {
        return Err(FozzyError::InvalidArgument(format!(
            "refusing to import into symlinked output directory: {}",
            out_dir.display()
        )));
    }

    validate_zip_archive_raw_entries(zip_path)?;

    let file = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| FozzyError::InvalidArgument(format!("invalid zip: {e}")))?;
    let mut seen_targets = HashSet::new();
    for i in 0..zip.len() {
        let f = zip
            .by_index(i)
            .map_err(|e| FozzyError::InvalidArgument(format!("zip read error: {e}")))?;
        if f.is_dir() {
            continue;
        }
        validate_zip_entry_name_raw(f.name_raw(), f.name())?;
        let rel = normalize_zip_entry_rel_path(f.name())?;
        validate_zip_target_secure(out_dir, &rel, &mut seen_targets)?;
    }

    let file = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| FozzyError::InvalidArgument(format!("invalid zip: {e}")))?;
    for i in 0..zip.len() {
        let mut f = zip
            .by_index(i)
            .map_err(|e| FozzyError::InvalidArgument(format!("zip read error: {e}")))?;
        if f.is_dir() {
            continue;
        }
        validate_zip_entry_name_raw(f.name_raw(), f.name())?;
        let name = f.name().to_string();
        write_zip_entry_secure(out_dir, &name, &mut f)?;
    }
    Ok(())
}

fn validate_zip_archive_raw_entries(zip_path: &Path) -> FozzyResult<()> {
    let bytes = std::fs::read(zip_path)?;
    let raw_names = parse_zip_central_directory_names(&bytes)?;
    let mut seen = HashSet::<String>::new();
    for raw in raw_names {
        if raw.contains(&0) {
            return Err(FozzyError::InvalidArgument(format!(
                "unsafe archive entry path rejected: {}",
                String::from_utf8_lossy(&raw)
            )));
        }
        let name = std::str::from_utf8(&raw).map_err(|_| {
            FozzyError::InvalidArgument(format!(
                "unsafe archive entry path rejected: {}",
                String::from_utf8_lossy(&raw)
            ))
        })?;
        let rel = normalize_zip_entry_rel_path(name)?;
        let key = portable_rel_key(&rel);
        if !seen.insert(key) {
            return Err(FozzyError::InvalidArgument(format!(
                "duplicate output file in archive is not allowed: {}",
                rel.display()
            )));
        }
    }
    Ok(())
}

fn parse_zip_central_directory_names(bytes: &[u8]) -> FozzyResult<Vec<Vec<u8>>> {
    const CEN_SIG: u32 = 0x0201_4b50;
    const ZIP64_U16_MAX: u16 = 0xFFFF;
    const ZIP64_U32_MAX: u32 = 0xFFFF_FFFF;

    let Some(eocd) = find_eocd_offset(bytes) else {
        return Err(FozzyError::InvalidArgument(
            "invalid zip: missing end-of-central-directory".to_string(),
        ));
    };
    let total_entries = read_u16_le(bytes, eocd + 10)?;
    let cd_size = read_u32_le(bytes, eocd + 12)?;
    let cd_offset = read_u32_le(bytes, eocd + 16)?;
    if total_entries == ZIP64_U16_MAX || cd_size == ZIP64_U32_MAX || cd_offset == ZIP64_U32_MAX {
        return Err(FozzyError::InvalidArgument(
            "invalid zip: zip64 archives are not supported for corpus import".to_string(),
        ));
    }

    let total_entries = total_entries as usize;
    let cd_offset = cd_offset as usize;
    let cd_size = cd_size as usize;
    let cd_end = cd_offset.checked_add(cd_size).ok_or_else(|| {
        FozzyError::InvalidArgument("invalid zip: central directory overflow".to_string())
    })?;
    if cd_end > bytes.len() {
        return Err(FozzyError::InvalidArgument(
            "invalid zip: central directory out of bounds".to_string(),
        ));
    }

    let mut names = Vec::with_capacity(total_entries);
    let mut pos = cd_offset;
    for _ in 0..total_entries {
        if pos + 46 > cd_end {
            return Err(FozzyError::InvalidArgument(
                "invalid zip: malformed central directory entry".to_string(),
            ));
        }
        let sig = read_u32_le(bytes, pos)?;
        if sig != CEN_SIG {
            return Err(FozzyError::InvalidArgument(
                "invalid zip: bad central directory signature".to_string(),
            ));
        }
        let name_len = read_u16_le(bytes, pos + 28)? as usize;
        let extra_len = read_u16_le(bytes, pos + 30)? as usize;
        let comment_len = read_u16_le(bytes, pos + 32)? as usize;
        let name_start = pos + 46;
        let name_end = name_start.checked_add(name_len).ok_or_else(|| {
            FozzyError::InvalidArgument("invalid zip: filename length overflow".to_string())
        })?;
        if name_end > cd_end {
            return Err(FozzyError::InvalidArgument(
                "invalid zip: filename out of bounds".to_string(),
            ));
        }
        names.push(bytes[name_start..name_end].to_vec());
        pos = name_end
            .checked_add(extra_len)
            .and_then(|p| p.checked_add(comment_len))
            .ok_or_else(|| {
                FozzyError::InvalidArgument("invalid zip: central directory overflow".to_string())
            })?;
    }

    Ok(names)
}

fn find_eocd_offset(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 22 {
        return None;
    }
    let start = bytes.len().saturating_sub(22 + 65_535);
    (start..=bytes.len() - 22)
        .rev()
        .find(|&i| bytes[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06]))
}

fn read_u16_le(bytes: &[u8], off: usize) -> FozzyResult<u16> {
    if off + 2 > bytes.len() {
        return Err(FozzyError::InvalidArgument(
            "invalid zip: truncated data".to_string(),
        ));
    }
    Ok(u16::from_le_bytes([bytes[off], bytes[off + 1]]))
}

fn read_u32_le(bytes: &[u8], off: usize) -> FozzyResult<u32> {
    if off + 4 > bytes.len() {
        return Err(FozzyError::InvalidArgument(
            "invalid zip: truncated data".to_string(),
        ));
    }
    Ok(u32::from_le_bytes([
        bytes[off],
        bytes[off + 1],
        bytes[off + 2],
        bytes[off + 3],
    ]))
}

fn write_zip_entry_secure(
    out_dir: &Path,
    entry_name: &str,
    reader: &mut dyn Read,
) -> FozzyResult<()> {
    let rel = normalize_zip_entry_rel_path(entry_name)?;
    let out_path = out_dir.join(&rel);

    let mut cur = out_dir.to_path_buf();
    if let Some(parent) = rel.parent() {
        for comp in parent.components() {
            use std::path::Component;
            let Component::Normal(seg) = comp else {
                continue;
            };
            cur.push(seg);
            if cur.exists() {
                let md = std::fs::symlink_metadata(&cur)?;
                if md.file_type().is_symlink() {
                    return Err(FozzyError::InvalidArgument(format!(
                        "refusing to write through symlinked output path: {}",
                        cur.display()
                    )));
                }
            } else {
                std::fs::create_dir(&cur)?;
            }
        }
    }

    if out_path.exists() {
        let md = std::fs::symlink_metadata(&out_path)?;
        if md.file_type().is_symlink() {
            return Err(FozzyError::InvalidArgument(format!(
                "refusing to overwrite symlinked output file: {}",
                out_path.display()
            )));
        }
        if !md.is_file() {
            return Err(FozzyError::InvalidArgument(format!(
                "refusing to overwrite non-file output path: {}",
                out_path.display()
            )));
        }
        return Err(FozzyError::InvalidArgument(format!(
            "refusing to overwrite existing output file: {}",
            out_path.display()
        )));
    }

    let parent = out_path.parent().unwrap_or(out_dir);
    let tmp_name = format!(
        ".{}.{}.{}.tmp",
        out_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("corpus"),
        std::process::id(),
        uuid::Uuid::new_v4()
    );
    let tmp = parent.join(tmp_name);
    let mut out = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&tmp)?;
    std::io::copy(reader, &mut out)?;
    std::fs::rename(&tmp, &out_path)?;
    Ok(())
}

fn validate_zip_target_secure(
    out_dir: &Path,
    rel: &Path,
    seen_targets: &mut HashSet<String>,
) -> FozzyResult<()> {
    let key = portable_rel_key(rel);
    if !seen_targets.insert(key) {
        return Err(FozzyError::InvalidArgument(format!(
            "duplicate output file in archive is not allowed: {}",
            rel.display()
        )));
    }

    let mut cur = out_dir.to_path_buf();
    if let Some(parent) = rel.parent() {
        for comp in parent.components() {
            use std::path::Component;
            let Component::Normal(seg) = comp else {
                continue;
            };
            cur.push(seg);
            if cur.exists() {
                let md = std::fs::symlink_metadata(&cur)?;
                if md.file_type().is_symlink() {
                    return Err(FozzyError::InvalidArgument(format!(
                        "refusing to write through symlinked output path: {}",
                        cur.display()
                    )));
                }
            }
        }
    }

    let out_path = out_dir.join(rel);
    if out_path.exists() {
        let md = std::fs::symlink_metadata(&out_path)?;
        if md.file_type().is_symlink() {
            return Err(FozzyError::InvalidArgument(format!(
                "refusing to overwrite symlinked output file: {}",
                out_path.display()
            )));
        }
        if !md.is_file() {
            return Err(FozzyError::InvalidArgument(format!(
                "refusing to overwrite non-file output path: {}",
                out_path.display()
            )));
        }
        return Err(FozzyError::InvalidArgument(format!(
            "refusing to overwrite existing output file: {}",
            out_path.display()
        )));
    }

    Ok(())
}
