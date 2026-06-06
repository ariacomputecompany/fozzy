use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::{FozzyError, FozzyResult, RunManifest, RunSummary, TraceFile};

fn validate_required_bundle_files(files: &[PathBuf], run: &str) -> FozzyResult<()> {
    let present: BTreeSet<String> = files
        .iter()
        .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .collect();
    let required = ["report.json", "manifest.json"];
    let missing: Vec<&str> = required
        .into_iter()
        .filter(|name| !present.contains(*name))
        .collect();
    if !missing.is_empty() {
        return Err(FozzyError::InvalidArgument(format!(
            "incomplete artifacts for {run:?}; missing required files: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

pub(crate) fn validate_run_bundle_integrity(files: &[PathBuf], run: &str) -> FozzyResult<()> {
    let present: BTreeSet<String> = files
        .iter()
        .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .collect();
    let has_report = present.contains("report.json");
    let has_manifest = present.contains("manifest.json");
    let has_trace = files.iter().any(|path| crate::is_trace_path(path));

    if has_report && has_manifest {
        return validate_manifest_integrity(files, run);
    }
    if !has_report && has_manifest && has_trace {
        return validate_manifest_trace_integrity(files, run);
    }

    validate_required_bundle_files(files, run)
}

pub(crate) fn validate_direct_trace_bundle(
    files: &[PathBuf],
    run: &str,
    allow_sidecars_without_metadata: bool,
) -> FozzyResult<()> {
    let present: BTreeSet<String> = files
        .iter()
        .filter_map(|p| p.file_name().map(|s| s.to_string_lossy().to_string()))
        .collect();
    let has_report = present.contains("report.json");
    let has_manifest = present.contains("manifest.json");
    let allows_manifest_only_metadata =
        allow_sidecars_without_metadata && has_manifest && !has_report;
    if has_report != has_manifest && !allows_manifest_only_metadata {
        return Err(FozzyError::InvalidArgument(format!(
            "incomplete direct trace artifacts for {run:?}; report.json and manifest.json must appear together"
        )));
    }
    if !allow_sidecars_without_metadata && !has_report && !has_manifest && files.len() > 1 {
        return Err(FozzyError::InvalidArgument(format!(
            "untrusted sibling artifacts for direct trace {run:?}; report.json and manifest.json are required to trust sibling files"
        )));
    }
    if has_report && has_manifest {
        validate_manifest_integrity(files, run)?;
    } else if allows_manifest_only_metadata {
        validate_manifest_trace_integrity(files, run)?;
    }
    Ok(())
}

pub(crate) fn validate_manifest_trace_integrity(files: &[PathBuf], run: &str) -> FozzyResult<()> {
    let manifest_path = files
        .iter()
        .find(|p| p.file_name().and_then(|s| s.to_str()) == Some("manifest.json"))
        .ok_or_else(|| {
            FozzyError::InvalidArgument(format!(
                "incomplete direct trace artifacts for {run:?}; missing manifest.json"
            ))
        })?;
    let artifacts_dir = manifest_path.parent().ok_or_else(|| {
        FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: unable to resolve artifact directory"
        ))
    })?;
    if crate::load_checked_manifest_trace_summary_from_artifacts_dir(artifacts_dir, run)?.is_none()
    {
        return Err(FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: unable to load manifest-backed trace summary"
        )));
    }
    let summary =
        crate::load_checked_manifest_trace_summary_from_artifacts_dir(artifacts_dir, run)?
            .ok_or_else(|| {
                FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: unable to load manifest-backed trace summary"
                ))
            })?;
    let trace_path =
        crate::resolve_trace_path_from_artifacts_dir(artifacts_dir)?.ok_or_else(|| {
            FozzyError::InvalidArgument(format!(
                "invalid manifest for {run:?}: missing declared trace artifact"
            ))
        })?;
    let trace = TraceFile::read_json(&trace_path)?;
    validate_profile_artifact_identities(
        files,
        run,
        &trace.summary.identity.run_id,
        trace.summary.identity.seed,
    )?;
    validate_memory_artifact_coherence(files, run, summary.memory.as_ref())?;
    validate_reporter_artifacts(files, run, &summary)?;
    validate_trace_event_artifacts(files, run, &trace.events)?;
    Ok(())
}

pub(crate) fn validate_manifest_integrity(files: &[PathBuf], run: &str) -> FozzyResult<()> {
    let manifest_path = files
        .iter()
        .find(|p| p.file_name().and_then(|s| s.to_str()) == Some("manifest.json"))
        .ok_or_else(|| {
            FozzyError::InvalidArgument(format!(
                "incomplete artifacts for {run:?}; missing required files: manifest.json"
            ))
        })?;
    let bytes = std::fs::read(manifest_path)?;
    let manifest: RunManifest = serde_json::from_slice(&bytes).map_err(|e| {
        FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: {} ({e})",
            manifest_path.display()
        ))
    })?;
    if manifest.schema_version != "fozzy.run_manifest.v1" {
        return Err(FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: unsupported schemaVersion {}",
            manifest.schema_version
        )));
    }
    let report_path = files
        .iter()
        .find(|p| p.file_name().and_then(|s| s.to_str()) == Some("report.json"))
        .ok_or_else(|| {
            FozzyError::InvalidArgument(format!(
                "incomplete artifacts for {run:?}; missing required files: report.json"
            ))
        })?;
    let report_bytes = std::fs::read(report_path)?;
    let report: RunSummary = serde_json::from_slice(&report_bytes).map_err(|e| {
        FozzyError::InvalidArgument(format!(
            "invalid report for {run:?}: {} ({e})",
            report_path.display()
        ))
    })?;
    let expected_artifacts_dir = report_path
        .parent()
        .ok_or_else(|| {
            FozzyError::InvalidArgument(format!(
                "invalid report for {run:?}: {} has no parent directory",
                report_path.display()
            ))
        })?
        .to_string_lossy()
        .to_string();
    let report_path_string = report_path.to_string_lossy().to_string();
    let trace_path = files
        .iter()
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("fozzy"));
    let report_trace_path = report.identity.trace_path.clone();
    let manifest_trace_path = manifest.trace_path.clone();
    if manifest.run_id != report.identity.run_id
        || manifest.mode != report.mode
        || manifest.status != report.status
        || manifest.seed != report.identity.seed
        || manifest.duration_ms != report.duration_ms
        || manifest.duration_ns != report.duration_ns
        || manifest.findings_count != report.findings.len()
        || manifest.report_path.as_ref() != Some(&report_path_string)
        || manifest.artifacts_dir.as_ref() != Some(&expected_artifacts_dir)
        || report.identity.report_path.as_ref() != Some(&report_path_string)
        || report.identity.artifacts_dir.as_ref() != Some(&expected_artifacts_dir)
        || manifest_trace_path != report_trace_path
    {
        return Err(FozzyError::InvalidArgument(format!(
            "invalid manifest for {run:?}: report/manifest identity mismatch"
        )));
    }
    match manifest_trace_path {
        Some(ref expected_trace) => {
            let actual_trace = trace_path.ok_or_else(|| {
                FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: missing declared trace artifact {expected_trace}"
                ))
            })?;
            if actual_trace.to_string_lossy() != expected_trace.as_str() {
                return Err(FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: declared trace artifact mismatch"
                )));
            }
            let trace_bytes = std::fs::read(actual_trace)?;
            let trace: TraceFile = serde_json::from_slice(&trace_bytes).map_err(|e| {
                FozzyError::InvalidArgument(format!(
                    "invalid trace for {run:?}: {} ({e})",
                    actual_trace.display()
                ))
            })?;
            if trace.summary.identity.trace_path.as_ref() != Some(expected_trace) {
                return Err(FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: trace/report identity mismatch"
                )));
            }
            if manifest.mode == crate::RunMode::Replay {
                if trace.summary.status != manifest.status
                    || trace.summary.identity.seed != manifest.seed
                {
                    return Err(FozzyError::InvalidArgument(format!(
                        "invalid manifest for {run:?}: replay source trace mismatch"
                    )));
                }
            } else if trace.summary.identity.run_id != manifest.run_id
                || trace.summary.mode != manifest.mode
                || trace.summary.status != manifest.status
                || trace.summary.identity.seed != manifest.seed
                || trace.summary.identity.report_path.as_ref() != Some(&report_path_string)
                || trace.summary.identity.artifacts_dir.as_ref() != Some(&expected_artifacts_dir)
            {
                return Err(FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: trace/report identity mismatch"
                )));
            }
        }
        None => {
            if trace_path.is_some() {
                return Err(FozzyError::InvalidArgument(format!(
                    "invalid manifest for {run:?}: undeclared trace artifact present"
                )));
            }
        }
    }
    validate_memory_artifact_coherence(files, run, report.memory.as_ref())?;
    validate_reporter_artifacts(files, run, &report)?;
    if let Some(trace_path) = trace_path {
        let trace = TraceFile::read_json(trace_path)?;
        validate_profile_artifact_identities(
            files,
            run,
            &trace.summary.identity.run_id,
            trace.summary.identity.seed,
        )?;
        validate_trace_event_artifacts(files, run, &trace.events)?;
    } else {
        validate_profile_artifact_identities(files, run, &manifest.run_id, manifest.seed)?;
    }
    Ok(())
}

fn validate_profile_artifact_identities(
    files: &[PathBuf],
    run: &str,
    expected_run_id: &str,
    expected_seed: u64,
) -> FozzyResult<()> {
    if let Some(metrics_path) = find_artifact_path(files, "profile.metrics.json") {
        let metrics: crate::ProfileMetrics = serde_json::from_slice(&std::fs::read(metrics_path)?)
            .map_err(|e| {
                FozzyError::InvalidArgument(format!(
                    "invalid profile metrics for {run:?}: {} ({e})",
                    metrics_path.display()
                ))
            })?;
        if metrics.run_id != expected_run_id {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid profile metrics for {run:?}: {} belong to runId={}, expected {expected_run_id}",
                metrics_path.display(),
                metrics.run_id
            )));
        }
    }

    if let Some(timeline_path) = find_artifact_path(files, "profile.timeline.json") {
        let timeline: crate::ProfileTimelineArtifact =
            serde_json::from_slice(&std::fs::read(timeline_path)?).map_err(|e| {
                FozzyError::InvalidArgument(format!(
                    "invalid profile timeline for {run:?}: {} ({e})",
                    timeline_path.display()
                ))
            })?;
        if timeline.run_id != expected_run_id {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid profile timeline for {run:?}: {} belong to runId={}, expected {expected_run_id}",
                timeline_path.display(),
                timeline.run_id
            )));
        }
        if let Some(event) = timeline
            .events
            .iter()
            .find(|event| event.run_id != expected_run_id || event.seed != expected_seed)
        {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid profile timeline for {run:?}: {} contains event identity runId={} seed={}, expected runId={} seed={}",
                timeline_path.display(),
                event.run_id,
                event.seed,
                expected_run_id,
                expected_seed
            )));
        }
    }

    validate_profile_run_scoped_artifact::<crate::CpuProfile>(
        files,
        run,
        "profile.cpu.json",
        expected_run_id,
        |profile| profile.run_id.as_str(),
        "profile cpu artifact",
    )?;
    validate_profile_run_scoped_artifact::<crate::HeapProfile>(
        files,
        run,
        "profile.heap.json",
        expected_run_id,
        |profile| profile.run_id.as_str(),
        "profile heap artifact",
    )?;
    validate_profile_run_scoped_artifact::<crate::LatencyProfile>(
        files,
        run,
        "profile.latency.json",
        expected_run_id,
        |profile| profile.run_id.as_str(),
        "profile latency artifact",
    )?;
    validate_profile_run_scoped_artifact::<crate::SymbolsMap>(
        files,
        run,
        "symbols.json",
        expected_run_id,
        |symbols| symbols.run_id.as_str(),
        "profile symbols artifact",
    )?;
    Ok(())
}

fn validate_memory_artifact_coherence(
    files: &[PathBuf],
    run: &str,
    summary: Option<&crate::MemorySummary>,
) -> FozzyResult<()> {
    let leaks_path = find_artifact_path(files, "memory.leaks.json");
    let graph_path = find_artifact_path(files, "memory.graph.json");
    let timeline_path = find_artifact_path(files, "memory.timeline.json");
    let delta_path = find_artifact_path(files, "memory.delta.json");
    if leaks_path.is_none()
        && graph_path.is_none()
        && timeline_path.is_none()
        && delta_path.is_none()
    {
        return Ok(());
    }
    let summary = summary.ok_or_else(|| {
        FozzyError::InvalidArgument(format!(
            "invalid memory artifacts for {run:?}: memory sidecars are present but the wrapper summary has no memory section"
        ))
    })?;

    if let Some(leaks_path) = leaks_path {
        let leaks: Vec<crate::MemoryLeak> = serde_json::from_slice(&std::fs::read(leaks_path)?)
            .map_err(|e| {
                FozzyError::InvalidArgument(format!(
                    "invalid memory leaks for {run:?}: {} ({e})",
                    leaks_path.display()
                ))
            })?;
        let leaked_allocs = leaks.len() as u64;
        let leaked_bytes: u64 = leaks.iter().map(|leak| leak.bytes).sum();
        if leaked_allocs != summary.leaked_allocs || leaked_bytes != summary.leaked_bytes {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid memory leaks for {run:?}: {} do not match summary leaked_bytes={} leaked_allocs={}, sidecar leaked_bytes={} leaked_allocs={}",
                leaks_path.display(),
                summary.leaked_bytes,
                summary.leaked_allocs,
                leaked_bytes,
                leaked_allocs
            )));
        }
    }

    if let Some(graph_path) = graph_path {
        let graph = crate::read_cached_memory_graph(graph_path).map_err(|e| {
            FozzyError::InvalidArgument(format!(
                "invalid memory graph for {run:?}: {} ({e})",
                graph_path.display()
            ))
        })?;
        crate::validate_memory_graph_structure(&graph, graph_path).map_err(|e| {
            FozzyError::InvalidArgument(format!(
                "invalid memory graph for {run:?}: {} ({e})",
                graph_path.display()
            ))
        })?;
        let alloc_nodes = graph
            .nodes
            .iter()
            .filter(|node| node.kind == "alloc")
            .count() as u64;
        let free_nodes = graph
            .nodes
            .iter()
            .filter(|node| node.kind == "free")
            .count() as u64;
        let allocates_edges = graph
            .edges
            .iter()
            .filter(|edge| edge.kind == "allocates")
            .count() as u64;
        let freed_by_edges = graph
            .edges
            .iter()
            .filter(|edge| edge.kind == "freed_by")
            .count() as u64;
        let successful_allocs = summary.free_count.saturating_add(summary.leaked_allocs);
        if alloc_nodes != successful_allocs
            || free_nodes != summary.free_count
            || allocates_edges != successful_allocs
            || freed_by_edges != summary.free_count
        {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid memory graph for {run:?}: {} does not match summary successful_allocs={} free_count={} leaked_allocs={}, graph alloc_nodes={} free_nodes={} allocates_edges={} freed_by_edges={}",
                graph_path.display(),
                successful_allocs,
                summary.free_count,
                summary.leaked_allocs,
                alloc_nodes,
                free_nodes,
                allocates_edges,
                freed_by_edges
            )));
        }
    }

    if let Some(timeline_path) = timeline_path {
        let timeline: Vec<crate::MemoryTimelineEntry> =
            serde_json::from_slice(&std::fs::read(timeline_path)?).map_err(|e| {
                FozzyError::InvalidArgument(format!(
                    "invalid memory timeline for {run:?}: {} ({e})",
                    timeline_path.display()
                ))
            })?;
        let alloc_count = timeline
            .iter()
            .filter(|entry| entry.kind == "alloc")
            .count() as u64;
        let free_count = timeline.iter().filter(|entry| entry.kind == "free").count() as u64;
        let failed_alloc_count = timeline
            .iter()
            .filter(|entry| entry.kind == "alloc_fail")
            .count() as u64;
        if alloc_count != summary.alloc_count
            || free_count != summary.free_count
            || failed_alloc_count != summary.failed_alloc_count
        {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid memory timeline for {run:?}: {} does not match summary alloc_count={} free_count={} failed_alloc_count={}, timeline alloc={} free={} alloc_fail={}",
                timeline_path.display(),
                summary.alloc_count,
                summary.free_count,
                summary.failed_alloc_count,
                alloc_count,
                free_count,
                failed_alloc_count
            )));
        }
    }

    if let Some(delta_path) = delta_path {
        let delta: crate::MemoryDelta = serde_json::from_slice(&std::fs::read(delta_path)?)
            .map_err(|e| {
                FozzyError::InvalidArgument(format!(
                    "invalid memory delta for {run:?}: {} ({e})",
                    delta_path.display()
                ))
            })?;
        if delta.after_leaked_bytes != summary.leaked_bytes
            || delta.after_leaked_allocs != summary.leaked_allocs
            || delta.after_alloc_count != summary.alloc_count
        {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid memory delta for {run:?}: {} does not match summary after_leaked_bytes={} after_leaked_allocs={} after_alloc_count={}, delta after_leaked_bytes={} after_leaked_allocs={} after_alloc_count={}",
                delta_path.display(),
                summary.leaked_bytes,
                summary.leaked_allocs,
                summary.alloc_count,
                delta.after_leaked_bytes,
                delta.after_leaked_allocs,
                delta.after_alloc_count
            )));
        }
    }

    Ok(())
}

fn validate_trace_event_artifacts(
    files: &[PathBuf],
    run: &str,
    expected_events: &[crate::TraceEvent],
) -> FozzyResult<()> {
    if let Some(events_path) = find_artifact_path(files, "events.json") {
        let events: Vec<crate::TraceEvent> = serde_json::from_slice(&std::fs::read(events_path)?)
            .map_err(|e| {
            FozzyError::InvalidArgument(format!(
                "invalid events artifact for {run:?}: {} ({e})",
                events_path.display()
            ))
        })?;
        let matches = events.len() == expected_events.len()
            && events
                .iter()
                .zip(expected_events.iter())
                .all(|(actual, expected)| {
                    actual.time_ms == expected.time_ms
                        && actual.name == expected.name
                        && actual.fields == expected.fields
                });
        if !matches {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid events artifact for {run:?}: {} does not match trace events",
                events_path.display()
            )));
        }
    }

    if let Some(timeline_path) = find_artifact_path(files, "timeline.json") {
        let timeline: Vec<crate::TimelineEntry> =
            serde_json::from_slice(&std::fs::read(timeline_path)?).map_err(|e| {
                FozzyError::InvalidArgument(format!(
                    "invalid timeline artifact for {run:?}: {} ({e})",
                    timeline_path.display()
                ))
            })?;
        let expected_timeline = expected_events
            .iter()
            .enumerate()
            .map(|(index, event)| crate::TimelineEntry {
                index,
                time_ms: event.time_ms,
                name: event.name.clone(),
                fields: event.fields.clone(),
            })
            .collect::<Vec<_>>();
        let matches = timeline.len() == expected_timeline.len()
            && timeline
                .iter()
                .zip(expected_timeline.iter())
                .all(|(actual, expected)| {
                    actual.index == expected.index
                        && actual.time_ms == expected.time_ms
                        && actual.name == expected.name
                        && actual.fields == expected.fields
                });
        if !matches {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid timeline artifact for {run:?}: {} does not match trace events",
                timeline_path.display()
            )));
        }
    }

    Ok(())
}

fn validate_reporter_artifacts(
    files: &[PathBuf],
    run: &str,
    summary: &RunSummary,
) -> FozzyResult<()> {
    if let Some(html_path) = find_artifact_path(files, "report.html") {
        let actual = std::fs::read(html_path)?;
        let expected = crate::render_html(summary).into_bytes();
        if actual != expected {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid html report for {run:?}: {} does not match summary rendering",
                html_path.display()
            )));
        }
    }

    if let Some(junit_path) = find_artifact_path(files, "junit.xml") {
        let actual = std::fs::read(junit_path)?;
        let expected = crate::render_junit_xml(summary).into_bytes();
        if actual != expected {
            return Err(FozzyError::InvalidArgument(format!(
                "invalid junit report for {run:?}: {} does not match summary rendering",
                junit_path.display()
            )));
        }
    }

    Ok(())
}

fn validate_profile_run_scoped_artifact<T>(
    files: &[PathBuf],
    run: &str,
    artifact_name: &str,
    expected_run_id: &str,
    run_id_for: impl Fn(&T) -> &str,
    label: &str,
) -> FozzyResult<()>
where
    T: DeserializeOwned,
{
    let Some(path) = find_artifact_path(files, artifact_name) else {
        return Ok(());
    };
    let artifact: T = serde_json::from_slice(&std::fs::read(path)?).map_err(|e| {
        FozzyError::InvalidArgument(format!(
            "invalid {label} for {run:?}: {} ({e})",
            path.display()
        ))
    })?;
    let actual_run_id = run_id_for(&artifact);
    if actual_run_id != expected_run_id {
        return Err(FozzyError::InvalidArgument(format!(
            "invalid {label} for {run:?}: {} belong to runId={}, expected {expected_run_id}",
            path.display(),
            actual_run_id
        )));
    }
    Ok(())
}

fn find_artifact_path<'a>(files: &'a [PathBuf], artifact_name: &str) -> Option<&'a Path> {
    files
        .iter()
        .find(|path| path.file_name().and_then(|s| s.to_str()) == Some(artifact_name))
        .map(PathBuf::as_path)
}
