use super::*;

#[test]
fn direct_trace_export_pack_and_bundle_allow_manifest_only_exact_sibling_wrapper() {
    let root = std::env::temp_dir().join(format!(
        "fozzy-direct-trace-manifest-only-sibling-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("mkdir");
    let trace_path = root.join("direct.trace.fozzy");
    let report_path = root.join("report.json");
    let (_, manifest) =
        valid_report_and_manifest_json("r1", &report_path, &root, Some(&trace_path));
    std::fs::write(
        &trace_path,
        valid_trace_json("r1", &trace_path, &report_path, &root),
    )
    .expect("trace");
    std::fs::write(root.join("manifest.json"), manifest).expect("manifest");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: crate::Reporter::Pretty,
        proc_backend: crate::ProcBackend::Scripted,
        fs_backend: crate::FsBackend::Virtual,
        http_backend: crate::HttpBackend::Scripted,
        mem_track: false,
        mem_limit_mb: None,
        mem_fail_after: None,
        fail_on_leak: false,
        leak_budget: None,
        mem_artifacts: false,
        profile_heap_alloc_budget: None,
        profile_heap_in_use_budget: None,
        mem_fragmentation_seed: None,
        mem_pressure_wave: None,
    };
    let out_pack = root.join("pack.zip");
    let out_export = root.join("export.zip");
    let out_bundle = root.join("bundle.zip");

    export_reproducer_pack(&cfg, &trace_path.to_string_lossy(), &out_pack)
        .expect("pack should succeed");
    export_artifacts(&cfg, &trace_path.to_string_lossy(), &out_export)
        .expect("export should succeed");
    export_gate_bundle(&cfg, &trace_path.to_string_lossy(), &out_bundle)
        .expect("bundle should succeed");

    assert!(out_pack.exists());
    assert!(out_export.exists());
    assert!(out_bundle.exists());
}
