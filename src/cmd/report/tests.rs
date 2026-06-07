use super::*;
use crate::{ExitStatus, Finding, FindingKind, RunIdentity, RunMode};
use uuid::Uuid;

fn write_summary(base: &std::path::Path, run_id: &str, status: ExitStatus) -> String {
    let dir = base.join(run_id);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let summary = RunSummary {
        status,
        mode: RunMode::Run,
        identity: RunIdentity {
            run_id: run_id.to_string(),
            seed: 1,
            trace_path: None,
            report_path: Some(dir.join("report.json").to_string_lossy().to_string()),
            artifacts_dir: Some(dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 0,
        duration_ns: 0,
        tests: None,
        memory: None,
        findings: if status == ExitStatus::Pass {
            Vec::new()
        } else {
            vec![Finding {
                kind: FindingKind::Assertion,
                title: "boom".to_string(),
                message: "x".to_string(),
                location: None,
            }]
        },
    };
    std::fs::write(
        dir.join("report.json"),
        serde_json::to_vec_pretty(&summary).expect("json"),
    )
    .expect("write");
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::json!({
            "schemaVersion": "fozzy.run_manifest.v1",
            "runId": run_id,
            "mode": "run",
            "status": if status == ExitStatus::Pass { "pass" } else { "fail" },
            "seed": 1,
            "startedAt": "2026-01-01T00:00:00Z",
            "finishedAt": "2026-01-01T00:00:00Z",
            "durationMs": 0,
            "durationNs": 0,
            "tracePath": serde_json::Value::Null,
            "reportPath": dir.join("report.json"),
            "artifactsDir": dir,
            "findingsCount": summary.findings.len()
        })
        .to_string(),
    )
    .expect("write manifest");
    run_id.to_string()
}

#[test]
fn query_accepts_dot_index_form() {
    let value = serde_json::json!({
        "findings": [{"title": "oops"}]
    });
    let out = query_value(&value, ".findings.0.title").expect("query");
    assert_eq!(out, serde_json::Value::String("oops".to_string()));
}

#[test]
fn query_run_id_alias_maps_to_identity() {
    let value = serde_json::json!({
        "identity": {"runId": "run-123"}
    });
    let out = query_value(&value, "runId").expect("query");
    assert_eq!(out, serde_json::Value::String("run-123".to_string()));
}

#[test]
fn query_identity_aliases_cover_all_documented_fields() {
    let value = serde_json::json!({
        "identity": {
            "runId": "run-123",
            "seed": 7,
            "tracePath": "t.fozzy",
            "reportPath": "r.json",
            "artifactsDir": ".fozzy/runs/run-123"
        }
    });
    let cases = [
        ("runId", serde_json::json!("run-123")),
        ("seed", serde_json::json!(7)),
        ("tracePath", serde_json::json!("t.fozzy")),
        ("reportPath", serde_json::json!("r.json")),
        ("artifactsDir", serde_json::json!(".fozzy/runs/run-123")),
        ("identity.runId", serde_json::json!("run-123")),
    ];
    for (expr, expected) in cases {
        let out = query_value(&value, expr).expect("query");
        assert_eq!(out, expected, "expr={expr}");
    }
}

#[test]
fn query_miss_reports_suggestion() {
    let value = serde_json::json!({
        "identity": {"runId": "run-123"}
    });
    let err = query_value(&value, "runid").expect_err("must miss");
    assert!(err.to_string().contains("did you mean"));
    assert!(err.to_string().contains("identity.runId"));
}

#[test]
fn list_paths_exposes_identity_shape() {
    let value = serde_json::json!({
        "identity": {"runId": "run-123", "seed": 1},
        "findings": [{"title": "oops"}]
    });
    let paths = list_query_paths(&value);
    assert!(paths.contains(&".".to_string()));
    assert!(paths.contains(&"identity.runId".to_string()));
    assert!(paths.contains(&"findings[0].title".to_string()));
}

#[test]
fn flaky_budget_enforced() {
    let root = std::env::temp_dir().join(format!("fozzy-flaky-{}", Uuid::new_v4()));
    let runs = root.join(".fozzy").join("runs");
    std::fs::create_dir_all(&runs).expect("mkdir");
    let a = write_summary(&runs, "r1", ExitStatus::Pass);
    let b = write_summary(&runs, "r2", ExitStatus::Fail);
    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: Reporter::Json,
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

    let out = flaky_command(
        &cfg,
        &[a.clone(), b.clone()],
        Some("60".parse::<crate::FlakeBudget>().expect("budget parse")),
    )
    .expect("within budget");
    let obj = out.as_object().expect("obj");
    assert!(obj.get("flakeRatePct").is_some());

    let err = flaky_command(
        &cfg,
        &[a, b],
        Some("10".parse::<crate::FlakeBudget>().expect("budget parse")),
    )
    .expect_err("over budget");
    assert!(err.to_string().contains("flake budget exceeded"));
}

#[test]
fn flaky_rejects_duplicate_run_references() {
    let root = std::env::temp_dir().join(format!("fozzy-flaky-dup-{}", Uuid::new_v4()));
    let runs = root.join(".fozzy").join("runs");
    std::fs::create_dir_all(&runs).expect("mkdir");
    let a = write_summary(&runs, "r1", ExitStatus::Pass);
    let b = write_summary(&runs, "r2", ExitStatus::Fail);
    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: Reporter::Json,
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

    let err = flaky_command(&cfg, &[a.clone(), a, b], None).expect_err("must reject duplicates");
    assert!(err.to_string().contains("duplicate run reference"));
}

#[test]
fn load_summary_uses_manifest_declared_external_trace_when_report_missing() {
    let root = std::env::temp_dir().join(format!("fozzy-report-trace-{}", Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let external_trace = root.join("external.trace.fozzy");
    let summary = RunSummary {
        status: ExitStatus::Pass,
        mode: RunMode::Run,
        identity: RunIdentity {
            run_id: "r1".to_string(),
            seed: 1,
            trace_path: Some(external_trace.to_string_lossy().to_string()),
            report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
            artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 0,
        duration_ns: 0,
        tests: None,
        memory: None,
        findings: Vec::new(),
    };
    let trace = TraceFile {
        format: crate::TRACE_FORMAT.to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Run,
        scenario_path: None,
        scenario: Some(crate::ScenarioV1Steps {
            version: 1,
            name: "x".to_string(),
            steps: Vec::new(),
        }),
        fuzz: None,
        explore: None,
        memory: None,
        decisions: Vec::new(),
        events: Vec::new(),
        summary: summary.clone(),
        checksum: None,
    };
    trace.write_json(&external_trace).expect("write trace");
    std::fs::write(
        run_dir.join("manifest.json"),
        serde_json::json!({
            "schemaVersion": "fozzy.run_manifest.v1",
            "runId": "r1",
            "mode": "run",
            "status": "pass",
            "seed": 1,
            "startedAt": "2026-01-01T00:00:00Z",
            "finishedAt": "2026-01-01T00:00:00Z",
            "durationMs": 0,
            "durationNs": 0,
            "tracePath": external_trace,
            "reportPath": run_dir.join("report.json"),
            "artifactsDir": run_dir,
            "findingsCount": 0
        })
        .to_string(),
    )
    .expect("write manifest");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: Reporter::Json,
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

    let loaded = load_summary(&cfg, "r1").expect("load summary");
    assert_eq!(loaded.identity.run_id, "r1");
    assert_eq!(
        loaded.identity.trace_path.as_deref(),
        Some(external_trace.to_string_lossy().as_ref())
    );
}

#[test]
fn load_summary_prefers_explicit_trace_over_sibling_report() {
    let root = std::env::temp_dir().join(format!("fozzy-report-direct-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("mkdir");
    let artifacts_dir = root.join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).expect("artifacts dir");
    let trace_path = root.join("direct.trace.fozzy");
    let trace_summary = RunSummary {
        status: ExitStatus::Fail,
        mode: RunMode::Replay,
        identity: RunIdentity {
            run_id: "trace-run".to_string(),
            seed: 7,
            trace_path: Some(trace_path.to_string_lossy().to_string()),
            report_path: Some(
                artifacts_dir
                    .join("report.json")
                    .to_string_lossy()
                    .to_string(),
            ),
            artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 5,
        duration_ns: 5_000_000,
        tests: None,
        memory: None,
        findings: vec![Finding {
            kind: FindingKind::Assertion,
            title: "trace".to_string(),
            message: "from trace".to_string(),
            location: None,
        }],
    };
    let report_summary = RunSummary {
        status: ExitStatus::Pass,
        mode: RunMode::Run,
        identity: RunIdentity {
            run_id: "report-run".to_string(),
            seed: 1,
            trace_path: Some(root.join("other.trace.fozzy").to_string_lossy().to_string()),
            report_path: Some(
                artifacts_dir
                    .join("report.json")
                    .to_string_lossy()
                    .to_string(),
            ),
            artifacts_dir: Some(artifacts_dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 0,
        duration_ns: 0,
        tests: None,
        memory: None,
        findings: Vec::new(),
    };
    let trace = TraceFile {
        format: crate::TRACE_FORMAT.to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: trace_summary.mode,
        scenario_path: None,
        scenario: Some(crate::ScenarioV1Steps {
            version: 1,
            name: "x".to_string(),
            steps: Vec::new(),
        }),
        fuzz: None,
        explore: None,
        memory: None,
        decisions: Vec::new(),
        events: Vec::new(),
        summary: trace_summary.clone(),
        checksum: None,
    };
    trace.write_json(&trace_path).expect("write trace");
    std::fs::write(
        artifacts_dir.join("report.json"),
        serde_json::to_vec_pretty(&report_summary).expect("report json"),
    )
    .expect("write report");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: Reporter::Json,
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

    let loaded = load_summary(&cfg, &trace_path.to_string_lossy()).expect("load summary");
    assert_eq!(loaded.identity.run_id, "trace-run");
    assert_eq!(loaded.status, ExitStatus::Fail);
    assert_eq!(loaded.mode, RunMode::Replay);
    assert_eq!(loaded.findings.len(), 1);
}

#[test]
fn load_summary_rejects_stale_report_without_manifest() {
    let root = std::env::temp_dir().join(format!("fozzy-report-stale-report-{}", Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let external_trace = root.join("external.trace.fozzy");
    let summary = RunSummary {
        status: ExitStatus::Pass,
        mode: RunMode::Run,
        identity: RunIdentity {
            run_id: "r1".to_string(),
            seed: 1,
            trace_path: Some(external_trace.to_string_lossy().to_string()),
            report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
            artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
        },
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 0,
        duration_ns: 0,
        tests: None,
        memory: None,
        findings: Vec::new(),
    };
    std::fs::write(
        run_dir.join("report.json"),
        serde_json::to_vec_pretty(&summary).expect("report json"),
    )
    .expect("write report");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: Reporter::Json,
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

    let err = load_summary(&cfg, "r1").expect_err("must reject stale report");
    assert!(
        err.to_string()
            .contains("missing required files: manifest.json")
    );
}

#[test]
fn load_summary_rejects_trace_only_run_wrapper_without_report_manifest() {
    let root = std::env::temp_dir().join(format!("fozzy-report-trace-only-{}", Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let trace_path = run_dir.join("trace.fozzy");

    let trace = TraceFile {
        format: crate::TRACE_FORMAT.to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Run,
        scenario_path: None,
        scenario: Some(crate::ScenarioV1Steps {
            version: 1,
            name: "x".to_string(),
            steps: Vec::new(),
        }),
        fuzz: None,
        explore: None,
        memory: None,
        decisions: Vec::new(),
        events: Vec::new(),
        summary: RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "r1".to_string(),
                seed: 1,
                trace_path: Some(trace_path.to_string_lossy().to_string()),
                report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: None,
            findings: Vec::new(),
        },
        checksum: None,
    };
    trace.write_json(&trace_path).expect("write trace");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: Reporter::Json,
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

    let err = load_summary(&cfg, "r1").expect_err("must reject trace-only wrapper");
    assert!(
        err.to_string()
            .contains("no coherent report/manifest pair found")
            || err
                .to_string()
                .contains("missing required files: report.json, manifest.json")
            || err.to_string().contains("no report found")
    );
}

#[test]
fn load_summary_rejects_incoherent_manifest_only_run_wrapper() {
    let root = std::env::temp_dir().join(format!("fozzy-report-manifest-only-{}", Uuid::new_v4()));
    let run_dir = root.join(".fozzy").join("runs").join("r1");
    std::fs::create_dir_all(&run_dir).expect("mkdir");
    let trace_path = run_dir.join("trace.fozzy");

    let trace = TraceFile {
        format: crate::TRACE_FORMAT.to_string(),
        version: crate::CURRENT_TRACE_VERSION,
        engine: crate::version_info(),
        mode: RunMode::Run,
        scenario_path: None,
        scenario: Some(crate::ScenarioV1Steps {
            version: 1,
            name: "x".to_string(),
            steps: Vec::new(),
        }),
        fuzz: None,
        explore: None,
        memory: None,
        decisions: Vec::new(),
        events: Vec::new(),
        summary: RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "r1".to_string(),
                seed: 1,
                trace_path: Some(trace_path.to_string_lossy().to_string()),
                report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
                artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: None,
            findings: Vec::new(),
        },
        checksum: None,
    };
    trace.write_json(&trace_path).expect("write trace");
    let manifest = crate::RunManifest {
        schema_version: "fozzy.run_manifest.v1".to_string(),
        run_id: "other".to_string(),
        mode: RunMode::Run,
        status: ExitStatus::Pass,
        seed: 1,
        started_at: "2026-01-01T00:00:00Z".to_string(),
        finished_at: "2026-01-01T00:00:00Z".to_string(),
        duration_ms: 0,
        duration_ns: 0,
        trace_path: Some(trace_path.to_string_lossy().to_string()),
        report_path: Some(run_dir.join("report.json").to_string_lossy().to_string()),
        artifacts_dir: Some(run_dir.to_string_lossy().to_string()),
        findings_count: 0,
        tests_passed: None,
        tests_failed: None,
        tests_skipped: None,
        memory_leaked_bytes: None,
        memory_leaked_allocs: None,
        memory_peak_bytes: None,
        profile_capabilities: Vec::new(),
        profile_artifacts: std::collections::BTreeMap::new(),
        profile_schema_versions: std::collections::BTreeMap::new(),
    };
    std::fs::write(
        run_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("manifest json"),
    )
    .expect("write manifest");

    let cfg = crate::Config {
        base_dir: root.join(".fozzy"),
        reporter: Reporter::Json,
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

    let err = load_summary(&cfg, "r1").expect_err("must reject incoherent manifest-only wrapper");
    assert!(err.to_string().contains("manifest/trace identity mismatch"));
}
