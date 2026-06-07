use super::*;

pub(crate) fn memory_top_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let total = value.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
    let leaks = value
        .get("leaks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let shown = leaks.len();
    let mut seen_alloc_ids = std::collections::BTreeSet::new();
    let duplicate_alloc_ids = leaks
        .iter()
        .filter(|leak| {
            leak.get("allocId")
                .and_then(|v| v.as_u64())
                .is_some_and(|id| !seen_alloc_ids.insert(id))
        })
        .count();
    let invalid_rows = leaks
        .iter()
        .filter(|leak| {
            leak.get("allocId")
                .and_then(|v| v.as_u64())
                .is_none_or(|id| id == 0)
                || leak
                    .get("bytes")
                    .and_then(|v| v.as_u64())
                    .is_none_or(|bytes| bytes == 0)
                || leak
                    .get("callsiteHash")
                    .and_then(|v| v.as_str())
                    .is_none_or(|hash| hash.trim().is_empty())
        })
        .count();
    let consistent = shown <= total as usize && duplicate_alloc_ids == 0 && invalid_rows == 0;
    (
        if total > 0 || !consistent {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        format!(
            "total_leaks={} shown={} duplicate_alloc_ids={} invalid_rows={} consistent={}",
            total, shown, duplicate_alloc_ids, invalid_rows, consistent
        ),
    )
}

pub(crate) fn memory_diff_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let left_leaked = value
        .get("leftLeakedBytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let right_leaked = value
        .get("rightLeakedBytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let left_peak = value
        .get("leftPeakBytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let right_peak = value
        .get("rightPeakBytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let left_allocs = value
        .get("leftLeakedAllocs")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let right_allocs = value
        .get("rightLeakedAllocs")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let leaked = value
        .get("deltaLeakedBytes")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let allocs = value
        .get("deltaLeakedAllocs")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let peak = value
        .get("deltaPeakBytes")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let consistent = leaked == right_leaked as i64 - left_leaked as i64
        && allocs == right_allocs as i64 - left_allocs as i64
        && peak == right_peak as i64 - left_peak as i64;
    (
        if leaked != 0 || allocs != 0 || peak != 0 || !consistent {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        format!(
            "delta_leaked_bytes={} delta_leaked_allocs={} delta_peak_bytes={} consistent={}",
            leaked, allocs, peak, consistent
        ),
    )
}

pub(crate) fn memory_graph_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let nodes = value
        .pointer("/graph/nodes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let edges = value
        .pointer("/graph/edges")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let node_count = nodes.len();
    let edge_count = edges.len();
    let mut node_ids = std::collections::BTreeSet::new();
    let mut duplicate_nodes = 0usize;
    let mut invalid_nodes = 0usize;
    for node in &nodes {
        match node.get("id").and_then(|v| v.as_str()).map(str::trim) {
            Some(id) if !id.is_empty() => {
                if !node_ids.insert(id.to_string()) {
                    duplicate_nodes += 1;
                }
            }
            _ => {
                invalid_nodes += 1;
            }
        }
    }
    let mut edge_keys = std::collections::BTreeSet::new();
    let mut duplicate_edges = 0usize;
    let mut invalid_edges = 0usize;
    for edge in &edges {
        let from = edge.get("from").and_then(|v| v.as_str()).map(str::trim);
        let to = edge.get("to").and_then(|v| v.as_str()).map(str::trim);
        let kind = edge.get("kind").and_then(|v| v.as_str()).map(str::trim);
        if let (Some(from), Some(to), Some(kind)) = (from, to, kind)
            && !from.is_empty()
            && !to.is_empty()
            && !kind.is_empty()
            && !edge_keys.insert(format!("{from}\u{0}{to}\u{0}{kind}"))
        {
            duplicate_edges += 1;
        }
        if from.is_none_or(|id| id.is_empty() || !node_ids.contains(id))
            || to.is_none_or(|id| id.is_empty() || !node_ids.contains(id))
            || kind.is_none_or(|kind| kind.is_empty())
        {
            invalid_edges += 1;
        }
    }
    let consistent = invalid_nodes == 0
        && invalid_edges == 0
        && duplicate_nodes == 0
        && duplicate_edges == 0
        && node_ids.len() == node_count;
    (
        if node_count == 0 && edge_count == 0 {
            FullStepStatus::Skipped
        } else if consistent {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        if node_count == 0 && edge_count == 0 {
            format!("nodes={} edges={}", node_count, edge_count)
        } else {
            format!(
                "nodes={} edges={} unique_nodes={} invalid_nodes={} duplicate_nodes={} duplicate_edges={} invalid_edges={} consistent={}",
                node_count,
                edge_count,
                node_ids.len(),
                invalid_nodes,
                duplicate_nodes,
                duplicate_edges,
                invalid_edges,
                consistent
            )
        },
    )
}
