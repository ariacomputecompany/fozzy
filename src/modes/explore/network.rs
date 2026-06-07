use crate::{DistributedStep, FozzyError, FozzyResult, TraceEvent};

use super::types::{Message, MessageQueue, NetRules, NodeMap};
use super::utils::bump;

pub(super) fn apply_script_step(
    step: &DistributedStep,
    nodes: &mut NodeMap,
    net: &mut NetRules,
    queue: &mut MessageQueue,
    next_id: &mut u64,
    events: &mut Vec<TraceEvent>,
    time_ms: &mut u64,
) -> FozzyResult<()> {
    match step {
        DistributedStep::ClientPut { node, key, value } => {
            let Some(n) = nodes.get_mut(node) else {
                return Err(FozzyError::Scenario(format!("unknown node {node:?}")));
            };
            if !n.running {
                return Ok(());
            }
            let version = n
                .kv_version
                .get(key)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            n.kv_version.insert(key.clone(), version);
            n.kv.insert(key.clone(), value.clone());
            for to in nodes.keys().cloned().collect::<Vec<_>>() {
                if to == *node {
                    continue;
                }
                queue.push_back(Message {
                    id: bump(next_id),
                    from: node.clone(),
                    to,
                    kind: "kv_repl".to_string(),
                    key: key.clone(),
                    value: value.clone(),
                    version,
                });
            }
            events.push(TraceEvent {
                time_ms: *time_ms,
                name: "client_put".to_string(),
                fields: serde_json::Map::from_iter([
                    ("node".to_string(), serde_json::Value::String(node.clone())),
                    ("key".to_string(), serde_json::Value::String(key.clone())),
                ]),
            });
            Ok(())
        }
        DistributedStep::ClientGetAssert {
            node,
            key,
            equals,
            is_null,
        } => {
            let Some(n) = nodes.get(node) else {
                return Err(FozzyError::Scenario(format!("unknown node {node:?}")));
            };
            if !n.running {
                return Ok(());
            }
            let got = n.kv.get(key).cloned();
            if is_null.unwrap_or(false) {
                if got.is_some() {
                    return Err(FozzyError::Scenario(format!(
                        "expected {node}.{key} to be null"
                    )));
                }
                return Ok(());
            }
            if let Some(expected) = equals {
                if got.as_deref() != Some(expected.as_str()) {
                    return Err(FozzyError::Scenario(format!(
                        "expected {node}.{key} == {expected:?}, got {got:?}"
                    )));
                }
            } else if got.is_none() {
                return Err(FozzyError::Scenario(format!(
                    "expected {node}.{key} to exist"
                )));
            }
            Ok(())
        }
        DistributedStep::Partition { a, b } => {
            net.partition(a, b);
            events.push(TraceEvent {
                time_ms: *time_ms,
                name: "partition".to_string(),
                fields: serde_json::Map::from_iter([
                    ("a".to_string(), serde_json::Value::String(a.clone())),
                    ("b".to_string(), serde_json::Value::String(b.clone())),
                ]),
            });
            Ok(())
        }
        DistributedStep::Heal { a, b } => {
            net.heal(a, b);
            events.push(TraceEvent {
                time_ms: *time_ms,
                name: "heal".to_string(),
                fields: serde_json::Map::from_iter([
                    ("a".to_string(), serde_json::Value::String(a.clone())),
                    ("b".to_string(), serde_json::Value::String(b.clone())),
                ]),
            });
            Ok(())
        }
        DistributedStep::Crash { node } => {
            if let Some(n) = nodes.get_mut(node) {
                n.running = false;
            }
            events.push(TraceEvent {
                time_ms: *time_ms,
                name: "crash".to_string(),
                fields: serde_json::Map::from_iter([(
                    "node".to_string(),
                    serde_json::Value::String(node.clone()),
                )]),
            });
            Ok(())
        }
        DistributedStep::Restart { node } => {
            if let Some(n) = nodes.get_mut(node) {
                n.running = true;
            }
            events.push(TraceEvent {
                time_ms: *time_ms,
                name: "restart".to_string(),
                fields: serde_json::Map::from_iter([(
                    "node".to_string(),
                    serde_json::Value::String(node.clone()),
                )]),
            });
            Ok(())
        }
        DistributedStep::Tick { duration: _ } => Ok(()),
    }
}

pub(super) fn deliver_message(
    msg: Message,
    nodes: &mut NodeMap,
    queue: &mut MessageQueue,
    next_id: &mut u64,
) -> FozzyResult<()> {
    let Some(to) = nodes.get_mut(&msg.to) else {
        return Ok(());
    };
    if !to.running {
        return Ok(());
    }
    if msg.kind == "kv_repl" {
        let current = to.kv_version.get(&msg.key).copied().unwrap_or(0);
        if msg.version >= current {
            to.kv_version.insert(msg.key.clone(), msg.version);
            to.kv.insert(msg.key.clone(), msg.value.clone());
        }
    } else if msg.kind == "kv_forward" {
        for peer in nodes.keys().cloned().collect::<Vec<_>>() {
            if peer == msg.to {
                continue;
            }
            queue.push_back(Message {
                id: bump(next_id),
                from: msg.to.clone(),
                to: peer,
                kind: "kv_repl".to_string(),
                key: msg.key.clone(),
                value: msg.value.clone(),
                version: msg.version,
            });
        }
    }
    Ok(())
}

pub(super) fn deliverable_indices(
    queue: &MessageQueue,
    nodes: &NodeMap,
    net: &NetRules,
) -> Vec<usize> {
    let mut out = Vec::new();
    for (idx, m) in queue.iter().enumerate() {
        let Some(from) = nodes.get(&m.from) else {
            continue;
        };
        let Some(to) = nodes.get(&m.to) else {
            continue;
        };
        if !from.running || !to.running {
            continue;
        }
        if net.is_blocked(&m.from, &m.to) {
            continue;
        }
        out.push(idx);
    }
    out
}
