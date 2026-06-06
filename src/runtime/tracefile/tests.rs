    use super::*;
    use crate::{ExitStatus, RunIdentity, RunSummary};
    use uuid::Uuid;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fozzy-trace-tests-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir.join(name)
    }

    fn sample_summary(trace_path: Option<String>) -> RunSummary {
        RunSummary {
            status: ExitStatus::Pass,
            mode: RunMode::Run,
            identity: RunIdentity {
                run_id: "run-1".to_string(),
                seed: 1,
                trace_path,
                report_path: None,
                artifacts_dir: None,
            },
            started_at: "2026-01-01T00:00:00Z".to_string(),
            finished_at: "2026-01-01T00:00:00Z".to_string(),
            duration_ms: 0,
            duration_ns: 0,
            tests: None,
            memory: None,
            findings: Vec::new(),
        }
    }

    #[test]
    fn trace_parses_legacy_scheduler_and_step_decisions() {
        let raw = r#"{
          "format":"fozzy-trace",
          "version":1,
          "engine":{"version":"0.1.0"},
          "mode":"run",
          "scenario_path":"tests/example.fozzy.json",
          "scenario":{"version":1,"name":"example","steps":[]},
          "decisions":[
            {"kind":"scheduler_pick","task_id":1,"label":"step0"},
            {"kind":"step","index":0,"name":"legacy-step"}
          ],
          "events":[],
          "summary":{
            "status":"pass",
            "mode":"run",
            "identity":{"runId":"r1","seed":1},
            "startedAt":"2026-01-01T00:00:00Z",
            "finishedAt":"2026-01-01T00:00:00Z",
            "durationMs":0
          }
        }"#;

        let trace: TraceFile = serde_json::from_str(raw).expect("legacy trace parses");
        assert_eq!(trace.version, 1);
        assert_eq!(trace.decisions.len(), 2);
    }

    #[test]
    fn trace_parses_network_replay_decisions() {
        let raw = r#"{
          "format":"fozzy-trace",
          "version":1,
          "engine":{"version":"0.1.0"},
          "mode":"run",
          "scenario_path":"tests/net.fozzy.json",
          "scenario":{"version":1,"name":"net","steps":[]},
          "decisions":[
            {"kind":"scheduler_pick","task_id":1,"label":"NetDeliverOne"},
            {"kind":"net_deliver_pick","message_id":42},
            {"kind":"net_drop","message_id":42,"dropped":false}
          ],
          "events":[],
          "summary":{
            "status":"pass",
            "mode":"run",
            "identity":{"runId":"r2","seed":2},
            "startedAt":"2026-01-01T00:00:00Z",
            "finishedAt":"2026-01-01T00:00:00Z",
            "durationMs":0
          }
        }"#;

        let trace: TraceFile = serde_json::from_str(raw).expect("network trace parses");
        assert_eq!(trace.decisions.len(), 3);
        let out = serde_json::to_string(&trace).expect("trace serializes");
        assert!(out.contains("net_deliver_pick"));
        assert!(out.contains("net_drop"));
    }

    #[test]
    fn checksum_mismatch_is_rejected() {
        let path = temp_file("bad.fozzy");
        let raw = r#"{
          "format":"fozzy-trace",
          "version":2,
          "engine":{"version":"0.1.0"},
          "mode":"run",
          "scenario_path":null,
          "scenario":{"version":1,"name":"x","steps":[]},
          "decisions":[],
          "events":[],
          "summary":{
            "status":"pass",
            "mode":"run",
            "identity":{"runId":"r1","seed":1},
            "startedAt":"2026-01-01T00:00:00Z",
            "finishedAt":"2026-01-01T00:00:00Z",
            "durationMs":0
          },
          "checksum":"deadbeef"
        }"#;
        std::fs::write(&path, raw).expect("write");
        let err = TraceFile::read_json(&path).expect_err("must reject checksum mismatch");
        assert!(err.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn record_collision_error_policy_rejects_existing_target() {
        let path = temp_file("exists.fozzy");
        std::fs::write(&path, b"old").expect("write existing");
        let trace = TraceFile::new(
            RunMode::Run,
            None,
            Some(ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            Vec::new(),
            Vec::new(),
            sample_summary(Some(path.to_string_lossy().to_string())),
        );
        let err = write_trace_with_policy(&trace, &path, RecordCollisionPolicy::Error)
            .expect_err("must fail");
        assert!(err.to_string().contains("record collision"));
    }

    #[test]
    fn record_collision_append_policy_picks_numbered_path() {
        let path = temp_file("trace.fozzy");
        std::fs::write(&path, b"old").expect("write existing");
        let trace = TraceFile::new(
            RunMode::Run,
            None,
            Some(ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            Vec::new(),
            Vec::new(),
            sample_summary(None),
        );
        let out =
            write_trace_with_policy(&trace, &path, RecordCollisionPolicy::Append).expect("append");
        assert_ne!(out, path);
        assert!(out.to_string_lossy().contains(".1.fozzy"));
        let loaded = TraceFile::read_json(&out).expect("trace exists");
        assert_eq!(loaded.format, "fozzy-trace");
    }

    #[test]
    fn truncated_trace_is_rejected() {
        let path = temp_file("truncated.fozzy");
        std::fs::write(&path, br#"{"format":"fozzy-trace""#).expect("write");
        let err = TraceFile::read_json(&path).expect_err("must fail");
        assert!(err.to_string().contains("failed to parse trace"));
    }

    #[test]
    fn random_bytes_trace_is_rejected() {
        let path = temp_file("random.fozzy");
        std::fs::write(&path, [0_u8, 159, 146, 150, 255, 0, 1, 2]).expect("write");
        let err = TraceFile::read_json(&path).expect_err("must fail");
        assert!(err.to_string().contains("failed to parse trace"));
    }

    #[test]
    fn unsupported_trace_format_is_rejected() {
        let path = temp_file("bad-format.fozzy");
        let raw = r#"{
          "format":"fozzy-trace-vX",
          "version":2,
          "engine":{"version":"0.1.0"},
          "mode":"run",
          "scenario_path":null,
          "scenario":{"version":1,"name":"x","steps":[]},
          "decisions":[],
          "events":[],
          "summary":{
            "status":"pass",
            "mode":"run",
            "identity":{"runId":"r1","seed":1},
            "startedAt":"2026-01-01T00:00:00Z",
            "finishedAt":"2026-01-01T00:00:00Z",
            "durationMs":0
          }
        }"#;
        std::fs::write(&path, raw).expect("write");
        let err = TraceFile::read_json(&path).expect_err("must reject unsupported format");
        assert!(err.to_string().contains("unsupported trace format"));
    }

    #[test]
    fn unsupported_trace_version_is_rejected() {
        let path = temp_file("bad-version.fozzy");
        let raw = r#"{
          "format":"fozzy-trace",
          "version":999,
          "engine":{"version":"0.1.0"},
          "mode":"run",
          "scenario_path":null,
          "scenario":{"version":1,"name":"x","steps":[]},
          "decisions":[],
          "events":[],
          "summary":{
            "status":"pass",
            "mode":"run",
            "identity":{"runId":"r1","seed":1},
            "startedAt":"2026-01-01T00:00:00Z",
            "finishedAt":"2026-01-01T00:00:00Z",
            "durationMs":0
          }
        }"#;
        std::fs::write(&path, raw).expect("write");
        let err = TraceFile::read_json(&path).expect_err("must reject unsupported version");
        assert!(err.to_string().contains("unsupported trace schema version"));
    }

    #[test]
    fn verify_warns_on_legacy_host_proc_trace_without_proc_decisions() {
        let path = temp_file("legacy-host-proc.fozzy");
        let trace = TraceFile::new(
            RunMode::Run,
            None,
            Some(ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            Vec::new(),
            vec![TraceEvent {
                time_ms: 0,
                name: "proc_spawn".to_string(),
                fields: serde_json::Map::from_iter([(
                    "backend".to_string(),
                    serde_json::Value::String("host".to_string()),
                )]),
            }],
            sample_summary(None),
        );
        trace.write_json(&path).expect("write");

        let verify = verify_trace_file(&path).expect("verify");
        assert!(
            verify
                .warnings
                .iter()
                .any(|w| w.contains("host proc backend") && w.contains("replay may drift"))
        );
    }

    #[test]
    fn verify_warns_on_legacy_host_fs_trace_without_fs_decisions() {
        let path = temp_file("legacy-host-fs.fozzy");
        let trace = TraceFile::new(
            RunMode::Run,
            None,
            Some(ScenarioV1Steps {
                version: 1,
                name: "x".to_string(),
                steps: Vec::new(),
            }),
            Vec::new(),
            vec![TraceEvent {
                time_ms: 0,
                name: "capability_fs".to_string(),
                fields: serde_json::Map::from_iter([(
                    "backend".to_string(),
                    serde_json::Value::String("host".to_string()),
                )]),
            }],
            sample_summary(None),
        );
        trace.write_json(&path).expect("write");

        let verify = verify_trace_file(&path).expect("verify");
        assert!(
            verify
                .warnings
                .iter()
                .any(|w| w.contains("host fs backend") && w.contains("replay may drift"))
        );
    }
