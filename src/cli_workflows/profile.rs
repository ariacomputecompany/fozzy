use super::*;

pub(crate) fn summarize_profile_top(value: &serde_json::Value) -> String {
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

pub(crate) fn known_profile_domain(domain: &str) -> bool {
    matches!(domain, "cpu" | "heap" | "latency" | "io" | "sched")
}

pub(crate) fn known_ci_check_name(name: &str) -> bool {
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

pub(crate) fn known_doctor_issue_code(code: &str) -> bool {
    matches!(
        code,
        "proc_unmatched_preflight" | "determinism_audit_mismatch"
    )
}

pub(crate) fn known_doctor_signal_source(source: &str) -> bool {
    matches!(source, "env")
}

pub(crate) fn known_profile_time_domain(time_domain: &str) -> bool {
    matches!(time_domain, "host_monotonic_time" | "virtual_time")
}

pub(crate) fn known_profile_metric(domain: &str, metric: &str) -> bool {
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

pub(crate) fn valid_profile_explain_shifted_path(domain: &str, shifted_path: &str) -> bool {
    shifted_path
        .strip_prefix("metric::")
        .is_some_and(|metric| known_profile_metric(domain, metric))
}

pub(crate) fn known_profile_evidence_pointer(pointer: &str) -> bool {
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

pub(crate) fn expected_profile_time_domain(metric: &str) -> &'static str {
    if metric == "cpu_time_ms" {
        "host_monotonic_time"
    } else {
        "virtual_time"
    }
}

pub(crate) fn profile_top_status(value: &serde_json::Value) -> (FullStepStatus, String) {
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
            value
                .get(*domain)
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

pub(crate) fn profile_diff_status(
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

pub(crate) fn profile_explain_status(value: &serde_json::Value) -> (FullStepStatus, String) {
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
