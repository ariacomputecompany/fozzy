use rand_chacha::ChaCha20Rng;
use rand_core::RngCore as _;

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::{ExitStatus, Finding, FindingKind, FozzyError, FozzyResult, TraceEvent};

use super::invariants::check_invariants;
use super::network::{apply_script_step, deliver_message, deliverable_indices};
use super::types::{
    ExploreExecResult, InvariantPhase, MessageQueue, NetRules, Node, NodeMap, ScenarioV1Explore,
    ScheduleStrategy,
};
use super::utils::{rng_from_seed, stable_edge};

pub(super) fn run_explore_inner(
    scenario: &ScenarioV1Explore,
    seed: u64,
    schedule: ScheduleStrategy,
    max_steps: Option<u64>,
    max_time: Option<Duration>,
) -> FozzyResult<ExploreExecResult> {
    let mut rng = rng_from_seed(seed);
    let started = Instant::now();
    let deadline = max_time.map(|d| started + d);
    let step_budget = max_steps.unwrap_or(u64::MAX);

    let mut nodes = init_nodes(scenario);
    let mut net = NetRules::default();
    let mut queue = MessageQueue::new();
    let mut next_id = 1u64;
    let mut events = Vec::new();
    let mut findings = Vec::new();
    let mut decisions: Vec<crate::Decision> = Vec::new();
    let mut seen_strategy_edges = HashSet::new();
    let mut delivered = 0u64;
    let mut time_ms = 0u64;

    for step in &scenario.steps {
        if let crate::DistributedStep::Tick { duration } = step {
            let d = crate::parse_duration(duration)?;
            time_ms = time_ms.saturating_add(d.as_millis().min(u128::from(u64::MAX)) as u64);
        }

        apply_script_step(
            step,
            &mut nodes,
            &mut net,
            &mut queue,
            &mut next_id,
            &mut events,
            &mut time_ms,
        )?;
    }

    while delivered < step_budget {
        if let Some(dl) = deadline
            && Instant::now() >= dl
        {
            findings.push(Finding {
                kind: FindingKind::Hang,
                title: "timeout".to_string(),
                message: "explore timed out".to_string(),
                location: None,
            });
            return Ok((ExitStatus::Timeout, findings, events, delivered, decisions));
        }

        let deliverable = deliverable_indices(&queue, &nodes, &net);
        if deliverable.is_empty() {
            emit_scheduler_idle_event(&mut events, &queue, &deliverable, time_ms);
            break;
        }

        let pick = pick_index(
            &queue,
            &deliverable,
            schedule,
            &mut rng,
            &mut seen_strategy_edges,
        );
        let idx = deliverable[pick];
        let msg = queue.remove(idx).expect("index exists");
        delivered += 1;
        time_ms = time_ms.saturating_add(1);
        decisions.push(crate::Decision::SchedulerPick {
            task_id: msg.id,
            label: "deliver".to_string(),
        });
        emit_delivery_start(&mut events, &queue, &msg, time_ms);
        let msg_id = msg.id;
        deliver_message(msg, &mut nodes, &mut queue, &mut next_id)?;
        emit_delivery_end(&mut events, msg_id, time_ms);

        if let Some(finding) = check_invariants(scenario, &nodes, InvariantPhase::Progress) {
            findings.push(finding);
            return Ok((ExitStatus::Fail, findings, events, delivered, decisions));
        }
    }

    if let Some(finding) = check_invariants(scenario, &nodes, InvariantPhase::Final) {
        findings.push(finding);
        return Ok((ExitStatus::Fail, findings, events, delivered, decisions));
    }

    Ok((ExitStatus::Pass, findings, events, delivered, decisions))
}

pub(super) fn run_explore_replay_inner(
    scenario: &ScenarioV1Explore,
    seed: u64,
    schedule: ScheduleStrategy,
    decisions: &[crate::Decision],
) -> FozzyResult<ExploreExecResult> {
    let mut rng = rng_from_seed(seed);
    let mut nodes = init_nodes(scenario);
    let mut net = NetRules::default();
    let mut queue = MessageQueue::new();
    let mut next_id = 1u64;
    let mut events = Vec::new();
    let mut findings = Vec::new();
    let mut delivered = 0u64;
    let mut time_ms = 0u64;

    for step in &scenario.steps {
        if let crate::DistributedStep::Tick { duration } = step {
            let d = crate::parse_duration(duration)?;
            time_ms = time_ms.saturating_add(d.as_millis().min(u128::from(u64::MAX)) as u64);
        }
        apply_script_step(
            step,
            &mut nodes,
            &mut net,
            &mut queue,
            &mut next_id,
            &mut events,
            &mut time_ms,
        )?;
    }

    for d in decisions {
        let msg_id = replay_message_id(d)?;
        let idx = queue.iter().position(|m| m.id == msg_id).ok_or_else(|| {
            FozzyError::Trace(format!("replay drift: message id {msg_id} not found"))
        })?;
        let msg = queue.remove(idx).expect("position exists");
        delivered += 1;
        time_ms = time_ms.saturating_add(1);
        emit_delivery_start(&mut events, &queue, &msg, time_ms);
        let delivered_msg_id = msg.id;
        deliver_message(msg, &mut nodes, &mut queue, &mut next_id)?;
        emit_delivery_end(&mut events, delivered_msg_id, time_ms);

        if let Some(finding) = check_invariants(scenario, &nodes, InvariantPhase::Progress) {
            findings.push(finding);
            return Ok((
                ExitStatus::Fail,
                findings,
                events,
                delivered,
                decisions.to_vec(),
            ));
        }
    }

    let deliverable = deliverable_indices(&queue, &nodes, &net);
    if !deliverable.is_empty() {
        let mut seen_strategy_edges = HashSet::new();
        let idx = deliverable[pick_index(
            &queue,
            &deliverable,
            schedule,
            &mut rng,
            &mut seen_strategy_edges,
        )];
        let msg = queue.remove(idx).expect("index exists");
        delivered += 1;
        time_ms = time_ms.saturating_add(1);
        let msg_id = msg.id;
        emit_replay_fallback_start(&mut events, &queue, &msg, time_ms);
        deliver_message(msg, &mut nodes, &mut queue, &mut next_id)?;
        emit_delivery_end(&mut events, msg_id, time_ms);
    } else {
        emit_scheduler_idle_event(&mut events, &queue, &deliverable, time_ms);
    }

    if let Some(finding) = check_invariants(scenario, &nodes, InvariantPhase::Final) {
        findings.push(finding);
        return Ok((
            ExitStatus::Fail,
            findings,
            events,
            delivered,
            decisions.to_vec(),
        ));
    }

    Ok((
        ExitStatus::Pass,
        findings,
        events,
        delivered,
        decisions.to_vec(),
    ))
}

fn init_nodes(scenario: &ScenarioV1Explore) -> NodeMap {
    scenario
        .nodes
        .iter()
        .map(|n| {
            (
                n.clone(),
                Node {
                    running: true,
                    kv: Default::default(),
                    kv_version: Default::default(),
                },
            )
        })
        .collect()
}

fn replay_message_id(decision: &crate::Decision) -> FozzyResult<u64> {
    match decision {
        crate::Decision::ExploreDeliver { msg_id } => Ok(*msg_id),
        crate::Decision::SchedulerPick { task_id, .. } => Ok(*task_id),
        crate::Decision::Step { name, .. } => {
            let Some(id_str) = name.strip_prefix("deliver:") else {
                return Err(FozzyError::Trace("invalid deliver decision".to_string()));
            };
            id_str
                .parse()
                .map_err(|_| FozzyError::Trace("invalid deliver decision".to_string()))
        }
        _ => Err(FozzyError::Trace("invalid deliver decision".to_string())),
    }
}

fn emit_scheduler_idle_event(
    events: &mut Vec<TraceEvent>,
    queue: &MessageQueue,
    deliverable: &[usize],
    time_ms: u64,
) {
    events.push(TraceEvent {
        time_ms,
        name: if queue.is_empty() {
            "sched_wait".to_string()
        } else {
            "sched_starvation".to_string()
        },
        fields: serde_json::Map::from_iter([
            (
                "queue_len".to_string(),
                serde_json::json!(queue.len() as u64),
            ),
            (
                "deliverable_len".to_string(),
                serde_json::json!(deliverable.len() as u64),
            ),
        ]),
    });
}

fn emit_delivery_start(
    events: &mut Vec<TraceEvent>,
    queue: &MessageQueue,
    msg: &super::types::Message,
    time_ms: u64,
) {
    events.push(TraceEvent {
        time_ms,
        name: "sched_pick".to_string(),
        fields: serde_json::Map::from_iter([
            ("task_id".to_string(), serde_json::json!(msg.id)),
            (
                "queue_len".to_string(),
                serde_json::json!(queue.len() as u64),
            ),
        ]),
    });
    events.push(TraceEvent {
        time_ms,
        name: "span_start".to_string(),
        fields: serde_json::Map::from_iter([
            (
                "span".to_string(),
                serde_json::json!(format!("deliver-{}", msg.id)),
            ),
            ("task".to_string(), serde_json::json!("deliver")),
        ]),
    });
    events.push(TraceEvent {
        time_ms,
        name: "deliver".to_string(),
        fields: serde_json::Map::from_iter([
            ("id".to_string(), serde_json::Value::Number(msg.id.into())),
            (
                "from".to_string(),
                serde_json::Value::String(msg.from.clone()),
            ),
            ("to".to_string(), serde_json::Value::String(msg.to.clone())),
            (
                "kind".to_string(),
                serde_json::Value::String(msg.kind.clone()),
            ),
            (
                "key".to_string(),
                serde_json::Value::String(msg.key.clone()),
            ),
            (
                "payload_size".to_string(),
                serde_json::json!(msg.value.len() as u64),
            ),
        ]),
    });
    events.push(TraceEvent {
        time_ms,
        name: "capability_net".to_string(),
        fields: serde_json::Map::from_iter([
            ("op".to_string(), serde_json::json!("deliver")),
            (
                "payload_bytes".to_string(),
                serde_json::json!(msg.value.len() as u64),
            ),
            ("duration_ms".to_string(), serde_json::json!(1u64)),
        ]),
    });
}

fn emit_replay_fallback_start(
    events: &mut Vec<TraceEvent>,
    queue: &MessageQueue,
    msg: &super::types::Message,
    time_ms: u64,
) {
    events.push(TraceEvent {
        time_ms,
        name: "sched_pick".to_string(),
        fields: serde_json::Map::from_iter([
            ("task_id".to_string(), serde_json::json!(msg.id)),
            (
                "queue_len".to_string(),
                serde_json::json!(queue.len() as u64),
            ),
        ]),
    });
    events.push(TraceEvent {
        time_ms,
        name: "span_start".to_string(),
        fields: serde_json::Map::from_iter([
            (
                "span".to_string(),
                serde_json::json!(format!("deliver-{}", msg.id)),
            ),
            ("task".to_string(), serde_json::json!("deliver")),
        ]),
    });
}

fn emit_delivery_end(events: &mut Vec<TraceEvent>, msg_id: u64, time_ms: u64) {
    events.push(TraceEvent {
        time_ms,
        name: "span_end".to_string(),
        fields: serde_json::Map::from_iter([
            (
                "span".to_string(),
                serde_json::json!(format!("deliver-{}", msg_id)),
            ),
            ("status".to_string(), serde_json::json!("ok")),
            ("duration_ms".to_string(), serde_json::json!(1u64)),
        ]),
    });
}

fn pick_index(
    queue: &MessageQueue,
    deliverable: &[usize],
    strategy: ScheduleStrategy,
    rng: &mut ChaCha20Rng,
    seen_edges: &mut HashSet<u64>,
) -> usize {
    match strategy {
        ScheduleStrategy::Fifo | ScheduleStrategy::Bfs => 0,
        ScheduleStrategy::Dfs => deliverable.len().saturating_sub(1),
        ScheduleStrategy::Random | ScheduleStrategy::Pct => {
            if deliverable.is_empty() {
                0
            } else {
                (rng.next_u64() as usize) % deliverable.len()
            }
        }
        ScheduleStrategy::CoverageGuided => {
            for (pos, idx) in deliverable.iter().enumerate() {
                let m = &queue[*idx];
                let edge = stable_edge(&format!("{}|{}|{}|{}", m.kind, m.from, m.to, m.key));
                if seen_edges.insert(edge) {
                    return pos;
                }
            }
            if deliverable.is_empty() {
                0
            } else {
                (rng.next_u64() as usize) % deliverable.len()
            }
        }
    }
}
