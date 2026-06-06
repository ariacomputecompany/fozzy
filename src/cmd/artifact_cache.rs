use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use crate::{FozzyResult, RunManifest, RunSummary, TraceFile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FileFingerprint {
    pub len: u64,
    pub modified_ns: u128,
}

impl FileFingerprint {
    pub(crate) fn for_path(path: &Path) -> FozzyResult<Self> {
        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified()?;
        let modified_ns = modified
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Ok(Self {
            len: metadata.len(),
            modified_ns,
        })
    }
}

#[derive(Debug, Clone)]
struct CachedValue<T> {
    fingerprint: FileFingerprint,
    value: T,
}

fn load_or_reuse<T: Clone>(
    cache: &Mutex<HashMap<PathBuf, CachedValue<T>>>,
    path: &Path,
    loader: impl FnOnce(&Path) -> FozzyResult<T>,
) -> FozzyResult<T> {
    let fingerprint = FileFingerprint::for_path(path)?;
    if let Some(cached) = cache
        .lock()
        .expect("artifact cache poisoned")
        .get(path)
        .cloned()
        .filter(|cached| cached.fingerprint == fingerprint)
    {
        return Ok(cached.value);
    }

    let value = loader(path)?;
    cache.lock().expect("artifact cache poisoned").insert(
        path.to_path_buf(),
        CachedValue {
            fingerprint,
            value: value.clone(),
        },
    );
    Ok(value)
}

fn summary_cache() -> &'static Mutex<HashMap<PathBuf, CachedValue<RunSummary>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedValue<RunSummary>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn manifest_cache() -> &'static Mutex<HashMap<PathBuf, CachedValue<RunManifest>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedValue<RunManifest>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn trace_cache() -> &'static Mutex<HashMap<PathBuf, CachedValue<TraceFile>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedValue<TraceFile>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn memory_graph_cache() -> &'static Mutex<HashMap<PathBuf, CachedValue<crate::MemoryGraph>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedValue<crate::MemoryGraph>>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn read_cached_run_summary(path: &Path) -> FozzyResult<RunSummary> {
    load_or_reuse(summary_cache(), path, |path| {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    })
}

pub(crate) fn read_cached_run_manifest(path: &Path) -> FozzyResult<RunManifest> {
    load_or_reuse(manifest_cache(), path, |path| {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    })
}

pub(crate) fn read_cached_trace_file(path: &Path) -> FozzyResult<TraceFile> {
    load_or_reuse(trace_cache(), path, TraceFile::read_json)
}

pub(crate) fn read_cached_memory_graph(path: &Path) -> FozzyResult<crate::MemoryGraph> {
    load_or_reuse(memory_graph_cache(), path, |path| {
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    })
}
