use crate::{DistributedInvariant, Finding, FindingKind};

use super::types::{InvariantPhase, NodeMap, ScenarioV1Explore};

pub(super) fn check_invariants(
    scenario: &ScenarioV1Explore,
    nodes: &NodeMap,
    phase: InvariantPhase,
) -> Option<Finding> {
    for inv in &scenario.invariants {
        match inv {
            DistributedInvariant::KvAllEqual { key } => {
                if phase == InvariantPhase::Progress {
                    continue;
                }
                let mut expected: Option<String> = None;
                for n in nodes.values() {
                    if !n.running {
                        continue;
                    }
                    let v = n.kv.get(key).cloned();
                    if expected.is_none() {
                        expected = v;
                        continue;
                    }
                    if v != expected {
                        return Some(Finding {
                            kind: FindingKind::Invariant,
                            title: "kv_all_equal".to_string(),
                            message: format!(
                                "invariant violated for key {key:?}: values diverged across nodes"
                            ),
                            location: None,
                        });
                    }
                }
            }
            DistributedInvariant::KvPresentOnAll { key } => {
                for (name, n) in nodes {
                    if !n.running {
                        continue;
                    }
                    if !n.kv.contains_key(key) {
                        return Some(Finding {
                            kind: FindingKind::Invariant,
                            title: "kv_present_on_all".to_string(),
                            message: format!(
                                "invariant violated: key {key:?} missing on node {name:?}"
                            ),
                            location: None,
                        });
                    }
                }
            }
            DistributedInvariant::KvNodeEquals { node, key, equals } => {
                let Some(n) = nodes.get(node) else {
                    return Some(Finding {
                        kind: FindingKind::Invariant,
                        title: "kv_node_equals".to_string(),
                        message: format!("invariant references unknown node {node:?}"),
                        location: None,
                    });
                };
                if !n.running {
                    continue;
                }
                if n.kv.get(key).map(String::as_str) != Some(equals.as_str()) {
                    return Some(Finding {
                        kind: FindingKind::Invariant,
                        title: "kv_node_equals".to_string(),
                        message: format!(
                            "invariant violated: expected {node}.{key} == {equals:?}, got {:?}",
                            n.kv.get(key)
                        ),
                        location: None,
                    });
                }
            }
        }
    }
    None
}
