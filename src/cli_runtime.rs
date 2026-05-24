use super::*;

pub(super) fn args_request_json(args: &[String]) -> bool {
    args.iter().any(|a| a == "--json" || a == "--json=true")
}

pub(super) fn normalize_global_args(args: impl IntoIterator<Item = String>) -> Vec<String> {
    let all: Vec<String> = args.into_iter().collect();
    if all.is_empty() {
        return all;
    }

    let mut globals = Vec::new();
    let mut rest = Vec::new();

    let mut i = 1usize;
    while i < all.len() {
        let arg = &all[i];
        match arg.as_str() {
            "--json" | "--no-color" | "--strict" | "--unsafe" => {
                globals.push(arg.clone());
                i += 1;
            }
            "--config" | "--cwd" | "--log" | "--proc-backend" | "--fs-backend"
            | "--http-backend" => {
                globals.push(arg.clone());
                if i + 1 < all.len() {
                    globals.push(all[i + 1].clone());
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ if arg.starts_with("--config=")
                || arg.starts_with("--cwd=")
                || arg.starts_with("--log=")
                || arg.starts_with("--proc-backend=")
                || arg.starts_with("--fs-backend=")
                || arg.starts_with("--http-backend=")
                || arg.starts_with("--strict=")
                || arg.starts_with("--unsafe=") =>
            {
                globals.push(arg.clone());
                i += 1;
            }
            _ => {
                rest.push(arg.clone());
                i += 1;
            }
        }
    }

    let mut normalized = Vec::with_capacity(all.len());
    normalized.push(all[0].clone());
    normalized.extend(globals);
    normalized.extend(rest);
    normalized
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

    let warnings: Vec<&str> = summary
        .findings
        .iter()
        .filter(|f| {
            f.kind == fozzy::FindingKind::Checker && summary.status == fozzy::ExitStatus::Pass
        })
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
    MemoryOptions {
        track: mem_track || config.mem_track,
        limit_mb: mem_limit_mb.or(config.mem_limit_mb),
        fail_after_allocs: mem_fail_after.or(config.mem_fail_after),
        fragmentation_seed: mem_fragmentation_seed.or(config.mem_fragmentation_seed),
        pressure_wave: mem_pressure_wave.or_else(|| config.mem_pressure_wave.clone()),
        fail_on_leak: fail_on_leak || config.fail_on_leak,
        leak_budget_bytes: leak_budget.or(config.leak_budget),
        artifacts: mem_artifacts || config.mem_artifacts,
    }
}

pub(super) fn strict_enabled(cli: &Cli) -> bool {
    cli.strict && !cli.unsafe_mode
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
