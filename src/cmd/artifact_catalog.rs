use std::path::{Path, PathBuf};

use crate::ArtifactKind;

const ARTIFACT_FILE_SPECS: &[(&str, ArtifactKind)] = &[
    ("timeline.json", ArtifactKind::Timeline),
    ("profile.timeline.json", ArtifactKind::Profile),
    ("profile.cpu.json", ArtifactKind::Profile),
    ("profile.heap.json", ArtifactKind::Profile),
    ("profile.latency.json", ArtifactKind::Profile),
    ("profile.metrics.json", ArtifactKind::Profile),
    ("symbols.json", ArtifactKind::Profile),
    ("memory.timeline.json", ArtifactKind::Memory),
    ("memory.leaks.json", ArtifactKind::Memory),
    ("memory.graph.json", ArtifactKind::Memory),
    ("memory.delta.json", ArtifactKind::Memory),
    ("report.json", ArtifactKind::Report),
    ("events.json", ArtifactKind::Events),
    ("coverage.json", ArtifactKind::Coverage),
    ("manifest.json", ArtifactKind::Manifest),
    ("report.html", ArtifactKind::Report),
    ("junit.xml", ArtifactKind::Report),
];

pub(crate) fn artifact_file_specs() -> &'static [(&'static str, ArtifactKind)] {
    ARTIFACT_FILE_SPECS
}

pub(crate) fn artifact_file_entries(artifacts_dir: &Path) -> Vec<(ArtifactKind, PathBuf)> {
    ARTIFACT_FILE_SPECS
        .iter()
        .map(|(name, kind)| (kind.clone(), artifacts_dir.join(name)))
        .collect()
}

pub(crate) fn artifact_file_paths(artifacts_dir: &Path) -> Vec<PathBuf> {
    ARTIFACT_FILE_SPECS
        .iter()
        .map(|(name, _)| artifacts_dir.join(name))
        .collect()
}
