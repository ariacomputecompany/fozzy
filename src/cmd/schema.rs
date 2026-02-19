//! Scenario/schema introspection for automation and authoring.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SchemaDoc {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "fileVariants")]
    pub file_variants: Vec<FileVariant>,
    #[serde(rename = "stepTypes")]
    pub step_types: Vec<&'static str>,
    #[serde(rename = "distributedStepTypes")]
    pub distributed_step_types: Vec<&'static str>,
    #[serde(rename = "distributedInvariantTypes")]
    pub distributed_invariant_types: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileVariant {
    pub name: &'static str,
    #[serde(rename = "requiredTopLevelKeys")]
    pub required_top_level_keys: Vec<&'static str>,
    #[serde(rename = "minimalExample")]
    pub minimal_example: serde_json::Value,
}

pub fn schema_doc() -> SchemaDoc {
    SchemaDoc {
        schema_version: "fozzy.schema_doc.v1".to_string(),
        file_variants: vec![
            FileVariant {
                name: "steps",
                required_top_level_keys: vec!["version", "name", "steps"],
                minimal_example: serde_json::json!({
                    "version": 1,
                    "name": "example",
                    "steps": [
                        { "type": "trace_event", "name": "setup" },
                        { "type": "assert_eq_int", "a": 1, "b": 1 }
                    ]
                }),
            },
            FileVariant {
                name: "distributed",
                required_top_level_keys: vec!["version", "name", "distributed"],
                minimal_example: serde_json::json!({
                    "version": 1,
                    "name": "distributed-example",
                    "distributed": {
                        "node_count": 3,
                        "steps": [
                            { "type": "client_put", "node": "n0", "key": "k", "value": "v" },
                            { "type": "tick", "duration": "10ms" }
                        ],
                        "invariants": [
                            { "type": "kv_present_on_all", "key": "k" }
                        ]
                    }
                }),
            },
            FileVariant {
                name: "suites",
                required_top_level_keys: vec!["version", "name", "suites"],
                minimal_example: serde_json::json!({
                    "version": 1,
                    "name": "suites-placeholder",
                    "suites": {}
                }),
            },
        ],
        step_types: vec![
            "trace_event",
            "rand_u64",
            "assert_ok",
            "assert_eq_int",
            "assert_ne_int",
            "assert_eq_str",
            "assert_ne_str",
            "sleep",
            "advance",
            "freeze",
            "unfreeze",
            "set_kv",
            "get_kv_assert",
            "fs_write",
            "fs_read_assert",
            "fs_snapshot",
            "fs_restore",
            "http_when",
            "http_request",
            "proc_when",
            "proc_spawn",
            "net_partition",
            "net_heal",
            "net_set_drop_rate",
            "net_set_reorder",
            "net_send",
            "net_deliver_one",
            "net_recv_assert",
            "memory_alloc",
            "memory_free",
            "memory_limit_mb",
            "memory_fail_after_allocs",
            "memory_fragmentation",
            "memory_pressure_wave",
            "memory_checkpoint",
            "memory_assert_in_use_bytes",
            "assert_throws",
            "assert_rejects",
            "assert_eventually_kv",
            "assert_never_kv",
            "fail",
            "panic",
        ],
        distributed_step_types: vec![
            "client_put",
            "client_get_assert",
            "partition",
            "heal",
            "crash",
            "restart",
            "tick",
        ],
        distributed_invariant_types: vec!["kv_all_equal", "kv_present_on_all", "kv_node_equals"],
    }
}
