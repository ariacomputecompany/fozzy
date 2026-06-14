use clap::{Parser, Subcommand, ValueEnum};

use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use walkdir::WalkDir;

use fozzy::{
    ArtifactCommand, CiOptions, CorpusCommand, ExitStatus, ExploreOptions, FlakeBudget,
    FozzyDuration, FsBackend, FuzzMode, FuzzOptions, FuzzTarget, HttpBackend, InitTemplate,
    InitTestType, MapCommand, MapSuitesOptions, MemoryCommand, MemoryOptions, ProcBackend,
    ProfileCaptureLevel, ProfileCommand, ProfileExportFormat, RecordCollisionPolicy, ReportCommand,
    Reporter, RunOptions, RunSummary, ScenarioPath, ScheduleStrategy, ShrinkCoveragePolicy,
    ShrinkMinimize, TopologyProfile, TracePath,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ExecutionReporter {
    Pretty,
    Junit,
    Html,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum PrettyReporter {
    Pretty,
}

impl From<ExecutionReporter> for Reporter {
    fn from(value: ExecutionReporter) -> Self {
        match value {
            ExecutionReporter::Pretty => Reporter::Pretty,
            ExecutionReporter::Junit => Reporter::Junit,
            ExecutionReporter::Html => Reporter::Html,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "fozzy")]
#[command(about = "deterministic full-stack testing + fuzzing + distributed exploration")]
#[command(
    after_help = "Start with `fozzy map suites --root . --scenario-root tests --profile pedantic --json` and follow suite gaps in full. Execution policy: use the full command surface by default (map/run/test/fuzz/explore/replay/shrink/trace verify/ci/report/artifacts/profile/memory/doctor/corpus/env/version/usage). Use `fozzy full` to run the end-to-end gate automatically; use `--unsafe` only when intentionally relaxing checks."
)]
pub(crate) struct Cli {
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
pub(crate) enum Command {
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

        /// Reporter artifact format (`pretty`, `junit`, or `html`). Use global `--json` for machine-readable stdout.
        /// Reporter artifact format (`pretty`, `junit`, or `html`). Use global `--json` for machine-readable stdout.
        #[arg(long, default_value = "pretty")]
        reporter: ExecutionReporter,

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

        /// Reporter artifact format (`pretty`, `junit`, or `html`). Use global `--json` for machine-readable stdout.
        #[arg(long, default_value = "pretty")]
        reporter: ExecutionReporter,

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

        /// Reporter artifact format (`pretty`, `junit`, or `html`). Use global `--json` for machine-readable stdout.
        #[arg(long, default_value = "pretty")]
        reporter: ExecutionReporter,

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
        reporter: ExecutionReporter,

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
        /// Recorded trace file to replay.
        trace: PathBuf,

        /// Stream the replay step-by-step instead of only printing the final summary.
        #[arg(long)]
        step: bool,

        /// Stop replay after this deterministic duration.
        #[arg(long)]
        until: Option<FozzyDuration>,

        /// Include recorded events in the replay output stream.
        #[arg(long)]
        dump_events: bool,

        /// Profiler capture overhead level.
        #[arg(long, default_value = "baseline")]
        profile_capture: ProfileCaptureLevel,

        /// Force replay-side profile artifact regeneration for this replay run.
        #[arg(long)]
        profile_regen: bool,

        /// Optional replay-side profiler export format.
        #[arg(long, requires = "profile_export_out")]
        profile_export_format: Option<ProfileExportFormat>,

        /// Output path used with --profile-export-format.
        #[arg(long)]
        profile_export_out: Option<PathBuf>,

        /// Reporter artifact format (`pretty`, `junit`, or `html`). Use global `--json` for machine-readable stdout.
        #[arg(long, default_value = "pretty")]
        reporter: ExecutionReporter,
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

        /// Use global `--json` for machine-readable output.
        #[arg(long, default_value = "pretty")]
        reporter: PrettyReporter,
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
pub(crate) enum TraceCommand {
    /// Verify checksum/integrity and schema warnings for a .fozzy trace
    Verify { path: PathBuf },
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GateProfile {
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
pub(crate) enum FullStepStatus {
    Passed,
    Failed,
    Skipped,
    Advisory,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct FullStepResult {
    name: String,
    status: FullStepStatus,
    detail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct FullReport {
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
pub(crate) struct FullScenarioDiscovery {
    steps: Vec<PathBuf>,
    distributed: Vec<PathBuf>,
    parse_errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct GateReport {
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
