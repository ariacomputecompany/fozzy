use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use crate::{Config, FozzyResult, RunSummary, TraceFile};

use super::list::artifacts_list;
use super::{ArtifactDiff, ArtifactFileDelta, ReportDelta, TraceDelta};

pub(super) fn artifacts_diff(
    config: &Config,
    left: &str,
    right: &str,
) -> FozzyResult<ArtifactDiff> {
    let left_entries = artifacts_list(config, left)?;
    let right_entries = artifacts_list(config, right)?;

    let mut left_map = BTreeMap::new();
    for entry in left_entries {
        let file = PathBuf::from(&entry.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&entry.path)
            .to_string();
        let key = format!("{:?}:{file}", entry.kind);
        left_map.insert(key, entry);
    }
    let mut right_map = BTreeMap::new();
    for entry in right_entries {
        let file = PathBuf::from(&entry.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&entry.path)
            .to_string();
        let key = format!("{:?}:{file}", entry.kind);
        right_map.insert(key, entry);
    }

    let mut keys: Vec<String> = left_map.keys().chain(right_map.keys()).cloned().collect();
    keys.sort();
    keys.dedup();

    let mut files = Vec::new();
    for key in keys {
        let l = left_map.get(&key);
        let r = right_map.get(&key);
        let left_path = l.map(|e| e.path.clone());
        let right_path = r.map(|e| e.path.clone());
        let left_size = l.and_then(|e| e.size_bytes);
        let right_size = r.and_then(|e| e.size_bytes);
        let changed = file_delta_changed(left_path.as_deref(), right_path.as_deref())?;
        files.push(ArtifactFileDelta {
            key,
            left_path,
            right_path,
            left_size_bytes: left_size,
            right_size_bytes: right_size,
            changed,
        });
    }

    let report = match (load_summary(config, left)?, load_summary(config, right)?) {
        (Some(l), Some(r)) => Some(report_delta(&l, &r)),
        _ => None,
    };
    let trace = match (load_trace(config, left)?, load_trace(config, right)?) {
        (Some(l), Some(r)) => Some(trace_delta(&l, &r)),
        _ => None,
    };

    Ok(ArtifactDiff {
        left: left.to_string(),
        right: right.to_string(),
        files,
        report,
        trace,
    })
}

fn file_delta_changed(left: Option<&str>, right: Option<&str>) -> FozzyResult<bool> {
    let Some(left) = left else {
        return Ok(right.is_some());
    };
    let Some(right) = right else {
        return Ok(true);
    };
    let left = Path::new(left);
    let right = Path::new(right);
    if !left.exists() || !right.exists() {
        return Ok(left.exists() != right.exists());
    }
    let left_md = std::fs::metadata(left)?;
    let right_md = std::fs::metadata(right)?;
    if left_md.len() != right_md.len() {
        return Ok(true);
    }
    Ok(!files_equal(left, right)?)
}

fn files_equal(left: &Path, right: &Path) -> FozzyResult<bool> {
    let mut left_file = std::fs::File::open(left)?;
    let mut right_file = std::fs::File::open(right)?;
    let mut left_buf = [0u8; 64 * 1024];
    let mut right_buf = [0u8; 64 * 1024];
    loop {
        let left_n = left_file.read(&mut left_buf)?;
        let right_n = right_file.read(&mut right_buf)?;
        if left_n != right_n {
            return Ok(false);
        }
        if left_n == 0 {
            return Ok(true);
        }
        if left_buf[..left_n] != right_buf[..right_n] {
            return Ok(false);
        }
    }
}

fn report_delta(left: &RunSummary, right: &RunSummary) -> ReportDelta {
    let left_titles: Vec<&str> = left.findings.iter().map(|f| f.title.as_str()).collect();
    let right_titles: Vec<&str> = right.findings.iter().map(|f| f.title.as_str()).collect();
    ReportDelta {
        left_status: format!("{:?}", left.status).to_lowercase(),
        right_status: format!("{:?}", right.status).to_lowercase(),
        left_mode: format!("{:?}", left.mode).to_lowercase(),
        right_mode: format!("{:?}", right.mode).to_lowercase(),
        left_findings: left.findings.len(),
        right_findings: right.findings.len(),
        left_duration_ms: left.duration_ms,
        right_duration_ms: right.duration_ms,
        finding_titles_changed: left_titles != right_titles,
    }
}

fn trace_delta(left: &TraceFile, right: &TraceFile) -> TraceDelta {
    let first_decision_diff_index = left
        .decisions
        .iter()
        .zip(right.decisions.iter())
        .position(|(a, b)| a != b)
        .or_else(|| {
            if left.decisions.len() != right.decisions.len() {
                Some(left.decisions.len().min(right.decisions.len()))
            } else {
                None
            }
        });

    let first_event_diff_index = left
        .events
        .iter()
        .zip(right.events.iter())
        .position(|(a, b)| a.time_ms != b.time_ms || a.name != b.name || a.fields != b.fields)
        .or_else(|| {
            if left.events.len() != right.events.len() {
                Some(left.events.len().min(right.events.len()))
            } else {
                None
            }
        });

    TraceDelta {
        left_mode: format!("{:?}", left.mode).to_lowercase(),
        right_mode: format!("{:?}", right.mode).to_lowercase(),
        left_decisions: left.decisions.len(),
        right_decisions: right.decisions.len(),
        left_events: left.events.len(),
        right_events: right.events.len(),
        first_decision_diff_index,
        first_event_diff_index,
    }
}

pub(super) fn load_summary(config: &Config, run: &str) -> FozzyResult<Option<RunSummary>> {
    if let Some(view) = crate::resolve_artifact_selector_view(config, run)? {
        return Ok(Some(match view {
            crate::ArtifactSelectorView::DirectTrace { trace, .. } => trace.summary,
            crate::ArtifactSelectorView::ValidatedBundle(bundle) => bundle.summary,
        }));
    }

    let trace = load_trace(config, run)?;
    Ok(trace.map(|t| t.summary))
}

fn load_trace(config: &Config, run: &str) -> FozzyResult<Option<TraceFile>> {
    let input = crate::normalize_trace_path(&PathBuf::from(run));
    let trace_path = if input.exists()
        && input.is_file()
        && input
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|s| s.eq_ignore_ascii_case("fozzy"))
    {
        input
    } else {
        let artifacts_dir = crate::resolve_artifacts_dir(config, run)?;
        let Some(trace_path) = crate::resolve_trace_path_from_artifacts_dir(&artifacts_dir)? else {
            return Ok(None);
        };
        trace_path
    };

    if !trace_path.exists() {
        return Ok(None);
    }
    Ok(Some(TraceFile::read_json(&trace_path)?))
}
