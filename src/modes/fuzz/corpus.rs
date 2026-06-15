use std::path::{Path, PathBuf};

use crate::FozzyResult;

pub(crate) fn load_corpus(dir: &Path) -> FozzyResult<Vec<Vec<u8>>> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("bin") {
            continue;
        }
        out.push(std::fs::read(path)?);
    }
    Ok(out)
}

pub(crate) fn persist_corpus_input(dir: &Path, bytes: &[u8]) -> FozzyResult<PathBuf> {
    let name = format!("input-{}.bin", blake3::hash(bytes).to_hex());
    let out = dir.join(name);
    if !out.exists() {
        std::fs::write(&out, bytes)?;
    }
    Ok(out)
}

pub(crate) fn persist_crash_input(dir: &Path, bytes: &[u8]) -> FozzyResult<PathBuf> {
    let name = format!("crash-{}.bin", blake3::hash(bytes).to_hex());
    let out = dir.join("crashes").join(name);
    if !out.exists() {
        std::fs::write(&out, bytes)?;
    }
    Ok(out)
}

pub(crate) fn persist_crash_min_input(dir: &Path, bytes: &[u8]) -> FozzyResult<PathBuf> {
    let name = format!("crash-{}.min.bin", blake3::hash(bytes).to_hex());
    let out = dir.join("crashes").join(name);
    if !out.exists() {
        std::fs::write(&out, bytes)?;
    }
    Ok(out)
}

pub(crate) fn crash_trace_output_path(
    record_path: Option<&Path>,
    artifacts_dir: &Path,
    crash_count: u64,
) -> PathBuf {
    let base = record_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| artifacts_dir.join("trace.fozzy"));
    if crash_count <= 1 {
        return base;
    }
    with_numeric_suffix(&base, crash_count - 1)
}

pub(crate) fn with_numeric_suffix(path: &Path, suffix: u64) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|segment| segment.to_str())
        .unwrap_or("trace");
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => parent.join(format!("{stem}.{suffix}.{ext}")),
        None => parent.join(format!("{stem}.{suffix}")),
    }
}
