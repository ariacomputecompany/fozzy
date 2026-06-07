use super::*;

pub(crate) fn discover_scenarios(root: &Path) -> FullScenarioDiscovery {
    let mut out = FullScenarioDiscovery {
        steps: Vec::new(),
        distributed: Vec::new(),
        parse_errors: Vec::new(),
    };
    if !root.exists() {
        return out;
    }
    for entry in WalkDir::new(root).into_iter().flatten() {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".fozzy.json") {
            continue;
        }
        let bytes = match std::fs::read(path) {
            Ok(v) => v,
            Err(err) => {
                out.parse_errors
                    .push(format!("{}: {}", path.display(), err));
                continue;
            }
        };
        match serde_json::from_slice::<fozzy::ScenarioFile>(&bytes) {
            Ok(fozzy::ScenarioFile::Steps(_)) => out.steps.push(path.to_path_buf()),
            Ok(fozzy::ScenarioFile::Distributed(_)) => out.distributed.push(path.to_path_buf()),
            Ok(fozzy::ScenarioFile::Suites(_)) => out.parse_errors.push(format!(
                "{}: suites format is not executable",
                path.display()
            )),
            Err(err) => out.parse_errors.push(format!("{}: {err}", path.display())),
        }
    }
    out.steps.sort();
    out.distributed.sort();
    out
}

pub(crate) fn git_clean_tree_check() -> anyhow::Result<String> {
    let out = ProcessCommand::new("git")
        .args(["status", "--porcelain"])
        .output()
        .map_err(|err| anyhow::anyhow!("failed to execute git status --porcelain: {err}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stderr_lower = stderr.to_ascii_lowercase();
        if stderr_lower.contains("not a git repository") {
            return Ok("git worktree check skipped: not a git repository".to_string());
        }
        return Err(anyhow::anyhow!(
            "git status --porcelain failed; verify this is a git worktree{}{}",
            if stderr.is_empty() { "" } else { ": " },
            stderr
        ));
    }
    let body = String::from_utf8_lossy(&out.stdout);
    let dirty: Vec<&str> = body.lines().collect();
    if dirty.is_empty() {
        return Ok("git worktree clean".to_string());
    }
    let preview = dirty
        .iter()
        .take(3)
        .copied()
        .collect::<Vec<_>>()
        .join(" | ");
    Err(anyhow::anyhow!(
        "git worktree is not clean ({} change(s)); example: {}",
        dirty.len(),
        preview
    ))
}

pub(crate) fn selected_init_test_types(
    with: &[InitTestType],
    all_tests: bool,
) -> Vec<InitTestType> {
    if all_tests || with.is_empty() {
        return vec![InitTestType::All];
    }
    let mut out = with.to_vec();
    if out.contains(&InitTestType::All) {
        return vec![InitTestType::All];
    }
    out.sort_by_key(|v| match v {
        InitTestType::Run => 0,
        InitTestType::Fuzz => 1,
        InitTestType::Explore => 2,
        InitTestType::Memory => 3,
        InitTestType::Host => 4,
        InitTestType::All => 5,
    });
    out.dedup();
    out
}

pub(crate) fn apply_full_policy_filters(
    steps: &mut [FullStepResult],
    skip_steps: &[String],
    required_steps: &[String],
) {
    use std::collections::BTreeSet;
    let skip: BTreeSet<String> = skip_steps
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let required: BTreeSet<String> = required_steps
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();

    for step in steps {
        let key = step.name.to_ascii_lowercase();
        if key == "policy_conflict" {
            continue;
        }
        if !required.is_empty() && !required.contains(&key) {
            step.status = FullStepStatus::Skipped;
            step.detail = format!("skipped by required-steps policy; {}", step.detail);
            continue;
        }
        if skip.contains(&key) {
            step.status = FullStepStatus::Skipped;
            step.detail = format!("skipped by skip-steps policy; {}", step.detail);
        }
    }
}

pub(crate) fn full_policy_conflict_details(
    skip_steps: &[String],
    required_steps: &[String],
    topology_required: bool,
) -> Option<String> {
    use std::collections::BTreeSet;
    if !topology_required {
        return None;
    }
    let req: BTreeSet<String> = required_steps
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    if !req.is_empty() && !req.contains("topology_coverage") {
        return Some(
            "--require-topology-coverage was set, but --required-steps excludes topology_coverage; refusing implicit policy neutralization"
                .to_string(),
        );
    }
    let skip: BTreeSet<String> = skip_steps
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    if skip.contains("topology_coverage") {
        return Some(
            "--require-topology-coverage conflicts with --skip-steps topology_coverage; remove one policy flag"
                .to_string(),
        );
    }
    None
}

pub(crate) fn shrink_status_matches(target: ExitStatus, candidate: ExitStatus) -> bool {
    if target == ExitStatus::Pass {
        candidate == ExitStatus::Pass
    } else {
        candidate != ExitStatus::Pass
    }
}

pub(crate) fn is_negative_fixture_scenario(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["fail", "leak", "panic", "timeout", "checkers", "assertions"]
        .iter()
        .any(|tok| name.contains(tok))
}

pub(crate) fn is_preferred_step_scenario(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.contains("pass") || name.contains("example")
}

pub(crate) fn is_preferred_host_step_scenario(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.contains("host")
}

pub(crate) fn is_preferred_distributed_scenario(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    !name.contains("checkers")
}

pub(crate) fn host_backed_trace_status(path: &Path) -> (FullStepStatus, String) {
    let trace = match fozzy::TraceFile::read_json(path) {
        Ok(trace) => trace,
        Err(err) => return (FullStepStatus::Failed, err.to_string()),
    };
    let used_host_proc = trace.events.iter().any(|event| {
        event.name == "proc_spawn"
            && event
                .fields
                .get("backend")
                .and_then(|value| value.as_str())
                .is_some_and(|backend| backend == "host")
    });
    let used_host_fs = trace.events.iter().any(|event| {
        matches!(
            event.name.as_str(),
            "capability_fs" | "fs_write" | "fs_read_assert" | "fs_snapshot" | "fs_restore"
        ) && event
            .fields
            .get("backend")
            .and_then(|value| value.as_str())
            .is_some_and(|backend| backend == "host")
    });
    let used_host_http = trace.events.iter().any(|event| {
        event.name == "http_request"
            && event
                .fields
                .get("backend")
                .and_then(|value| value.as_str())
                .is_some_and(|backend| backend == "host")
    });
    let exercised = used_host_proc || used_host_fs || used_host_http;
    (
        if exercised {
            FullStepStatus::Passed
        } else {
            FullStepStatus::Failed
        },
        format!(
            "host_proc={} host_fs={} host_http={}",
            used_host_proc, used_host_fs, used_host_http
        ),
    )
}
