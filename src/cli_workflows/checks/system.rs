use super::*;

pub(crate) fn env_step_status(env: &fozzy::EnvInfo) -> (FullStepStatus, String) {
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

pub(crate) fn ci_report_status(report: &fozzy::CiReport) -> (FullStepStatus, String) {
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

pub(crate) fn doctor_report_status(
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
