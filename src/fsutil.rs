//! Small filesystem utilities.

use globset::{Glob, GlobSet, GlobSetBuilder};

use std::path::PathBuf;

use walkdir::WalkDir;

use crate::{FozzyError, FozzyResult};

pub fn find_matching_files(patterns: &[String]) -> FozzyResult<Vec<PathBuf>> {
    let set = compile_globset(patterns)?;
    let mut out = Vec::new();
    for entry in WalkDir::new(".").follow_links(false) {
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
        let p = entry.path();
        let rel = p.strip_prefix(".").unwrap_or(p);
        if set.is_match(rel) {
            out.push(rel.to_path_buf());
        }
    }
    out.sort();
    Ok(out)
}

fn compile_globset(patterns: &[String]) -> FozzyResult<GlobSet> {
    let mut b = GlobSetBuilder::new();
    for p in patterns {
        let g = Glob::new(p).map_err(|e| FozzyError::InvalidArgument(format!("invalid glob {p:?}: {e}")))?;
        b.add(g);
    }
    b.build()
        .map_err(|e| FozzyError::InvalidArgument(format!("invalid globset: {e}")))
}
