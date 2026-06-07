#[allow(unused_imports)]
use super::*;

#[test]
fn stale_direct_trace_profile_sidecars_are_rebuilt_in_memory_without_mutating_artifacts() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-profile-stale-read-no-mutate-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("create root");
    let trace_path = root.join("trace.fozzy");
    let artifacts_dir = root.join("artifacts");

    let mut trace = sample_trace_with_full_profile_support();
    trace.summary.identity.trace_path = Some(trace_path.to_string_lossy().to_string());
    trace.summary.identity.artifacts_dir = Some(artifacts_dir.to_string_lossy().to_string());
    trace.write_json(&trace_path).expect("write trace");

    write_profile_artifacts_from_trace_with_source(&trace, Some(&trace_path), &artifacts_dir)
        .expect("write profile artifacts");

    let source_path = artifacts_dir.join("profile.source.json");
    let original_source = std::fs::read(&source_path).expect("read profile source");
    let mut source_json: serde_json::Value =
        serde_json::from_slice(&original_source).expect("parse profile source");
    source_json["traceModifiedNs"] = serde_json::json!(0u64);
    std::fs::write(
        &source_path,
        serde_json::to_vec(&source_json).expect("encode stale source"),
    )
    .expect("write stale source");
    let stale_source = std::fs::read(&source_path).expect("read stale source");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        ..crate::Config::default()
    };
    let bundle = load_profile_bundle(
        &cfg,
        &trace_path.to_string_lossy(),
        ProfileLoadSpec {
            timeline: true,
            cpu: true,
            heap: true,
            latency: true,
            symbols: true,
        },
    )
    .expect("load bundle from stale sidecars");

    assert_eq!(bundle.metrics.run_id, trace.summary.identity.run_id);
    assert!(
        bundle.timeline.is_some(),
        "timeline should be rebuilt in-memory"
    );
    assert!(bundle.cpu.is_some(), "cpu should be rebuilt in-memory");
    assert!(bundle.heap.is_some(), "heap should be rebuilt in-memory");
    assert!(
        bundle.latency.is_some(),
        "latency should be rebuilt in-memory"
    );
    assert!(
        bundle.symbols.is_some(),
        "symbols should be rebuilt in-memory"
    );

    let source_after = std::fs::read(&source_path).expect("read source after load");
    assert_eq!(
        source_after, stale_source,
        "profile read path must not rewrite stale profile.source.json"
    );
}
