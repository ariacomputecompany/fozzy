//! Fozzy CLI entrypoint.

mod cli_dispatch;
mod cli_logger;
mod cli_runtime;
mod cli_workflows;

use std::process::ExitCode;

use clap::error::ErrorKind;
use cli_logger::CliLogger;
use cli_runtime::{
    args_request_json, init_tracing, print_clap_error_and_exit, print_error_and_exit,
};
use tracing_subscriber::EnvFilter;

include!("cli_args.rs");

pub(crate) use cli_runtime::{
    enforce_strict_run, enforce_strict_summary, exit_code_for_status, resolve_memory_options,
    strict_enabled,
};
pub(crate) use fozzy::*;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let json_requested = args_request_json(&args);
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => return print_clap_error_and_exit(json_requested, err),
    };
    let logger = CliLogger::new(cli.json, cli.no_color);

    if let Err(err) = init_tracing(&cli.log) {
        // Tracing is best-effort; if it fails, we still continue.
        logger.print_warning(&format!("failed to init tracing: {err:#}"));
    }

    let cwd = cli
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    if let Err(err) = std::env::set_current_dir(&cwd) {
        return print_error_and_exit(
            &logger,
            anyhow::anyhow!(err).context(format!("failed to set cwd to {}", cwd.display())),
        );
    }

    let config = match Config::load_optional_checked(&cli.config) {
        Ok(cfg) => cfg,
        Err(err) => return print_error_and_exit(&logger, anyhow::anyhow!("{err}")),
    };

    match cli_dispatch::run_command(&cli, &config, &logger) {
        Ok(code) => code,
        Err(err) => print_error_and_exit(&logger, err),
    }
}

fn selected_init_test_types(with: &[InitTestType], all_tests: bool) -> Vec<InitTestType> {
    cli_workflows::selected_init_test_types(with, all_tests)
}
