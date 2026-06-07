use crate::cli_workflows::*;
use crate::FullStepStatus;

#[test]
fn memory_graph_status_skips_empty_graph() {
    let value = serde_json::json!({"graph": {"nodes": [], "edges": []}});
    let (status, detail) = memory_graph_status(&value);
    assert!(matches!(status, FullStepStatus::Skipped));
    assert!(detail.contains("nodes=0"));
    assert!(detail.contains("edges=0"));
}

#[test]
fn memory_graph_status_rejects_invalid_edge_references() {
    let value = serde_json::json!({
        "graph": {
            "nodes": [{"id": "alloc:1", "kind": "alloc", "label": "a"}],
            "edges": [{"from": "alloc:1", "to": "alloc:2", "kind": "owns"}]
        }
    });
    let (status, detail) = memory_graph_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_edges=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn memory_graph_status_rejects_invalid_node_ids() {
    let value = serde_json::json!({
        "graph": {
            "nodes": [
                {"id": "", "kind": "alloc", "label": "blank"},
                {"kind": "alloc", "label": "missing"}
            ],
            "edges": []
        }
    });
    let (status, detail) = memory_graph_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_nodes=2"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn memory_graph_status_rejects_blank_edge_kind() {
    let value = serde_json::json!({
        "graph": {
            "nodes": [
                {"id": "alloc:1", "kind": "alloc", "label": "a"},
                {"id": "alloc:2", "kind": "alloc", "label": "b"}
            ],
            "edges": [{"from": "alloc:1", "to": "alloc:2", "kind": ""}]
        }
    });
    let (status, detail) = memory_graph_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid_edges=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn memory_graph_status_rejects_duplicate_node_ids() {
    let value = serde_json::json!({
        "graph": {
            "nodes": [
                {"id": "alloc:1", "kind": "alloc", "label": "a"},
                {"id": "alloc:1", "kind": "alloc", "label": "a-dup"}
            ],
            "edges": []
        }
    });
    let (status, detail) = memory_graph_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_nodes=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn memory_graph_status_rejects_duplicate_edges() {
    let value = serde_json::json!({
        "graph": {
            "nodes": [
                {"id": "alloc:1", "kind": "alloc", "label": "a"},
                {"id": "alloc:2", "kind": "alloc", "label": "b"}
            ],
            "edges": [
                {"from": "alloc:1", "to": "alloc:2", "kind": "owns"},
                {"from": "alloc:1", "to": "alloc:2", "kind": "owns"}
            ]
        }
    });
    let (status, detail) = memory_graph_status(&value);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("duplicate_edges=1"));
    assert!(detail.contains("consistent=false"));
}

#[test]
fn artifacts_list_status_rejects_empty_entries() {
    let output = fozzy::ArtifactOutput::List {
        entries: Vec::new(),
    };
    let path = PathBuf::from("/tmp/example.trace.fozzy");
    let (status, detail) = artifacts_list_status(&output, &path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("entries=0"));
}

#[test]
fn artifacts_list_status_rejects_missing_entry_file() {
    let output = fozzy::ArtifactOutput::List {
        entries: vec![fozzy::ArtifactEntry {
            kind: fozzy::ArtifactKind::Trace,
            path: "/tmp/definitely-missing-fozzy-artifact".to_string(),
            size_bytes: Some(10),
        }],
    };
    let path = PathBuf::from("/tmp/example.trace.fozzy");
    let (status, detail) = artifacts_list_status(&output, &path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid="));
    assert!(detail.contains("definitely-missing-fozzy-artifact"));
}

#[test]
fn artifacts_list_status_rejects_duplicate_entry_paths() {
    let dir =
        std::env::temp_dir().join(format!("fozzy-artifacts-list-dup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create artifact dir");
    let artifact = dir.join("trace.fozzy");
    std::fs::write(&artifact, b"trace").expect("write artifact");
    let path_str = artifact.to_string_lossy().to_string();
    let output = fozzy::ArtifactOutput::List {
        entries: vec![
            fozzy::ArtifactEntry {
                kind: fozzy::ArtifactKind::Trace,
                path: path_str.clone(),
                size_bytes: Some(5),
            },
            fozzy::ArtifactEntry {
                kind: fozzy::ArtifactKind::Trace,
                path: path_str,
                size_bytes: Some(5),
            },
        ],
    };
    let path = PathBuf::from("/tmp/example.trace.fozzy");
    let (status, detail) = artifacts_list_status(&output, &path);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("entries=2"));
    assert!(detail.contains("duplicate artifact path"));
}

#[test]
fn artifacts_list_status_rejects_blank_entry_path() {
    let output = fozzy::ArtifactOutput::List {
        entries: vec![fozzy::ArtifactEntry {
            kind: fozzy::ArtifactKind::Trace,
            path: "   ".to_string(),
            size_bytes: Some(5),
        }],
    };
    let path = PathBuf::from("/tmp/example.trace.fozzy");
    let (status, detail) = artifacts_list_status(&output, &path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("entries=1"));
    assert!(detail.contains("blank artifact path"));
}

#[test]
fn artifacts_diff_status_rejects_inconsistent_file_delta() {
    let output = fozzy::ArtifactOutput::Diff {
        diff: Box::new(fozzy::ArtifactDiff {
            left: "left".to_string(),
            right: "right".to_string(),
            files: vec![fozzy::ArtifactFileDelta {
                key: "Trace:trace.fozzy".to_string(),
                left_path: Some("/tmp/left.trace.fozzy".to_string()),
                right_path: Some("/tmp/right.trace.fozzy".to_string()),
                left_size_bytes: Some(10),
                right_size_bytes: Some(11),
                changed: false,
            }],
            report: None,
            trace: None,
        }),
    };
    let (status, detail) = artifacts_diff_status(&output);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid=1"));
}

#[test]
fn artifacts_diff_status_allows_same_size_changed_file_delta() {
    let output = fozzy::ArtifactOutput::Diff {
        diff: Box::new(fozzy::ArtifactDiff {
            left: "left".to_string(),
            right: "right".to_string(),
            files: vec![fozzy::ArtifactFileDelta {
                key: "Trace:trace.fozzy".to_string(),
                left_path: Some("/tmp/left.trace.fozzy".to_string()),
                right_path: Some("/tmp/right.trace.fozzy".to_string()),
                left_size_bytes: Some(10),
                right_size_bytes: Some(10),
                changed: true,
            }],
            report: None,
            trace: None,
        }),
    };
    let (status, detail) = artifacts_diff_status(&output);
    assert!(matches!(status, FullStepStatus::Passed));
    assert!(detail.contains("invalid=0"));
}

#[test]
fn artifacts_diff_status_rejects_duplicate_file_delta_keys() {
    let output = fozzy::ArtifactOutput::Diff {
        diff: Box::new(fozzy::ArtifactDiff {
            left: "left".to_string(),
            right: "right".to_string(),
            files: vec![
                fozzy::ArtifactFileDelta {
                    key: "Trace:trace.fozzy".to_string(),
                    left_path: Some("/tmp/left.trace.fozzy".to_string()),
                    right_path: Some("/tmp/right.trace.fozzy".to_string()),
                    left_size_bytes: Some(10),
                    right_size_bytes: Some(11),
                    changed: true,
                },
                fozzy::ArtifactFileDelta {
                    key: "Trace:trace.fozzy".to_string(),
                    left_path: Some("/tmp/left.trace.fozzy".to_string()),
                    right_path: Some("/tmp/right.trace.fozzy".to_string()),
                    left_size_bytes: Some(10),
                    right_size_bytes: Some(11),
                    changed: true,
                },
            ],
            report: None,
            trace: None,
        }),
    };
    let (status, detail) = artifacts_diff_status(&output);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("invalid=1"));
}

#[test]
fn artifacts_diff_status_rejects_blank_diff_identities() {
    let output = fozzy::ArtifactOutput::Diff {
        diff: Box::new(fozzy::ArtifactDiff {
            left: "   ".to_string(),
            right: "".to_string(),
            files: vec![fozzy::ArtifactFileDelta {
                key: "Trace:trace.fozzy".to_string(),
                left_path: Some("/tmp/left.trace.fozzy".to_string()),
                right_path: Some("/tmp/right.trace.fozzy".to_string()),
                left_size_bytes: Some(10),
                right_size_bytes: Some(11),
                changed: true,
            }],
            report: None,
            trace: None,
        }),
    };
    let (status, detail) = artifacts_diff_status(&output);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("left_ok=false"));
    assert!(detail.contains("right_ok=false"));
}

#[test]
fn env_step_status_rejects_unknown_backends() {
    let env = fozzy::EnvInfo {
        os: "macos".to_string(),
        arch: "aarch64".to_string(),
        fozzy: fozzy::version_info(),
        capabilities: std::collections::BTreeMap::new(),
    };
    let (status, detail) = env_step_status(&env);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("proc=unknown"));
    assert!(detail.contains("known_proc=false"));
}

#[test]
fn env_step_status_rejects_invalid_backend_names() {
    let mut capabilities = std::collections::BTreeMap::new();
    capabilities.insert(
        "proc".to_string(),
        fozzy::CapabilityInfo {
            backend: "sandboxed".to_string(),
            deterministic: true,
        },
    );
    capabilities.insert(
        "fs".to_string(),
        fozzy::CapabilityInfo {
            backend: "overlay".to_string(),
            deterministic: true,
        },
    );
    capabilities.insert(
        "http".to_string(),
        fozzy::CapabilityInfo {
            backend: "mock".to_string(),
            deterministic: true,
        },
    );
    let env = fozzy::EnvInfo {
        os: "macos".to_string(),
        arch: "aarch64".to_string(),
        fozzy: fozzy::version_info(),
        capabilities,
    };
    let (status, detail) = env_step_status(&env);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("proc=sandboxed"));
    assert!(detail.contains("known_proc=false"));
    assert!(detail.contains("known_fs=false"));
    assert!(detail.contains("known_http=false"));
}

#[test]
fn zip_artifact_status_rejects_invalid_zip_payload() {
    let path = std::env::temp_dir().join(format!("fozzy-invalid-zip-{}.zip", uuid::Uuid::new_v4()));
    std::fs::write(&path, b"not a zip").expect("write invalid zip");
    let (status, detail) = zip_artifact_status(&path);
    let _ = std::fs::remove_file(&path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("zip_parse_error="));
}

#[test]
fn zip_artifact_status_rejects_empty_zip_archive() {
    let path = std::env::temp_dir().join(format!("fozzy-empty-zip-{}.zip", uuid::Uuid::new_v4()));
    {
        let file = std::fs::File::create(&path).expect("create empty zip");
        let zip = zip::ZipWriter::new(file);
        zip.finish().expect("finish empty zip");
    }
    let (status, detail) = zip_artifact_status(&path);
    let _ = std::fs::remove_file(&path);
    assert!(matches!(status, FullStepStatus::Failed));
    assert!(detail.contains("zip_entries=0"));
}
