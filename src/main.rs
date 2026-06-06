//! Fozzy CLI entrypoint.

mod cli_dispatch;
mod cli_logger;
mod cli_runtime;
mod cli_workflows;

use clap::{Parser, Subcommand, error::ErrorKind};
use tracing_subscriber::EnvFilter;

use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};
use walkdir::WalkDir;

use cli_logger::CliLogger;
use cli_runtime::{
    args_request_json, enforce_strict_run, enforce_strict_summary, exit_code_for_status,
    init_tracing, normalize_global_args, print_clap_error_and_exit, print_error_and_exit,
    resolve_memory_options, strict_enabled,
};
use fozzy::{
    ArtifactCommand, CiOptions, Config, CorpusCommand, ExitStatus, ExploreOptions, FlakeBudget,
    FozzyDuration, FsBackend, FuzzMode, FuzzOptions, FuzzTarget, HttpBackend, InitTemplate,
    InitTestType, MapCommand, MapSuitesOptions, MemoryCommand, MemoryOptions, ProcBackend,
    ProfileCaptureLevel, ProfileCommand, ProfileExportFormat, RecordCollisionPolicy, ReportCommand,
    Reporter, RunOptions, RunSummary, ScenarioPath, ScheduleStrategy, ShrinkCoveragePolicy,
    ShrinkMinimize, TopologyProfile, TracePath,
};

#[derive(Debug, Parser)]
#[command(name = "fozzy")]
#[command(about = "deterministic full-stack testing + fuzzing + distributed exploration")]
#[command(
    after_help = "Start with `fozzy map suites --root . --scenario-root tests --profile pedantic --json` and follow suite gaps in full. Execution policy: use the full command surface by default (map/run/test/fuzz/explore/replay/shrink/trace verify/ci/report/artifacts/profile/memory/doctor/corpus/env/version/usage). Use `fozzy full` to run the end-to-end gate automatically; use `--unsafe` only when intentionally relaxing checks."
)]
struct Cli {
    /// Path to config file. Missing configs are treated as "defaults".
    #[arg(long, global = true, default_value = "fozzy.toml")]
    config: PathBuf,

    /// Working directory for execution.
    #[arg(long, global = true)]
    cwd: Option<PathBuf>,

    /// Log level.
    #[arg(long, global = true, default_value = "info")]
    log: String,

    /// Machine-readable output to stdout (JSON).
    #[arg(long, global = true)]
    json: bool,

    /// Disable color output.
    #[arg(long, global = true)]
    no_color: bool,

    /// Treat warning-like conditions as errors (non-zero exit). Enabled by default.
    #[arg(long, global = true, default_value_t = true)]
    strict: bool,

    /// Opt out of strict mode and allow potentially unsafe relaxed checks.
    #[arg(long = "unsafe", global = true)]
    unsafe_mode: bool,

    /// Proc backend for proc_spawn steps.
    #[arg(long, global = true)]
    proc_backend: Option<ProcBackend>,

    /// Filesystem backend for fs_* steps.
    #[arg(long, global = true)]
    fs_backend: Option<FsBackend>,

    /// HTTP backend for http_* steps.
    #[arg(long, global = true)]
    http_backend: Option<HttpBackend>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize a fozzy project (config + scaffolding)
    Init {
        #[arg(long)]
        force: bool,

        #[arg(long)]
        template: Option<InitTemplate>,

        /// Scaffold test types (`run`, `fuzz`, `explore`, `memory`, `host`, `all`).
        #[arg(long = "with", value_enum, value_delimiter = ',')]
        with: Vec<InitTestType>,

        /// Explicitly scaffold all available test types.
        #[arg(long)]
        all_tests: bool,
    },

    /// Run regular tests (optionally deterministic)
    Test {
        /// Glob patterns for scenario files.
        globs: Vec<String>,

        /// Enable deterministic runtime (seeded RNG + virtual time + decision logging).
        #[arg(long)]
        det: bool,

        /// Seed to use in deterministic mode (or to make nondet runs reproducible).
        #[arg(long)]
        seed: Option<u64>,

        /// Number of parallel jobs (best-effort; determinism is preserved within a run).
        #[arg(long)]
        jobs: Option<usize>,

        /// Per-test timeout.
        #[arg(long)]
        timeout: Option<FozzyDuration>,

        /// Name filter expression (substring match; v1).
        #[arg(long)]
        filter: Option<String>,

        /// Reporter format.
        #[arg(long, default_value = "pretty")]
        reporter: Reporter,

        /// Record trace (.fozzy) to path.
        #[arg(long)]
        record: Option<PathBuf>,

        /// Behavior when --record target exists: error, overwrite, or append with numeric suffix.
        #[arg(long, default_value = "append")]
        record_collision: RecordCollisionPolicy,

        /// Stop on first failure.
        #[arg(long)]
        fail_fast: bool,

        /// Enable deterministic memory tracking capability.
        #[arg(long)]
        mem_track: bool,

        /// Deterministic memory ceiling in MB.
        #[arg(long)]
        mem_limit_mb: Option<u64>,

        /// Deterministic allocation failure after N allocations.
        #[arg(long)]
        mem_fail_after: Option<u64>,

        /// Deterministic fragmentation overhead seed.
        #[arg(long)]
        mem_fragmentation_seed: Option<u64>,

        /// Deterministic pressure wave pattern (for example \"1,2,4\").
        #[arg(long)]
        mem_pressure_wave: Option<String>,

        /// Fail run on any detected leak.
        #[arg(long)]
        fail_on_leak: bool,

        /// Leak budget in bytes.
        #[arg(long)]
        leak_budget: Option<u64>,

        /// Emit dedicated memory artifacts.
        #[arg(long)]
        mem_artifacts: bool,

        /// Profiler capture overhead level.
        #[arg(long, default_value = "baseline")]
        profile_capture: ProfileCaptureLevel,
    },

    /// Run a single scenario file (one-off)
    Run {
        scenario: PathBuf,

        #[arg(long)]
        det: bool,

        #[arg(long)]
        seed: Option<u64>,

        #[arg(long)]
        timeout: Option<FozzyDuration>,

        #[arg(long, default_value = "pretty")]
        reporter: Reporter,

        #[arg(long)]
        record: Option<PathBuf>,

        /// Behavior when --record target exists: error, overwrite, or append with numeric suffix.
        #[arg(long, default_value = "append")]
        record_collision: RecordCollisionPolicy,

        #[arg(long)]
        mem_track: bool,
        #[arg(long)]
        mem_limit_mb: Option<u64>,
        #[arg(long)]
        mem_fail_after: Option<u64>,
        #[arg(long)]
        mem_fragmentation_seed: Option<u64>,
        #[arg(long)]
        mem_pressure_wave: Option<String>,
        #[arg(long)]
        fail_on_leak: bool,
        #[arg(long)]
        leak_budget: Option<u64>,
        #[arg(long)]
        mem_artifacts: bool,

        /// Profiler capture overhead level.
        #[arg(long, default_value = "baseline")]
        profile_capture: ProfileCaptureLevel,
    },

    /// Coverage-guided or property-based fuzzing
    Fuzz {
        target: String,

        #[arg(long)]
        det: bool,

        #[arg(long, default_value = "coverage")]
        mode: FuzzMode,

        #[arg(long)]
        seed: Option<u64>,

        #[arg(long)]
        time: Option<FozzyDuration>,

        #[arg(long)]
        runs: Option<u64>,

        #[arg(long, default_value_t = 4096)]
        max_input: usize,

        #[arg(long)]
        corpus: Option<PathBuf>,

        #[arg(long)]
        mutator: Option<String>,

        #[arg(long)]
        shrink: bool,

        #[arg(long)]
        record: Option<PathBuf>,

        #[arg(long, default_value = "pretty")]
        reporter: Reporter,

        #[arg(long)]
        crash_only: bool,

        #[arg(long)]
        minimize: bool,

        /// Behavior when --record target exists: error, overwrite, or append with numeric suffix.
        #[arg(long, default_value = "append")]
        record_collision: RecordCollisionPolicy,

        #[arg(long)]
        mem_track: bool,
        #[arg(long)]
        mem_limit_mb: Option<u64>,
        #[arg(long)]
        mem_fail_after: Option<u64>,
        #[arg(long)]
        mem_fragmentation_seed: Option<u64>,
        #[arg(long)]
        mem_pressure_wave: Option<String>,
        #[arg(long)]
        fail_on_leak: bool,
        #[arg(long)]
        leak_budget: Option<u64>,
        #[arg(long)]
        mem_artifacts: bool,

        /// Profiler capture overhead level.
        #[arg(long, default_value = "baseline")]
        profile_capture: ProfileCaptureLevel,
    },

    /// Deterministic distributed schedule + fault exploration
    Explore {
        scenario: PathBuf,

        #[arg(long)]
        seed: Option<u64>,

        #[arg(long)]
        time: Option<FozzyDuration>,

        #[arg(long)]
        steps: Option<u64>,

        #[arg(long)]
        nodes: Option<usize>,

        #[arg(long)]
        faults: Option<String>,

        #[arg(long, default_value = "fifo")]
        schedule: ScheduleStrategy,

        #[arg(long)]
        checker: Option<String>,

        #[arg(long)]
        record: Option<PathBuf>,

        #[arg(long)]
        shrink: bool,

        #[arg(long)]
        minimize: bool,

        #[arg(long, default_value = "pretty")]
        reporter: Reporter,

        /// Behavior when --record target exists: error, overwrite, or append with numeric suffix.
        #[arg(long, default_value = "error")]
        record_collision: RecordCollisionPolicy,

        #[arg(long)]
        mem_track: bool,
        #[arg(long)]
        mem_limit_mb: Option<u64>,
        #[arg(long)]
        mem_fail_after: Option<u64>,
        #[arg(long)]
        mem_fragmentation_seed: Option<u64>,
        #[arg(long)]
        mem_pressure_wave: Option<String>,
        #[arg(long)]
        fail_on_leak: bool,
        #[arg(long)]
        leak_budget: Option<u64>,
        #[arg(long)]
        mem_artifacts: bool,

        /// Profiler capture overhead level.
        #[arg(long, default_value = "baseline")]
        profile_capture: ProfileCaptureLevel,
    },

    /// Replay a previously recorded run exactly
    Replay {
        trace: PathBuf,

        #[arg(long)]
        step: bool,

        #[arg(long)]
        until: Option<FozzyDuration>,

        #[arg(long)]
        dump_events: bool,

        /// Profiler capture overhead level.
        #[arg(long, default_value = "baseline")]
        profile_capture: ProfileCaptureLevel,

        /// Force replay-side profile artifact regeneration for this replay run.
        #[arg(long)]
        profile_regen: bool,

        /// Optional replay-side profiler export format.
        #[arg(long)]
        profile_export_format: Option<ProfileExportFormat>,

        /// Output path used with --profile-export-format.
        #[arg(long)]
        profile_export_out: Option<PathBuf>,

        #[arg(long, default_value = "pretty")]
        reporter: Reporter,
    },

    /// Inspect and verify trace-file integrity/versioning
    Trace {
        #[command(subcommand)]
        command: TraceCommand,
    },

    /// Minimize a failing run (input + schedule + fault trace)
    Shrink {
        trace: PathBuf,

        #[arg(long)]
        out: Option<PathBuf>,

        #[arg(long)]
        budget: Option<FozzyDuration>,

        #[arg(long)]
        aggressive: bool,

        #[arg(long, default_value = "all")]
        minimize: ShrinkMinimize,

        /// Only `pretty` is supported here; use global `--json` for machine-readable output.
        #[arg(long, default_value = "pretty")]
        reporter: Reporter,
    },

    /// Manage fuzz corpora
    Corpus {
        #[command(subcommand)]
        command: CorpusCommand,
    },

    /// Inspect/export artifacts (traces, timelines, diffs)
    Artifacts {
        #[command(subcommand)]
        command: ArtifactCommand,
    },

    /// Render / query run reports (JSON, JUnit, HTML)
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },

    /// Inspect memory artifacts and summaries
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },

    /// Performance forensics profiler commands
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },

    /// Analyze repository topology and hotspot candidates for granular Fozzy suites
    Map {
        #[command(subcommand)]
        command: MapCommand,
    },

    /// Diagnose nondeterminism + environment issues
    Doctor {
        #[arg(long)]
        deep: bool,

        /// Scenario path for deterministic repeated-run audit (used with --deep).
        #[arg(long)]
        scenario: Option<PathBuf>,

        /// Number of repeated deterministic runs for audit (minimum 2).
        #[arg(long, default_value_t = 3)]
        runs: u32,

        /// Fixed seed used by deterministic audit runs.
        #[arg(long)]
        seed: Option<u64>,
    },

    /// Print environment + capability backend info
    Env,

    /// Run canonical CI gate checks for reproducibility/integrity
    Ci {
        /// Trace path used as the anchor artifact for verify/replay/export checks.
        trace: PathBuf,
        /// Optional run ids/trace paths used for flake-rate budget checks.
        #[arg(long = "flake-run")]
        flake_runs: Vec<String>,
        /// Maximum allowed flake rate percentage.
        #[arg(long = "flake-budget")]
        flake_budget: Option<FlakeBudget>,
        /// Baseline run/trace used for profiler latency budget checks.
        #[arg(long = "perf-baseline")]
        perf_baseline: Option<String>,
        /// Maximum allowed p99 latency delta percent vs --perf-baseline.
        #[arg(long = "max-p99-delta-pct")]
        max_p99_delta_pct: Option<f64>,
    },

    /// Run strict deterministic gate checks with optional scoped targeting.
    Gate {
        /// Gate profile.
        #[arg(long, default_value = "targeted")]
        profile: GateProfile,
        /// Root directory scanned for `*.fozzy.json` scenarios.
        #[arg(long, default_value = "tests")]
        scenario_root: PathBuf,
        /// Substring scope matcher applied to scenario paths (comma-separated).
        #[arg(long, value_delimiter = ',')]
        scope: Vec<String>,
        /// Deterministic seed for reproducible runs.
        #[arg(long)]
        seed: Option<u64>,
        /// Number of repeated deterministic runs in doctor deep audit.
        #[arg(long, default_value_t = 5)]
        doctor_runs: u32,
    },

    /// Print version and build info
    Version,

    /// Show a compact "what to use when" guide for each command, with examples.
    Usage,

    /// Print scenario/schema surface (file variants + step kinds) for automation.
    #[command(alias = "steps")]
    Schema,

    /// Validate a scenario file and emit parser/step-shape diagnostics.
    Validate { scenario: PathBuf },

    /// Run an end-to-end full-surface Fozzy gate with setup guidance and graceful skips.
    Full {
        /// Root directory scanned for `*.fozzy.json` scenarios.
        #[arg(long, default_value = "tests")]
        scenario_root: PathBuf,

        /// Deterministic seed for reproducible full runs.
        #[arg(long)]
        seed: Option<u64>,

        /// Number of repeated deterministic runs in doctor deep audit.
        #[arg(long, default_value_t = 5)]
        doctor_runs: u32,

        /// Fuzz duration used by `fozzy full`.
        #[arg(long, default_value = "2s")]
        fuzz_time: FozzyDuration,

        /// Explore step budget used for distributed scenarios.
        #[arg(long, default_value_t = 200)]
        explore_steps: u64,

        /// Explore node count override used for distributed scenarios.
        #[arg(long, default_value_t = 3)]
        explore_nodes: usize,

        /// Treat fail-class scenario outcomes as valid if replay/ci preserve the outcome class.
        #[arg(long)]
        allow_expected_failures: bool,

        /// Run only scenarios whose path contains this substring.
        #[arg(long)]
        scenario_filter: Option<String>,

        /// Skip specific full steps (comma-separated list).
        #[arg(long, value_delimiter = ',')]
        skip_steps: Vec<String>,

        /// If set, only these full steps are considered required (others are marked skipped).
        #[arg(long, value_delimiter = ',')]
        required_steps: Vec<String>,

        /// Require coverage for high-risk topology hotspots (pass repo root path to analyze).
        #[arg(long)]
        require_topology_coverage: Option<PathBuf>,

        /// Minimum hotspot risk score (0-100) considered required for topology coverage.
        #[arg(long, default_value_t = 60)]
        topology_min_risk: u8,

        /// Topology strictness profile used when checking coverage.
        #[arg(long, default_value = "pedantic")]
        topology_profile: TopologyProfile,

        /// Shrink evidence policy used by topology coverage (`failure_only`, `exercised_ok`, `no_known_failures`).
        #[arg(long, default_value = "no-known-failures")]
        topology_shrink_policy: ShrinkCoveragePolicy,
    },
}

#[derive(Debug, Subcommand)]
enum TraceCommand {
    /// Verify checksum/integrity and schema warnings for a .fozzy trace
    Verify { path: PathBuf },
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum GateProfile {
    Targeted,
}

impl clap::ValueEnum for GateProfile {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Targeted]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new("targeted"))
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum FullStepStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, serde::Serialize)]
struct FullStepResult {
    name: String,
    status: FullStepStatus,
    detail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct FullReport {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    strict: bool,
    unsafe_mode: bool,
    #[serde(rename = "scenarioRoot")]
    scenario_root: String,
    guidance: Vec<String>,
    #[serde(
        rename = "shrinkClassification",
        skip_serializing_if = "Option::is_none"
    )]
    shrink_classification: Option<String>,
    steps: Vec<FullStepResult>,
}

#[derive(Debug, Clone)]
struct FullScenarioDiscovery {
    steps: Vec<PathBuf>,
    distributed: Vec<PathBuf>,
    parse_errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct GateReport {
    #[serde(rename = "schemaVersion")]
    schema_version: String,
    profile: GateProfile,
    strict: bool,
    #[serde(rename = "scenarioRoot")]
    scenario_root: String,
    scopes: Vec<String>,
    #[serde(rename = "matchedScenarios")]
    matched_scenarios: Vec<String>,
    steps: Vec<FullStepResult>,
}

fn main() -> ExitCode {
    let normalized_args = normalize_global_args(std::env::args());
    let json_requested = args_request_json(&normalized_args);
    let cli = match Cli::try_parse_from(normalized_args) {
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
