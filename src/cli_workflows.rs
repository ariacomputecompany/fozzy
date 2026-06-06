use super::*;
use fozzy::RunMode;

fn summarize_profile_top(value: &serde_json::Value) -> String {
    let warnings = value
        .get("warnings")
        .and_then(|v| v.as_array())
        .map(|items| {
            let rows = items
                .iter()
                .filter_map(|v| v.as_str().map(ToString::to_string))
                .collect::<Vec<_>>();
            if rows.is_empty() {
                "<none>".to_string()
            } else {
                rows.join("; ")
            }
        })
        .unwrap_or_else(|| "<none>".to_string());
    let empty_domains = value
        .get("emptyDomains")
        .and_then(|v| v.as_array())
        .map(|items| {
            let rows = items
                .iter()
                .filter_map(|item| {
                    let domain = item.get("domain").and_then(|v| v.as_str())?;
                    let reason = item.get("reason").and_then(|v| v.as_str())?;
                    Some(format!("{domain}:{reason}"))
                })
                .collect::<Vec<_>>();
            if rows.is_empty() {
                "<none>".to_string()
            } else {
                rows.join("; ")
            }
        })
        .unwrap_or_else(|| "<none>".to_string());
    format!("warnings={warnings} empty_domains={empty_domains}")
}

fn known_profile_domain(domain: &str) -> bool {
    matches!(domain, "cpu" | "heap" | "latency" | "io" | "sched")
}

fn known_ci_check_name(name: &str) -> bool {
    matches!(
        name,
        "trace_verify"
            | "replay_outcome_class"
            | "replay_warning_parity"
            | "strict_warning_policy"
            | "replay_memory_parity"
            | "memory_policy"
            | "artifacts_zip_integrity"
            | "flake_budget"
            | "perf_p99_budget"
    )
}

fn known_doctor_issue_code(code: &str) -> bool {
    matches!(
        code,
        "proc_unmatched_preflight" | "determinism_audit_mismatch"
    )
}

fn known_doctor_signal_source(source: &str) -> bool {
    matches!(source, "env")
}

fn known_profile_time_domain(time_domain: &str) -> bool {
    matches!(time_domain, "host_monotonic_time" | "virtual_time")
}

fn known_profile_metric(domain: &str, metric: &str) -> bool {
    match domain {
        "cpu" => metric == "cpu_time_ms",
        "heap" => {
            matches!(metric, "alloc_bytes" | "in_use_bytes")
                || metric.starts_with("callsite:")
                    && (metric.ends_with(".alloc_bytes")
                        || metric.ends_with(".in_use_bytes")
                        || metric.ends_with(".alloc_rate_per_sec"))
        }
        "latency" => matches!(
            metric,
            "p95_latency_ms" | "p99_latency_ms" | "max_latency_ms"
        ),
        "io" => metric == "io_ops",
        "sched" => metric == "sched_ops",
        _ => false,
    }
}

fn valid_profile_explain_shifted_path(domain: &str, shifted_path: &str) -> bool {
    shifted_path
        .strip_prefix("metric::")
        .is_some_and(|metric| known_profile_metric(domain, metric))
}

fn known_profile_evidence_pointer(pointer: &str) -> bool {
    std::path::Path::new(pointer)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                "profile.metrics.json"
                    | "profile.latency.json"
                    | "profile.cpu.json"
                    | "profile.heap.json"
            )
        })
}

fn expected_profile_time_domain(metric: &str) -> &'static str {
    if metric == "cpu_time_ms" {
        "host_monotonic_time"
    } else {
        "virtual_time"
    }
}

fn profile_top_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let warnings = value
        .get("warnings")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let empty_domains = value
        .get("emptyDomains")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let warning_count = warnings.len();
    let empty_count = empty_domains.len();
    let invalid_warnings = warnings
        .iter()
        .filter(|warning| warning.as_str().is_none_or(|s| s.trim().is_empty()))
        .count();
    let mut seen_warnings = std::collections::BTreeSet::new();
    let duplicate_warnings = warnings
        .iter()
        .filter(|warning| {
            warning
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_some_and(|s| !seen_warnings.insert(s.to_string()))
        })
        .count();
    let invalid_empty_domains = empty_domains
        .iter()
        .filter(|item| {
            item.get("domain")
                .and_then(|v| v.as_str())
                .is_none_or(|s| s.trim().is_empty() || !known_profile_domain(s.trim()))
                || item
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .is_none_or(|s| s.trim().is_empty())
                || item.get("empty").and_then(|v| v.as_bool()) != Some(true)
        })
        .count();
    let mut seen_empty_domains = std::collections::BTreeSet::new();
    let duplicate_empty_domains = empty_domains
        .iter()
        .filter(|item| {
            let domain = item.get("domain").and_then(|v| v.as_str()).map(str::trim);
            let reason = item.get("reason").and_then(|v| v.as_str()).map(str::trim);
            match (domain, reason) {
                (Some(domain), Some(reason)) if !domain.is_empty() && !reason.is_empty() => {
                    !seen_empty_domains.insert(format!("{domain}\u{0}{reason}"))
                }
                _ => false,
            }
        })
        .count();
    let has_concrete_domain_data = ["cpu", "heap", "latency", "io", "sched"]
        .iter()
        .any(|domain| {
            value.get(*domain)
                .and_then(|v| v.as_array())
                .is_some_and(|items| !items.is_empty())
        });
    let structural_invalid = invalid_warnings > 0
        || duplicate_warnings > 0
        || invalid_empty_domains > 0
        || duplicate_empty_domains > 0;
    let status = if structural_invalid || warning_count > 0 {
        FullStepStatus::Failed
    } else if empty_count > 0 && !has_concrete_domain_data {
        FullStepStatus::Skipped
    } else {
        FullStepStatus::Passed
    };
    (
        status,
        format!(
            "{} has_concrete_domain_data={} invalid_warnings={} duplicate_warnings={} invalid_empty_domains={} duplicate_empty_domains={}",
            summarize_profile_top(value),
            has_concrete_domain_data,
            invalid_warnings,
            duplicate_warnings,
            invalid_empty_domains,
            duplicate_empty_domains
        ),
    )
}

fn profile_diff_status(
    value: &serde_json::Value,
    require_stable: bool,
) -> (FullStepStatus, String) {
    let domains = value
        .get("domains")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let regressions = value
        .get("regressions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let verdict = value
        .pointer("/summary/verdict")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let regression_count = value
        .pointer("/summary/regressionCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let improvement_count = value
        .pointer("/summary/improvementCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let significant = value
        .pointer("/summary/significantRegressionCount")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let top_regression_metric = value
        .pointer("/summary/topRegressionMetric")
        .and_then(|v| v.as_str());
    let derived_regressions = regressions
        .iter()
        .filter(|row| row.get("isRegression").and_then(|v| v.as_bool()) == Some(true))
        .count() as u64;
    let derived_improvements = regressions
        .iter()
        .filter(|row| row.get("classification").and_then(|v| v.as_str()) == Some("improvement"))
        .count() as u64;
    let derived_significant = regressions
        .iter()
        .filter(|row| {
            row.get("isRegression").and_then(|v| v.as_bool()) == Some(true)
                && row.get("isSignificant").and_then(|v| v.as_bool()) == Some(true)
        })
        .count() as u64;
    let derived_top_regression_metric = regressions.iter().find_map(|row| {
        (row.get("isRegression").and_then(|v| v.as_bool()) == Some(true))
            .then(|| row.get("metric").and_then(|v| v.as_str()))
            .flatten()
    });
    let invalid_domains = domains
        .iter()
        .filter(|domain| {
            domain
                .as_str()
                .is_none_or(|s| s.trim().is_empty() || !known_profile_domain(s.trim()))
        })
        .count();
    let mut seen_domains = std::collections::BTreeSet::new();
    let duplicate_domains = domains
        .iter()
        .filter(|domain| {
            domain
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_some_and(|s| !seen_domains.insert(s.to_string()))
        })
        .count();
    let declared_domains = domains
        .iter()
        .filter_map(|domain| domain.as_str().map(str::trim))
        .filter(|domain| !domain.is_empty() && known_profile_domain(domain))
        .map(ToString::to_string)
        .collect::<std::collections::BTreeSet<_>>();
    let derived_domains = regressions
        .iter()
        .filter_map(|row| row.get("domain").and_then(|v| v.as_str()).map(str::trim))
        .filter(|domain| !domain.is_empty() && known_profile_domain(domain))
        .map(ToString::to_string)
        .collect::<std::collections::BTreeSet<_>>();
    let invalid_rows = regressions
        .iter()
        .filter(|row| {
            let classification = row.get("classification").and_then(|v| v.as_str());
            let is_regression = row.get("isRegression").and_then(|v| v.as_bool());
            let is_significant = row.get("isSignificant").and_then(|v| v.as_bool());
            let domain = row.get("domain").and_then(|v| v.as_str()).map(str::trim);
            let metric = row.get("metric").and_then(|v| v.as_str()).map(str::trim);
            let time_domain = row.get("timeDomain").and_then(|v| v.as_str()).map(str::trim);
            let classification_invalid = !matches!(
                classification,
                Some("regression") | Some("improvement") | Some("stable")
            );
            let semantic_mismatch = match (classification, is_regression, is_significant) {
                (Some("regression"), Some(true), Some(_)) => false,
                (Some("improvement"), Some(false), Some(false)) => false,
                (Some("stable"), Some(false), Some(false)) => false,
                (Some("improvement"), Some(false), Some(true)) => false,
                _ => true,
            };
            domain.is_none_or(|s| s.is_empty() || !known_profile_domain(s))
                || metric.is_none_or(|s| s.is_empty())
                || time_domain.is_none_or(|s| s.is_empty() || !known_profile_time_domain(s))
                || !matches!((domain, metric), (Some(domain), Some(metric)) if known_profile_metric(domain, metric))
                || !matches!((metric, time_domain), (Some(metric), Some(time_domain)) if expected_profile_time_domain(metric) == time_domain)
                || row
                    .get("classification")
                    .and_then(|v| v.as_str())
                    .is_none_or(|s| s.trim().is_empty())
                || classification_invalid
                || semantic_mismatch
        })
        .count();
    let mut seen_regressions = std::collections::BTreeSet::new();
    let duplicate_rows = regressions
        .iter()
        .filter(|row| {
            let domain = row.get("domain").and_then(|v| v.as_str()).map(str::trim);
            let metric = row.get("metric").and_then(|v| v.as_str()).map(str::trim);
            let time_domain = row
                .get("timeDomain")
                .and_then(|v| v.as_str())
                .map(str::trim);
            match (domain, metric, time_domain) {
                (Some(domain), Some(metric), Some(time_domain))
                    if !domain.is_empty() && !metric.is_empty() && !time_domain.is_empty() =>
                {
                    !seen_regressions.insert(format!("{domain}\u{0}{metric}\u{0}{time_domain}"))
                }
                _ => false,
            }
        })
        .count();
    let expected_verdict = if derived_significant > 0 {
        "regression_detected"
    } else if derived_regressions > 0 {
        "minor_regression"
    } else if derived_improvements > 0 {
        "improvement"
    } else {
        "stable"
    };
    let consistent = regression_count == derived_regressions
        && improvement_count == derived_improvements
        && significant == derived_significant
        && verdict == expected_verdict
        && top_regression_metric == derived_top_regression_metric
        && invalid_domains == 0
        && duplicate_domains == 0
        && declared_domains == derived_domains
        && invalid_rows == 0
        && duplicate_rows == 0;
    let known_non_regression = matches!(verdict, "stable" | "improvement");
    let status = if significant > 0
        || regression_count > 0
        || !known_non_regression
        || !consistent
        || (require_stable && verdict != "stable")
    {
        FullStepStatus::Failed
    } else {
        FullStepStatus::Passed
    };
    (
        status,
        format!(
            "verdict={} regressions={} significant_regressions={} improvements={} invalid_domains={} duplicate_domains={} domains_consistent={} invalid_rows={} duplicate_rows={} consistent={}",
            verdict,
            regression_count,
            significant,
            improvement_count,
            invalid_domains,
            duplicate_domains,
            declared_domains == derived_domains,
            invalid_rows,
            duplicate_rows,
            consistent
        ),
    )
}

fn profile_explain_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let cause_domain = value
        .get("likelyCauseDomain")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let shifted_path = value
        .get("topShiftedPath")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let regression_statement = value
        .get("regressionStatement")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let evidence_pointers = value
        .get("evidencePointers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let invalid_evidence_pointers = evidence_pointers
        .iter()
        .filter(|pointer| {
            pointer.as_str().is_none_or(|s| {
                let s = s.trim();
                s.is_empty() || !known_profile_evidence_pointer(s)
            })
        })
        .count();
    let normalized_pointers = evidence_pointers
        .iter()
        .filter_map(|pointer| pointer.as_str().map(str::trim))
        .filter(|s| !s.is_empty())
        .filter_map(|pointer| {
            std::path::Path::new(pointer)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut seen_pointers = std::collections::BTreeSet::new();
    let duplicate_evidence_pointers = evidence_pointers
        .iter()
        .filter(|pointer| {
            pointer
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_some_and(|s| !seen_pointers.insert(s.to_string()))
        })
        .count();
    let metrics_pointer_present = normalized_pointers.contains("profile.metrics.json");
    let domain_pointer_required = matches!(cause_domain.trim(), "cpu" | "heap" | "latency");
    let domain_pointer_present = match cause_domain.trim() {
        "cpu" => normalized_pointers.contains("profile.cpu.json"),
        "heap" => normalized_pointers.contains("profile.heap.json"),
        "latency" => normalized_pointers.contains("profile.latency.json"),
        _ => true,
    };
    let evidence_count = evidence_pointers.len();
    let status = if cause_domain == "unknown"
        || cause_domain.trim().is_empty()
        || !known_profile_domain(cause_domain.trim())
        || shifted_path == "n/a"
        || shifted_path.trim().is_empty()
        || shifted_path == "unknown"
        || !valid_profile_explain_shifted_path(cause_domain.trim(), shifted_path.trim())
        || regression_statement.is_empty()
        || regression_statement == "no measurable regression shift found"
        || regression_statement.starts_with("run ")
        || evidence_count == 0
        || !metrics_pointer_present
        || (domain_pointer_required && !domain_pointer_present)
        || invalid_evidence_pointers > 0
        || duplicate_evidence_pointers > 0
    {
        FullStepStatus::Skipped
    } else {
        FullStepStatus::Passed
    };
    (
        status,
        format!(
            "cause_domain={} shifted_path={} evidence_pointers={} metrics_pointer_present={} domain_pointer_required={} domain_pointer_present={} invalid_evidence_pointers={} duplicate_evidence_pointers={}",
            cause_domain,
            shifted_path,
            evidence_count,
            metrics_pointer_present,
            domain_pointer_required,
            domain_pointer_present,
            invalid_evidence_pointers,
            duplicate_evidence_pointers
        ),
    )
}

fn flaky_report_status(value: &serde_json::Value) -> (FullStepStatus, String) {
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

fn memory_top_status(value: &serde_json::Value) -> (FullStepStatus, String) {
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

fn memory_diff_status(value: &serde_json::Value) -> (FullStepStatus, String) {
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

fn memory_graph_status(value: &serde_json::Value) -> (FullStepStatus, String) {
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

fn replay_summary_status(
    expected: Option<ExitStatus>,
    summary: &RunSummary,
    strict: bool,
    expected_seed: u64,
    expected_mode: RunMode,
) -> (FullStepStatus, String) {
    let class_ok = expected
        .map(|s| (s == ExitStatus::Pass) == (summary.status == ExitStatus::Pass))
        .unwrap_or(false);
    let strict_ok = enforce_strict_summary(strict, summary).is_ok();
    let run_id_present = !summary.identity.run_id.trim().is_empty();
    let seed_matches = summary.identity.seed == expected_seed;
    let mode_matches = summary.mode == expected_mode;
    let (artifact_identity_ok, artifact_identity_detail) =
        summary_artifact_identity_status(summary);
    (
        if class_ok
            && strict_ok
            && run_id_present
            && seed_matches
            && mode_matches
            && artifact_identity_ok
        {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "status={:?} class_ok={} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} {}",
            summary.status,
            class_ok,
            strict_ok,
            run_id_present,
            seed_matches,
            expected_seed,
            mode_matches,
            expected_mode,
            artifact_identity_detail
        ),
    )
}

fn file_artifact_status(path: &Path) -> (FullStepStatus, String) {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() > 0 => (
            FullStepStatus::Passed,
            format!("path={} bytes={}", path.display(), metadata.len()),
        ),
        Ok(metadata) if metadata.is_file() => (
            FullStepStatus::Failed,
            format!("path={} bytes=0", path.display()),
        ),
        Ok(_) => (
            FullStepStatus::Failed,
            format!("path={} is not a file", path.display()),
        ),
        Err(err) => (
            FullStepStatus::Failed,
            format!("path={} missing: {err}", path.display()),
        ),
    }
}

fn zip_artifact_status(path: &Path) -> (FullStepStatus, String) {
    let (file_status, file_detail) = file_artifact_status(path);
    if !matches!(file_status, FullStepStatus::Passed) {
        return (file_status, file_detail);
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(err) => {
            return (
                FullStepStatus::Failed,
                format!("{file_detail} zip_open_error={err}"),
            );
        }
    };
    let archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(err) => {
            return (
                FullStepStatus::Failed,
                format!("{file_detail} zip_parse_error={err}"),
            );
        }
    };
    let entries = archive.len();
    (
        if entries > 0 {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!("{file_detail} zip_entries={entries}"),
    )
}

fn report_show_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let format = value
        .get("format")
        .and_then(|v| v.as_str())
        .unwrap_or("pretty");
    let known_format = matches!(format, "pretty" | "junit" | "html");
    let content = value.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let bytes = content.len();
    let non_blank = !content.trim().is_empty();
    (
        if bytes > 0 && known_format && non_blank {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "format={format} known_format={} non_blank={} content_bytes={bytes}",
            known_format, non_blank
        ),
    )
}

fn report_query_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let status_value = value
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| value.to_string());
    (
        if status_value == "pass" {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(".status={status_value}"),
    )
}

fn report_query_paths_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let paths = value
        .get("paths")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let count = paths.len();
    let mut seen = std::collections::BTreeSet::new();
    let invalid = paths
        .iter()
        .filter(|path| path.as_str().is_none_or(|s| s.trim().is_empty()))
        .count();
    let duplicate = paths
        .iter()
        .filter_map(|path| path.as_str().map(str::trim))
        .filter(|s| !s.is_empty())
        .filter(|path| !seen.insert((*path).to_string()))
        .count();
    (
        if count > 0 && invalid == 0 && duplicate == 0 {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!("paths={count} invalid={invalid} duplicate={duplicate}"),
    )
}

fn summary_artifact_identity_status(summary: &RunSummary) -> (bool, String) {
    let report_path = summary.identity.report_path.as_deref().map(str::trim);
    let artifacts_dir = summary.identity.artifacts_dir.as_deref().map(str::trim);
    let report_present = report_path.is_some_and(|path| !path.is_empty());
    let artifacts_present = artifacts_dir.is_some_and(|path| !path.is_empty());
    let report_path = report_path
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let artifacts_dir = artifacts_dir
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let report_exists = report_path.as_ref().is_some_and(|path| {
        std::fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.len() > 0)
            .unwrap_or(false)
    });
    let artifacts_exists = artifacts_dir.as_ref().is_some_and(|path| {
        std::fs::metadata(path)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
    });
    let report_matches_dir = report_path
        .as_ref()
        .zip(artifacts_dir.as_ref())
        .is_some_and(|(report, dir)| {
            report.parent().is_some_and(|parent| parent == dir)
                && report.file_name().is_some_and(|name| name == "report.json")
                && dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == summary.identity.run_id)
        });
    let manifest_path = artifacts_dir.as_ref().map(|dir| dir.join("manifest.json"));
    let manifest_exists = manifest_path.as_ref().is_some_and(|path| {
        std::fs::metadata(path)
            .map(|metadata| metadata.is_file() && metadata.len() > 0)
            .unwrap_or(false)
    });
    let report_content_matches = report_path.as_ref().is_some_and(|path| {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<RunSummary>(&bytes).ok())
            .is_some_and(|report| {
                report.identity.run_id == summary.identity.run_id
                    && report.identity.seed == summary.identity.seed
                    && report.mode == summary.mode
                    && report.status == summary.status
                    && report.identity.report_path == summary.identity.report_path
                    && report.identity.artifacts_dir == summary.identity.artifacts_dir
            })
    });
    let manifest_content_matches = manifest_path.as_ref().is_some_and(|path| {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<fozzy::RunManifest>(&bytes).ok())
            .is_some_and(|manifest| {
                manifest.run_id == summary.identity.run_id
                    && manifest.seed == summary.identity.seed
                    && manifest.mode == summary.mode
                    && manifest.status == summary.status
                    && manifest.report_path == summary.identity.report_path
                    && manifest.artifacts_dir == summary.identity.artifacts_dir
                    && manifest.trace_path == summary.identity.trace_path
                    && manifest.findings_count == summary.findings.len()
                    && manifest.duration_ms == summary.duration_ms
                    && manifest.duration_ns == summary.duration_ns
            })
    });
    (
        report_present
            && artifacts_present
            && report_exists
            && artifacts_exists
            && report_matches_dir
            && report_content_matches
            && manifest_exists
            && manifest_content_matches,
        format!(
            "report_present={} artifacts_present={} report_exists={} artifacts_exists={} report_matches_dir={} report_content_matches={} manifest_exists={} manifest_content_matches={}",
            report_present,
            artifacts_present,
            report_exists,
            artifacts_exists,
            report_matches_dir,
            report_content_matches,
            manifest_exists,
            manifest_content_matches
        ),
    )
}

fn trace_summary_identity_status(trace_path: &Path, summary: &RunSummary) -> (bool, String) {
    let trace_exists = std::fs::metadata(trace_path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false);
    let trace_content_matches = std::fs::read(trace_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<fozzy::TraceFile>(&bytes).ok())
        .is_some_and(|trace| {
            trace.summary.identity.run_id == summary.identity.run_id
                && trace.summary.identity.seed == summary.identity.seed
                && trace.summary.mode == summary.mode
                && trace.summary.status == summary.status
                && trace.summary.identity.trace_path == summary.identity.trace_path
                && trace.summary.identity.report_path == summary.identity.report_path
                && trace.summary.identity.artifacts_dir == summary.identity.artifacts_dir
        });
    (
        trace_exists && trace_content_matches,
        format!(
            "trace_exists={} trace_content_matches={}",
            trace_exists, trace_content_matches
        ),
    )
}

fn run_summary_pass_status(
    summary: &RunSummary,
    strict: bool,
    expected_seed: u64,
    expected_mode: RunMode,
) -> (FullStepStatus, String) {
    let strict_ok = enforce_strict_summary(strict, summary).is_ok();
    let run_id_present = !summary.identity.run_id.trim().is_empty();
    let seed_matches = summary.identity.seed == expected_seed;
    let mode_matches = summary.mode == expected_mode;
    let (artifact_identity_ok, artifact_identity_detail) =
        summary_artifact_identity_status(summary);
    (
        if summary.status == ExitStatus::Pass
            && strict_ok
            && run_id_present
            && seed_matches
            && mode_matches
            && artifact_identity_ok
        {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "status={:?} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} {}",
            summary.status,
            strict_ok,
            run_id_present,
            seed_matches,
            expected_seed,
            mode_matches,
            expected_mode,
            artifact_identity_detail
        ),
    )
}

fn recorded_trace_status(
    summary: &RunSummary,
    strict: bool,
    expected_seed: u64,
    expected_mode: RunMode,
    trace_path: &Path,
) -> (FullStepStatus, String) {
    let (summary_status, summary_detail) =
        run_summary_pass_status(summary, strict, expected_seed, expected_mode);
    let (file_status, file_detail) = file_artifact_status(trace_path);
    let (trace_identity_ok, trace_identity_detail) =
        trace_summary_identity_status(trace_path, summary);
    let reported_trace = summary.identity.trace_path.as_deref();
    let reported_matches = reported_trace.is_some_and(|reported| Path::new(reported) == trace_path);
    let status = if reported_matches
        && matches!(summary_status, FullStepStatus::Passed)
        && matches!(file_status, FullStepStatus::Passed)
        && trace_identity_ok
    {
        FullStepStatus::Passed
    } else {
        FullStepStatus::Failed
    };
    (
        status,
        format!(
            "{} trace_reported={} trace_matches={} {} {}",
            summary_detail,
            reported_trace.is_some(),
            reported_matches,
            file_detail,
            trace_identity_detail
        ),
    )
}

fn shrink_step_status(
    primary_status: Option<ExitStatus>,
    summary: &RunSummary,
    strict: bool,
    expected_seed: u64,
    expected_mode: RunMode,
    allow_expected_failures: bool,
    out_trace: &Path,
) -> (FullStepStatus, String, String) {
    let strict_ok = enforce_strict_summary(strict, summary).is_ok();
    let run_id_present = !summary.identity.run_id.trim().is_empty();
    let seed_matches = summary.identity.seed == expected_seed;
    let mode_matches = summary.mode == expected_mode;
    let (file_status, file_detail) = file_artifact_status(out_trace);
    let artifact_ok = matches!(file_status, FullStepStatus::Passed);
    let (trace_identity_ok, trace_identity_detail) =
        trace_summary_identity_status(out_trace, summary);
    let reported_trace = summary.identity.trace_path.as_deref();
    let reported_matches = reported_trace.is_some_and(|reported| Path::new(reported) == out_trace);
    if allow_expected_failures {
        match primary_status {
            Some(primary) => {
                let class_ok = shrink_status_matches(primary, summary.status);
                let classification = if class_ok
                    && strict_ok
                    && run_id_present
                    && seed_matches
                    && mode_matches
                    && artifact_ok
                    && reported_matches
                    && trace_identity_ok
                {
                    "expected_fail_class_preserved"
                } else if !class_ok {
                    "expected_fail_class_mismatch"
                } else if !run_id_present {
                    "run_identity_missing"
                } else if !seed_matches {
                    "seed_mismatch"
                } else if !mode_matches {
                    "mode_mismatch"
                } else if !artifact_ok {
                    "out_trace_missing"
                } else if !reported_matches {
                    "out_trace_identity_mismatch"
                } else if !trace_identity_ok {
                    "out_trace_content_mismatch"
                } else {
                    "strict_policy_rejected"
                };
                (
                    if class_ok
                        && strict_ok
                        && run_id_present
                        && seed_matches
                        && mode_matches
                        && artifact_ok
                        && reported_matches
                        && trace_identity_ok
                    {
                        FullStepStatus::Passed
                    } else {
                        FullStepStatus::Failed
                    },
                    format!(
                        "status={:?} class_ok={} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} trace_reported={} trace_matches={} {} {}",
                        summary.status,
                        class_ok,
                        strict_ok,
                        run_id_present,
                        seed_matches,
                        expected_seed,
                        mode_matches,
                        expected_mode,
                        reported_trace.is_some(),
                        reported_matches,
                        file_detail,
                        trace_identity_detail
                    ),
                    classification.to_string(),
                )
            }
            None => (
                FullStepStatus::Failed,
                format!(
                    "status={:?} class_ok=false strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} trace_reported={} trace_matches={} {} {}",
                    summary.status,
                    strict_ok,
                    run_id_present,
                    seed_matches,
                    expected_seed,
                    mode_matches,
                    expected_mode,
                    reported_trace.is_some(),
                    reported_matches,
                    file_detail,
                    trace_identity_detail
                ),
                "primary_status_missing".to_string(),
            ),
        }
    } else if summary.status == ExitStatus::Pass
        && strict_ok
        && run_id_present
        && seed_matches
        && mode_matches
        && artifact_ok
        && reported_matches
        && trace_identity_ok
    {
        (
            FullStepStatus::Passed,
            format!(
                "status={:?} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} trace_reported={} trace_matches={} {} {}",
                summary.status,
                strict_ok,
                run_id_present,
                seed_matches,
                expected_seed,
                mode_matches,
                expected_mode,
                reported_trace.is_some(),
                reported_matches,
                file_detail,
                trace_identity_detail
            ),
            "pass_required_policy".to_string(),
        )
    } else {
        let classification = if summary.status != ExitStatus::Pass {
            "policy_rejected_non_pass"
        } else if !run_id_present {
            "run_identity_missing"
        } else if !seed_matches {
            "seed_mismatch"
        } else if !mode_matches {
            "mode_mismatch"
        } else if !artifact_ok {
            "out_trace_missing"
        } else if !reported_matches {
            "out_trace_identity_mismatch"
        } else if !trace_identity_ok {
            "out_trace_content_mismatch"
        } else {
            "strict_policy_rejected"
        };
        (
            FullStepStatus::Failed,
            format!(
                "status={:?} strict_ok={} run_id_present={} seed_matches={} seed={} mode_matches={} mode={:?} trace_reported={} trace_matches={} {} {}",
                summary.status,
                strict_ok,
                run_id_present,
                seed_matches,
                expected_seed,
                mode_matches,
                expected_mode,
                reported_trace.is_some(),
                reported_matches,
                file_detail,
                trace_identity_detail
            ),
            classification.to_string(),
        )
    }
}

fn corpus_add_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let Some(added) = value.get("added").and_then(|v| v.as_str()) else {
        return (
            FullStepStatus::Failed,
            "missing added path in corpus add response".to_string(),
        );
    };
    let added = PathBuf::from(added);
    file_artifact_status(&added)
}

fn listed_file_status(path: &Path) -> anyhow::Result<u64> {
    let metadata = std::fs::metadata(path)
        .map_err(|err| anyhow::anyhow!("{} missing: {err}", path.display()))?;
    anyhow::ensure!(metadata.is_file(), "{} is not a file", path.display());
    anyhow::ensure!(metadata.len() > 0, "{} is empty", path.display());
    Ok(metadata.len())
}

fn corpus_list_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let entries = value.as_array().cloned().unwrap_or_default();
    let count = entries.len();
    let mut invalid = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for entry in &entries {
        let Some(path_str) = entry.as_str() else {
            invalid.push(format!("non-string entry: {}", entry));
            continue;
        };
        let trimmed = path_str.trim();
        if trimmed.is_empty() {
            invalid.push("blank entry path".to_string());
            continue;
        }
        if !seen.insert(trimmed.to_string()) {
            invalid.push(format!("duplicate entry path: {trimmed}"));
            continue;
        }
        if let Err(err) = listed_file_status(Path::new(trimmed)) {
            invalid.push(err.to_string());
        }
    }
    (
        if count > 0 && invalid.is_empty() {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        if invalid.is_empty() {
            format!("files={count} invalid=<none>")
        } else {
            format!("files={count} invalid={}", invalid.join("; "))
        },
    )
}

fn corpus_minimize_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let before = value
        .get("filesBefore")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let after = value
        .get("filesAfter")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let removed = value
        .get("duplicatesRemoved")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bytes_before = value
        .get("bytesBefore")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bytes_after = value
        .get("bytesAfter")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let bytes_removed = value
        .get("bytesRemoved")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let file_math_ok =
        before > 0 && after > 0 && after <= before && removed == before.saturating_sub(after);
    let bytes_present = value.get("bytesBefore").is_some()
        || value.get("bytesAfter").is_some()
        || value.get("bytesRemoved").is_some();
    let byte_math_ok = !bytes_present
        || (bytes_before >= bytes_after
            && bytes_removed == bytes_before.saturating_sub(bytes_after));
    let ok = file_math_ok && byte_math_ok;
    (
        if ok {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "files_before={before} files_after={after} duplicates_removed={removed} bytes_before={bytes_before} bytes_after={bytes_after} bytes_removed={bytes_removed}"
        ),
    )
}

fn corpus_import_status(value: &serde_json::Value) -> (FullStepStatus, String) {
    let Some(import_dir) = value.get("dir").and_then(|v| v.as_str()) else {
        return (
            FullStepStatus::Failed,
            "missing dir path in corpus import response".to_string(),
        );
    };
    let path = Path::new(import_dir);
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            let mut entries = 0usize;
            let mut invalid = Vec::new();
            match std::fs::read_dir(path) {
                Ok(iter) => {
                    for entry in iter {
                        match entry {
                            Ok(entry) => {
                                entries += 1;
                                if let Err(err) = listed_file_status(&entry.path()) {
                                    invalid.push(err.to_string());
                                }
                            }
                            Err(err) => invalid
                                .push(format!("{} read_dir entry error: {err}", path.display())),
                        }
                    }
                }
                Err(err) => invalid.push(format!("{} read_dir error: {err}", path.display())),
            }
            (
                if entries > 0 && invalid.is_empty() {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                if invalid.is_empty() {
                    format!("path={} entries={} invalid=<none>", path.display(), entries)
                } else {
                    format!(
                        "path={} entries={} invalid={}",
                        path.display(),
                        entries,
                        invalid.join("; ")
                    )
                },
            )
        }
        Ok(_) => (
            FullStepStatus::Failed,
            format!("path={} is not a directory", path.display()),
        ),
        Err(err) => (
            FullStepStatus::Failed,
            format!("path={} missing: {err}", path.display()),
        ),
    }
}

fn artifacts_list_status(
    output: &fozzy::ArtifactOutput,
    fallback: &Path,
) -> (FullStepStatus, String) {
    match output {
        fozzy::ArtifactOutput::List { entries } => {
            let mut invalid = Vec::new();
            let mut seen = std::collections::BTreeSet::new();
            for entry in entries {
                let trimmed = entry.path.trim();
                if trimmed.is_empty() {
                    invalid.push("blank artifact path".to_string());
                    continue;
                }
                if !seen.insert(trimmed.to_string()) {
                    invalid.push(format!("duplicate artifact path: {trimmed}"));
                    continue;
                }
                let path = Path::new(trimmed);
                match listed_file_status(path) {
                    Ok(size) => {
                        if let Some(reported) = entry.size_bytes
                            && reported != size
                        {
                            invalid.push(format!(
                                "{} size mismatch reported={} actual={}",
                                path.display(),
                                reported,
                                size
                            ));
                        }
                    }
                    Err(err) => invalid.push(err.to_string()),
                }
            }
            (
                if entries.is_empty() || !invalid.is_empty() {
                    FullStepStatus::Failed
                } else {
                    FullStepStatus::Passed
                },
                if invalid.is_empty() {
                    format!(
                        "entries={} run={} invalid=<none>",
                        entries.len(),
                        fallback.display()
                    )
                } else {
                    format!(
                        "entries={} run={} invalid={}",
                        entries.len(),
                        fallback.display(),
                        invalid.join("; ")
                    )
                },
            )
        }
        _ => (
            FullStepStatus::Failed,
            format!("unexpected artifacts ls payload for {}", fallback.display()),
        ),
    }
}

fn artifacts_diff_status(output: &fozzy::ArtifactOutput) -> (FullStepStatus, String) {
    match output {
        fozzy::ArtifactOutput::Diff { diff } => {
            let left_ok = !diff.left.trim().is_empty();
            let right_ok = !diff.right.trim().is_empty();
            let mut invalid = 0usize;
            let mut seen = std::collections::BTreeSet::new();
            for file in &diff.files {
                let trimmed = file.key.trim();
                let has_left = file.left_path.is_some();
                let has_right = file.right_path.is_some();
                let size_differs = file.left_size_bytes != file.right_size_bytes;
                let impossible_unchanged =
                    !file.changed && (size_differs || !has_left || !has_right);
                let duplicate = !trimmed.is_empty() && !seen.insert(trimmed.to_string());
                if trimmed.is_empty()
                    || (!has_left && !has_right)
                    || impossible_unchanged
                    || duplicate
                {
                    invalid += 1;
                }
            }
            let evidence_count = diff.files.len()
                + usize::from(diff.report.is_some())
                + usize::from(diff.trace.is_some());
            (
                if evidence_count > 0 && left_ok && right_ok && invalid == 0 {
                    FullStepStatus::Passed
                } else {
                    FullStepStatus::Failed
                },
                format!(
                    "left={} left_ok={} right={} right_ok={} file_deltas={} report={} trace={} invalid={}",
                    diff.left,
                    left_ok,
                    diff.right,
                    right_ok,
                    diff.files.len(),
                    diff.report.is_some(),
                    diff.trace.is_some(),
                    invalid
                ),
            )
        }
        _ => (
            FullStepStatus::Failed,
            "unexpected artifacts diff payload".to_string(),
        ),
    }
}

fn env_step_status(env: &fozzy::EnvInfo) -> (FullStepStatus, String) {
    let proc_backend = env
        .capabilities
        .get("proc")
        .map(|c| c.backend.as_str())
        .unwrap_or("unknown");
    let fs_backend = env
        .capabilities
        .get("fs")
        .map(|c| c.backend.as_str())
        .unwrap_or("unknown");
    let http_backend = env
        .capabilities
        .get("http")
        .map(|c| c.backend.as_str())
        .unwrap_or("unknown");
    let known_proc = matches!(proc_backend, "scripted" | "host");
    let known_fs = matches!(fs_backend, "virtual_overlay" | "host");
    let known_http = matches!(http_backend, "scripted" | "host");
    let ok = known_proc && known_fs && known_http;
    (
        if ok {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "proc={} known_proc={} fs={} known_fs={} http={} known_http={}",
            proc_backend, known_proc, fs_backend, known_fs, http_backend, known_http
        ),
    )
}

fn ci_report_status(report: &fozzy::CiReport) -> (FullStepStatus, String) {
    let check_count = report.checks.len();
    let mut seen = std::collections::BTreeSet::new();
    let invalid = report
        .checks
        .iter()
        .filter(|check| {
            let name = check.name.trim();
            name.is_empty() || !known_ci_check_name(name)
        })
        .count();
    let duplicate = report
        .checks
        .iter()
        .filter(|check| {
            let key = check.name.trim();
            !key.is_empty() && !seen.insert(key.to_string())
        })
        .count();
    let failing = report
        .checks
        .iter()
        .filter(|check| !check.ok)
        .map(|check| match check.detail.as_deref() {
            Some(detail) if !detail.is_empty() => format!("{}: {}", check.name, detail),
            _ => check.name.clone(),
        })
        .collect::<Vec<_>>();
    let derived_ok = check_count > 0 && failing.is_empty() && invalid == 0 && duplicate == 0;
    let detail = if failing.is_empty() {
        format!(
            "checks={} failed=<none> invalid={} duplicate={} reported_ok={} derived_ok={}",
            check_count, invalid, duplicate, report.ok, derived_ok
        )
    } else {
        format!(
            "checks={} failed={} invalid={} duplicate={} reported_ok={} derived_ok={}",
            check_count,
            failing.join("; "),
            invalid,
            duplicate,
            report.ok,
            derived_ok
        )
    };
    (
        if report.ok == derived_ok && derived_ok {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        detail,
    )
}

fn doctor_report_status(
    report: &fozzy::DoctorReport,
    strict: bool,
    scenario: &Path,
    runs: u32,
    expected_seed: u64,
) -> (FullStepStatus, String) {
    let expected_scenario = scenario.display().to_string();
    let mut seen_issues = std::collections::BTreeSet::new();
    let invalid_issues = report
        .issues
        .iter()
        .filter(|issue| {
            let code = issue.code.trim();
            code.is_empty() || !known_doctor_issue_code(code) || issue.message.trim().is_empty()
        })
        .count();
    let duplicate_issues = report
        .issues
        .iter()
        .filter(|issue| {
            let code = issue.code.trim();
            let message = issue.message.trim();
            !code.is_empty()
                && !message.is_empty()
                && !seen_issues.insert(format!("{code}\u{0}{message}"))
        })
        .count();
    let mismatch_issue_present = report
        .issues
        .iter()
        .any(|issue| issue.code.trim() == "determinism_audit_mismatch");
    let signal_count = report
        .nondeterminism_signals
        .as_ref()
        .map(|signals| signals.len())
        .unwrap_or(0);
    let mut seen_signals = std::collections::BTreeSet::new();
    let invalid_signals = report
        .nondeterminism_signals
        .as_ref()
        .map(|signals| {
            signals
                .iter()
                .filter(|signal| {
                    let source = signal.source.trim();
                    source.is_empty()
                        || !known_doctor_signal_source(source)
                        || signal.detail.trim().is_empty()
                })
                .count()
        })
        .unwrap_or(0);
    let duplicate_signals = report
        .nondeterminism_signals
        .as_ref()
        .map(|signals| {
            signals
                .iter()
                .filter(|signal| {
                    let source = signal.source.trim();
                    let detail = signal.detail.trim();
                    !source.is_empty()
                        && !detail.is_empty()
                        && !seen_signals.insert(format!("{source}\u{0}{detail}"))
                })
                .count()
        })
        .unwrap_or(0);
    let audit_present = report.determinism_audit.is_some();
    let audit_valid = report.determinism_audit.as_ref().is_some_and(|audit| {
        audit.scenario == expected_scenario
            && audit.runs == runs
            && audit.seed == expected_seed
            && audit.signatures.len() == audit.runs as usize
            && audit
                .signatures
                .iter()
                .all(|signature| !signature.trim().is_empty())
            && if audit.consistent {
                audit.first_mismatch_run.is_none()
            } else {
                audit
                    .first_mismatch_run
                    .is_some_and(|run| run >= 2 && run <= audit.runs)
            }
    });
    let audit_issue_consistent = report.determinism_audit.as_ref().is_some_and(|audit| {
        if audit.consistent {
            !mismatch_issue_present
        } else {
            mismatch_issue_present
        }
    });
    let derived_ok = runs > 0
        && audit_present
        && audit_valid
        && audit_issue_consistent
        && report.issues.is_empty()
        && invalid_issues == 0
        && duplicate_issues == 0
        && invalid_signals == 0
        && duplicate_signals == 0;
    let policy_ok =
        !strict || (report.issues.is_empty() && signal_count == 0 && invalid_signals == 0);
    let failing = report
        .issues
        .iter()
        .map(|issue| match issue.hint.as_deref() {
            Some(hint) if !hint.is_empty() => format!("{}: {} ({hint})", issue.code, issue.message),
            _ => format!("{}: {}", issue.code, issue.message),
        })
        .chain(
            report
                .nondeterminism_signals
                .as_ref()
                .into_iter()
                .flatten()
                .map(|signal| format!("signal {}: {}", signal.source, signal.detail)),
        )
        .collect::<Vec<_>>();
    let detail = if failing.is_empty() {
        format!(
            "issues=0 signals=0 invalid_issues={} duplicate_issues={} invalid_signals={} duplicate_signals={} audit_present={} audit_valid={} audit_issue_consistent={} runs={} seed={} scenario={} failed=<none> reported_ok={} derived_ok={} strict_policy_ok={}",
            invalid_issues,
            duplicate_issues,
            invalid_signals,
            duplicate_signals,
            audit_present,
            audit_valid,
            audit_issue_consistent,
            runs,
            expected_seed,
            expected_scenario,
            report.ok,
            derived_ok,
            policy_ok
        )
    } else {
        format!(
            "issues={} signals={} invalid_issues={} duplicate_issues={} invalid_signals={} duplicate_signals={} audit_present={} audit_valid={} audit_issue_consistent={} runs={} seed={} scenario={} failed={} reported_ok={} derived_ok={} strict_policy_ok={}",
            report.issues.len(),
            signal_count,
            invalid_issues,
            duplicate_issues,
            invalid_signals,
            duplicate_signals,
            audit_present,
            audit_valid,
            audit_issue_consistent,
            runs,
            expected_seed,
            expected_scenario,
            failing.join("; "),
            report.ok,
            derived_ok,
            policy_ok
        )
    };
    (
        if report.ok == derived_ok && derived_ok && policy_ok {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        detail,
    )
}

fn topology_coverage_status(
    report: &fozzy::MapSuitesReport,
    expected_root: &Path,
    expected_scenario_root: &Path,
    expected_profile: TopologyProfile,
    expected_shrink_policy: ShrinkCoveragePolicy,
    expected_base_min_risk: u8,
) -> (FullStepStatus, String) {
    fn known_topology_suite(name: &str) -> bool {
        matches!(
            name,
            "test_det"
                | "run_record_replay_ci"
                | "fuzz_inputs"
                | "explore_schedule_faults"
                | "host_backends_run"
                | "memory_graph_diff_top"
                | "shrink_exercised"
                | "shrink_failure_trace"
        )
    }

    let warnings = if report.warnings.is_empty() {
        "<none>".to_string()
    } else {
        report.warnings.join("; ")
    };
    let root_ok = report.root == expected_root.display().to_string();
    let scenario_root_ok = report.scenario_root == expected_scenario_root.display().to_string();
    let profile_ok = report.profile == expected_profile;
    let shrink_policy_ok = report.shrink_policy == expected_shrink_policy;
    let base_min_risk_ok = report.base_min_risk == expected_base_min_risk;
    let hotspot_math_ok = report.covered_hotspot_count <= report.required_hotspot_count
        && report.uncovered_hotspot_count
            == report
                .required_hotspot_count
                .saturating_sub(report.covered_hotspot_count);
    let pagination_math_ok = report.returned_suites == report.suites.len()
        && report.returned_suites <= report.total_suites
        && report.returned_suites <= report.limit
        && if report.truncated {
            report.offset.saturating_add(report.returned_suites) < report.total_suites
        } else {
            report.offset.saturating_add(report.returned_suites) >= report.total_suites
        };
    let mut seen_hotspots = std::collections::BTreeSet::new();
    let invalid_suites = report
        .suites
        .iter()
        .filter(|suite| {
            let mut seen_coverage_evidence = std::collections::BTreeSet::new();
            let mut evidence_suite_set = std::collections::BTreeSet::new();
            let invalid_coverage_evidence = suite.coverage_evidence.iter().any(|evidence| {
                let suite_name = evidence.suite.trim();
                let reason = evidence.reason.trim();
                let mut seen_matched_scenarios = std::collections::BTreeSet::new();
                let invalid_matched_scenarios = evidence.matched_scenarios.iter().any(|scenario| {
                    let scenario = scenario.trim();
                    scenario.is_empty() || !seen_matched_scenarios.insert(scenario.to_string())
                });
                if !suite_name.is_empty() {
                    evidence_suite_set.insert(suite_name.to_string());
                }
                let duplicate_coverage_evidence = !suite_name.is_empty()
                    && !reason.is_empty()
                    && !invalid_matched_scenarios
                    && !evidence.matched_scenarios.is_empty()
                    && !seen_coverage_evidence.insert(format!(
                        "{suite_name}\u{0}{reason}\u{0}{}",
                        evidence
                            .matched_scenarios
                            .iter()
                            .map(|scenario| scenario.trim())
                            .collect::<Vec<_>>()
                            .join("\u{0}")
                    ));
                suite_name.is_empty()
                    || !known_topology_suite(suite_name)
                    || reason.is_empty()
                    || evidence.matched_scenarios.is_empty()
                    || invalid_matched_scenarios
                    || duplicate_coverage_evidence
            });
            let required_set = suite
                .required_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .collect::<std::collections::BTreeSet<_>>();
            let required_duplicates = suite
                .required_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .count()
                != required_set.len();
            let covered_set = suite
                .covered_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .collect::<std::collections::BTreeSet<_>>();
            let covered_duplicates = suite
                .covered_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .count()
                != covered_set.len();
            let missing_set = suite
                .missing_required_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .collect::<std::collections::BTreeSet<_>>();
            let missing_duplicates = suite
                .missing_required_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .count()
                != missing_set.len();
            let recommended_set = suite
                .recommended_suites
                .iter()
                .map(|suite| suite.trim())
                .filter(|suite| !suite.is_empty())
                .collect::<std::collections::BTreeSet<_>>();
            let suite_math_invalid = suite.required_suites.iter().any(|suite| {
                let suite = suite.trim();
                suite.is_empty() || !known_topology_suite(suite)
            }) || suite.covered_suites.iter().any(|suite| {
                let suite = suite.trim();
                suite.is_empty() || !known_topology_suite(suite)
            }) || suite.missing_required_suites.iter().any(|suite| {
                let suite = suite.trim();
                suite.is_empty() || !known_topology_suite(suite)
            }) || required_duplicates
                || covered_duplicates
                || missing_duplicates
                || suite.recommended_suites.iter().any(|suite| {
                    let suite = suite.trim();
                    suite.is_empty() || !known_topology_suite(suite)
                })
                || suite
                    .coverage_hints
                    .iter()
                    .any(|hint| hint.trim().is_empty())
                || suite
                    .coverage_hints
                    .iter()
                    .map(|hint| hint.trim())
                    .filter(|hint| !hint.is_empty())
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != suite
                        .coverage_hints
                        .iter()
                        .map(|hint| hint.trim())
                        .filter(|hint| !hint.is_empty())
                        .count()
                || recommended_set.len() != suite.recommended_suites.len()
                || !covered_set.is_subset(&required_set)
                || !missing_set.is_subset(&required_set)
                || !covered_set.is_disjoint(&missing_set)
                || !required_set.is_subset(&recommended_set)
                || covered_set
                    != evidence_suite_set
                        .iter()
                        .map(|suite| suite.as_str())
                        .collect::<std::collections::BTreeSet<_>>()
                || required_set
                    != covered_set
                        .union(&missing_set)
                        .copied()
                        .collect::<std::collections::BTreeSet<_>>()
                || suite.covered != (!suite.required_by_policy || missing_set.is_empty());
            suite.hotspot_id.trim().is_empty()
                || suite.component.trim().is_empty()
                || suite.path.trim().is_empty()
                || invalid_coverage_evidence
                || suite_math_invalid
                || !seen_hotspots.insert(suite.hotspot_id.trim().to_string())
        })
        .count();
    let ok = report.uncovered_hotspot_count == 0
        && report.required_hotspot_count > 0
        && report.warnings.is_empty()
        && root_ok
        && scenario_root_ok
        && profile_ok
        && shrink_policy_ok
        && base_min_risk_ok
        && hotspot_math_ok
        && pagination_math_ok
        && report.returned_suites > 0
        && invalid_suites == 0;
    (
        if ok {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "required_hotspots={} covered={} uncovered={} root_ok={} scenario_root_ok={} profile_ok={} shrink_policy_ok={} base_min_risk_ok={} hotspot_math_ok={} total_suites={} returned_suites={} offset={} limit={} truncated={} pagination_math_ok={} invalid_suites={} min_risk={} profile={} root={} scenario_root={} warnings={}",
            report.required_hotspot_count,
            report.covered_hotspot_count,
            report.uncovered_hotspot_count,
            root_ok,
            scenario_root_ok,
            profile_ok,
            shrink_policy_ok,
            base_min_risk_ok,
            hotspot_math_ok,
            report.total_suites,
            report.returned_suites,
            report.offset,
            report.limit,
            report.truncated,
            pagination_math_ok,
            invalid_suites,
            report.effective_min_risk,
            format!("{:?}", report.profile).to_lowercase(),
            report.root,
            report.scenario_root,
            warnings
        ),
    )
}

mod runner;
pub(crate) use runner::{
    run_full_command, run_gate_command, selected_init_test_types, shrink_status_matches,
};

#[cfg(test)]
mod tests;
