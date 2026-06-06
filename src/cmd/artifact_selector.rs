use crate::{Config, FozzyResult, TraceFile, ValidatedArtifactBundle};

#[derive(Debug, Clone)]
pub(crate) enum ArtifactSelectorView {
    DirectTrace { trace: TraceFile },
    ValidatedBundle(ValidatedArtifactBundle),
}

pub(crate) fn resolve_artifact_selector_view(
    config: &Config,
    selector: &str,
) -> FozzyResult<Option<ArtifactSelectorView>> {
    let input = std::path::PathBuf::from(crate::normalize_run_or_trace_selector(selector));
    if input.exists() && input.is_file() && crate::is_trace_path(&input) {
        let trace = crate::read_cached_trace_file(&input)?;
        return Ok(Some(ArtifactSelectorView::DirectTrace { trace }));
    }

    Ok(crate::load_validated_artifact_bundle(config, selector)?
        .map(ArtifactSelectorView::ValidatedBundle))
}
