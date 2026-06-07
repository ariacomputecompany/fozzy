use std::path::Path;

pub(super) fn valid_manifest_json(run_id: &str) -> String {
    format!(
        r#"{{"schemaVersion":"fozzy.run_manifest.v1","runId":"{run_id}","mode":"run","status":"pass","seed":1,"startedAt":"2026-01-01T00:00:00Z","finishedAt":"2026-01-01T00:00:00Z","durationMs":0,"findingsCount":0}}"#
    )
}

pub(super) fn valid_report_json(run_id: &str, report_path: &Path, artifacts_dir: &Path) -> String {
    format!(
        r#"{{
  "status":"pass",
  "mode":"run",
  "identity":{{
    "runId":"{run_id}",
    "seed":1,
    "reportPath":"{}",
    "artifactsDir":"{}"
  }},
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "findings":[]
}}"#,
        report_path.display(),
        artifacts_dir.display()
    )
}

pub(super) fn valid_report_and_manifest_json(
    run_id: &str,
    report_path: &Path,
    artifacts_dir: &Path,
    trace_path: Option<&Path>,
) -> (String, String) {
    let trace_json = trace_path.map(|path| format!(r#","tracePath":"{}""#, path.display()));
    let report = format!(
        r#"{{
  "status":"pass",
  "mode":"run",
  "identity":{{
    "runId":"{run_id}",
    "seed":1,
    "reportPath":"{}",
    "artifactsDir":"{}"{}
  }},
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "findings":[]
}}"#,
        report_path.display(),
        artifacts_dir.display(),
        trace_json.clone().unwrap_or_default()
    );
    let manifest = format!(
        r#"{{
  "schemaVersion":"fozzy.run_manifest.v1",
  "runId":"{run_id}",
  "mode":"run",
  "status":"pass",
  "seed":1,
  "startedAt":"2026-01-01T00:00:00Z",
  "finishedAt":"2026-01-01T00:00:00Z",
  "durationMs":0,
  "durationNs":0,
  "reportPath":"{}",
  "artifactsDir":"{}",
  "findingsCount":0{}
}}"#,
        report_path.display(),
        artifacts_dir.display(),
        trace_path
            .map(|path| format!(r#","tracePath":"{}""#, path.display()))
            .unwrap_or_default()
    );
    (report, manifest)
}

pub(super) fn valid_trace_json(
    run_id: &str,
    trace_path: &Path,
    report_path: &Path,
    artifacts_dir: &Path,
) -> String {
    format!(
        r#"{{
  "format":"fozzy-trace",
  "version":4,
  "engine":{{"version":"0.1.0"}},
  "mode":"run",
  "scenario_path":null,
  "scenario":{{"version":1,"name":"x","steps":[]}},
  "decisions":[],
  "events":[],
  "summary":{{
    "status":"pass",
    "mode":"run",
    "identity":{{
      "runId":"{run_id}",
      "seed":1,
      "tracePath":"{}",
      "reportPath":"{}",
      "artifactsDir":"{}"
    }},
    "startedAt":"2026-01-01T00:00:00Z",
    "finishedAt":"2026-01-01T00:00:00Z",
    "durationMs":0,
    "durationNs":0
  }}
}}"#,
        trace_path.display(),
        report_path.display(),
        artifacts_dir.display()
    )
}

pub(super) fn valid_profile_metrics_json(run_id: &str) -> String {
    format!(
        r#"{{
  "schemaVersion":"fozzy.profile_metrics.v1",
  "runId":"{run_id}",
  "timeDomains":{{
    "virtualTime":"virtual_time",
    "hostMonotonicTime":"host_monotonic_time"
  }},
  "virtualTimeMs":0,
  "hostTimeMs":0,
  "cpuTimeMs":0,
  "allocBytes":0,
  "inUseBytes":0,
  "p50LatencyMs":0,
  "p95LatencyMs":0,
  "p99LatencyMs":0,
  "maxLatencyMs":0,
  "ioOps":0,
  "schedOps":0
}}"#
    )
}

pub(super) fn valid_profile_timeline_json(run_id: &str, seed: u64) -> String {
    format!(
        r#"{{
  "schemaVersion":"fozzy.profile_timeline.v1",
  "runId":"{run_id}",
  "timeDomains":{{
    "virtualTime":"virtual_time",
    "hostMonotonicTime":"host_monotonic_time"
  }},
  "events":[{{
    "t_virtual":0,
    "kind":"event",
    "run_id":"{run_id}",
    "seed":{seed},
    "thread":"main",
    "span_id":"root",
    "tags":{{}},
    "cost":{{}}
  }}]
}}"#
    )
}

pub(super) fn valid_memory_leaks_json(bytes: u64) -> String {
    format!(r#"[{{"allocId":1,"bytes":{bytes},"callsiteHash":"callsite-1"}}]"#)
}

pub(super) fn valid_memory_graph_json() -> &'static str {
    r#"{
  "nodes":[
    {"id":"alloc:1","kind":"alloc","label":"1"},
    {"id":"free:1","kind":"free","label":"1"},
    {"id":"callsite:callsite-1","kind":"callsite","label":"callsite-1"}
  ],
  "edges":[
    {"from":"callsite:callsite-1","to":"alloc:1","kind":"allocates"},
    {"from":"alloc:1","to":"free:1","kind":"freed_by"}
  ]
}"#
}

pub(super) fn valid_memory_timeline_json() -> &'static str {
    r#"[
  {"index":0,"timeMs":0,"kind":"alloc","fields":{"allocId":1,"bytes":128}},
  {"index":1,"timeMs":1,"kind":"free","fields":{"allocId":1,"bytes":128}}
]"#
}

pub(super) fn valid_memory_delta_json(
    after_leaked_bytes: u64,
    after_leaked_allocs: u64,
    after_alloc_count: u64,
) -> String {
    format!(
        r#"{{
  "schemaVersion":"fozzy.memory_delta.v1",
  "beforeLeakedBytes":0,
  "afterLeakedBytes":{after_leaked_bytes},
  "beforeLeakedAllocs":0,
  "afterLeakedAllocs":{after_leaked_allocs},
  "beforeAllocCount":0,
  "afterAllocCount":{after_alloc_count}
}}"#
    )
}

pub(super) fn valid_events_json(name: &str) -> String {
    format!(r#"[{{"time_ms":0,"name":"{name}","fields":{{}}}}]"#)
}

pub(super) fn valid_timeline_json(name: &str) -> String {
    format!(r#"[{{"index":0,"time_ms":0,"name":"{name}","fields":{{}}}}]"#)
}
