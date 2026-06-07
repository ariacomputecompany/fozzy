use super::*;

pub(crate) fn flaky_report_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let run_count = value.get("runCount").and_then(|v| v.as_u64()).unwrap_or(0);
    let is_flaky = value
        .get("isFlaky")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let flake_rate = value
        .get("flakeRatePct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let status_counts = value
        .get("statusCounts")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let invalid_status_keys = status_counts
        .keys()
        .filter(|key| {
            let key = key.trim();
            key.is_empty() || !matches!(key, "pass" | "fail" | "error" | "timeout")
        })
        .count();
    let invalid_status_values = status_counts
        .values()
        .filter(|count| count.as_u64().is_none_or(|v| v == 0))
        .count();
    let status_variant_count = status_counts.len() as u64;
    let status_total = status_counts
        .values()
        .filter_map(|count| count.as_u64())
        .sum::<u64>();
    let finding_title_sets = value
        .get("findingTitleSets")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let invalid_finding_rows = finding_title_sets
        .iter()
        .filter(|set| {
            set.as_array().is_none_or(|items| {
                items
                    .iter()
                    .any(|item| item.as_str().is_none_or(|s| s.trim().is_empty()))
            })
        })
        .count();
    let duplicate_titles_within_rows = finding_title_sets
        .iter()
        .filter_map(|set| set.as_array())
        .map(|items| {
            let mut seen = std::collections::BTreeSet::new();
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim))
                .filter(|s| !s.is_empty())
                .filter(|title| !seen.insert((*title).to_string()))
                .count()
        })
        .sum::<usize>();
    let mut seen_finding_rows = std::collections::BTreeSet::new();
    let duplicate_finding_rows = finding_title_sets
        .iter()
        .filter_map(|set| set.as_array())
        .filter(|items| {
            let normalized = items
                .iter()
                .filter_map(|item| item.as_str().map(str::trim))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\u{0}");
            !normalized.is_empty() && !seen_finding_rows.insert(normalized)
        })
        .count();
    let unique_finding_variant_count = seen_finding_rows.len() as u64;
    let finding_variant_count = finding_title_sets.len() as u64;
    let derived_flaky = status_variant_count > 1 || unique_finding_variant_count > 1;
    let rate_ok = if derived_flaky {
        flake_rate > 0.0 && flake_rate <= 100.0
    } else {
        flake_rate == 0.0
    };
    let consistent = run_count > 0
        && is_flaky == derived_flaky
        && rate_ok
        && invalid_status_keys == 0
        && invalid_status_values == 0
        && status_total == run_count
        && invalid_finding_rows == 0
        && duplicate_titles_within_rows == 0
        && duplicate_finding_rows == 0;
    (
        if !consistent || is_flaky {
            FullStepStatus::Failed
        } else {
            FullStepStatus::Passed
        },
        format!(
            "run_count={} status_total={} is_flaky={} derived_flaky={} flake_rate_pct={} status_variants={} finding_variants={} unique_finding_variants={} invalid_status_keys={} invalid_status_values={} invalid_finding_rows={} duplicate_titles_within_rows={} duplicate_finding_rows={}",
            run_count,
            status_total,
            is_flaky,
            derived_flaky,
            flake_rate,
            status_variant_count,
            finding_variant_count,
            unique_finding_variant_count,
            invalid_status_keys,
            invalid_status_values,
            invalid_finding_rows,
            duplicate_titles_within_rows,
            duplicate_finding_rows
        ),
    )
}
