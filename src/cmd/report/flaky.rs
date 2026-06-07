use super::*;

pub(super) fn flaky_command(
    config: &Config,
    runs: &[String],
    flake_budget: Option<FlakeBudget>,
) -> FozzyResult<serde_json::Value> {
    if runs.len() < 2 {
        return Err(FozzyError::Report(
            "flaky analysis requires at least two runs/traces".to_string(),
        ));
    }

    let mut status_counts = std::collections::BTreeMap::<String, usize>::new();
    let mut finding_sets = std::collections::BTreeSet::<Vec<String>>::new();
    let mut signatures = std::collections::BTreeMap::<String, usize>::new();
    let mut seen_run_ids = std::collections::BTreeSet::<String>::new();

    for run in runs {
        let summary = load_summary(config, run)?;
        if !seen_run_ids.insert(summary.identity.run_id.clone()) {
            return Err(FozzyError::Report(format!(
                "duplicate run reference detected for runId={} (duplicates are not allowed in flaky analysis)",
                summary.identity.run_id
            )));
        }
        let status_key = format!("{:?}", summary.status).to_lowercase();
        *status_counts.entry(status_key.clone()).or_insert(0) += 1;

        let mut titles: Vec<String> = summary.findings.iter().map(|f| f.title.clone()).collect();
        titles.sort();
        titles.dedup();
        finding_sets.insert(titles);
        let sig = format!(
            "{status_key}|{}",
            summary
                .findings
                .iter()
                .map(|f| f.title.as_str())
                .collect::<Vec<_>>()
                .join("|")
        );
        *signatures.entry(sig).or_insert(0) += 1;
    }

    let is_flaky = status_counts.len() > 1 || finding_sets.len() > 1;
    let dominant = signatures.values().copied().max().unwrap_or(0) as f64;
    let total = runs.len() as f64;
    let flake_rate_pct = if total == 0.0 {
        0.0
    } else {
        ((total - dominant) / total) * 100.0
    };
    if let Some(budget) = flake_budget
        && flake_rate_pct > budget.pct()
    {
        return Err(FozzyError::Report(format!(
            "flake budget exceeded: {:.2}% > {:.2}%",
            flake_rate_pct,
            budget.pct()
        )));
    }
    let out = FlakyReport {
        schema_version: "fozzy.flaky_report.v1".to_string(),
        run_count: runs.len(),
        status_counts,
        finding_title_sets: finding_sets.into_iter().collect(),
        is_flaky,
        flake_rate_pct,
    };
    Ok(serde_json::to_value(out)?)
}
