use super::*;

pub(super) fn args_request_json(args: &[String]) -> bool {
    args.iter().any(|a| a == "--json" || a == "--json=true")
}

pub(super) fn init_tracing(level: &str) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
    Ok(())
}

pub(super) fn enforce_strict_run(cli: &Cli, summary: &RunSummary) -> anyhow::Result<()> {
    enforce_strict_summary(strict_enabled(cli), summary)
}

pub(super) fn enforce_strict_summary(strict: bool, summary: &RunSummary) -> anyhow::Result<()> {
    if !strict {
        return Ok(());
    }

    let warnings: Vec<&str> = fozzy::pass_checker_warnings(summary)
        .into_iter()
        .map(|f| f.message.as_str())
        .collect();
    if warnings.is_empty() {
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "strict mode: run contains warning findings: {}",
        warnings.join("; ")
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_memory_options(
    config: &Config,
    mem_track: bool,
    mem_artifacts: bool,
    mem_limit_mb: Option<u64>,
    mem_fail_after: Option<u64>,
    mem_fragmentation_seed: Option<u64>,
    mem_pressure_wave: Option<String>,
    fail_on_leak: bool,
    leak_budget: Option<u64>,
) -> MemoryOptions {
    let track = mem_track
        || mem_artifacts
        || mem_limit_mb.is_some()
        || mem_fail_after.is_some()
        || mem_fragmentation_seed.is_some()
        || mem_pressure_wave.is_some()
        || fail_on_leak
        || leak_budget.is_some()
        || config.mem_track
        || config.mem_artifacts
        || config.mem_limit_mb.is_some()
        || config.mem_fail_after.is_some()
        || config.mem_fragmentation_seed.is_some()
        || config.mem_pressure_wave.is_some()
        || config.fail_on_leak
        || config.leak_budget.is_some();
    MemoryOptions {
        track,
        limit_mb: mem_limit_mb.or(config.mem_limit_mb),
        fail_after_allocs: mem_fail_after.or(config.mem_fail_after),
        fragmentation_seed: mem_fragmentation_seed.or(config.mem_fragmentation_seed),
        pressure_wave: mem_pressure_wave.or_else(|| config.mem_pressure_wave.clone()),
        fail_on_leak: fail_on_leak || config.fail_on_leak,
        leak_budget_bytes: leak_budget.or(config.leak_budget),
        artifacts: mem_artifacts || config.mem_artifacts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_memory_options_enables_tracking_for_memory_policy() {
        let opts = resolve_memory_options(
            &Config::default(),
            false,
            false,
            None,
            None,
            None,
            None,
            false,
            Some(256),
        );
        assert!(
            opts.track,
            "leak-budget policy should not be silently ignored"
        );
    }
}

pub(super) fn strict_enabled(cli: &Cli) -> bool {
    cli.strict || !cli.unsafe_mode
}

pub(super) fn print_error_and_exit(logger: &CliLogger, err: anyhow::Error) -> ExitCode {
    let msg = format!("{err:#}");
    logger.print_error(&msg);
    ExitCode::from(2)
}

pub(super) fn print_clap_error_and_exit(json: bool, err: clap::Error) -> ExitCode {
    let kind = err.kind();
    let code = err.exit_code();
    if matches!(kind, ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        let _ = err.print();
        return ExitCode::from(code as u8);
    }
    if json {
        let out = serde_json::json!({
            "code": "error",
            "message": err.to_string().trim_end(),
        });
        match serde_json::to_string_pretty(&out) {
            Ok(s) => println!("{s}"),
            Err(_) => println!("{out}"),
        }
    } else {
        let _ = err.print();
    }
    ExitCode::from(code as u8)
}

pub(super) fn exit_code_for_status(status: ExitStatus) -> ExitCode {
    match status {
        ExitStatus::Pass => ExitCode::SUCCESS,
        ExitStatus::Fail => ExitCode::from(1),
        ExitStatus::Timeout => ExitCode::from(3),
        ExitStatus::Crash => ExitCode::from(4),
        ExitStatus::Error => ExitCode::from(2),
    }
}
