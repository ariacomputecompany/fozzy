//! Deterministic profiler commands (`fozzy profile ...`).

#[path = "profile_analysis.rs"]
mod profile_analysis;
#[path = "profile_build.rs"]
mod profile_build;
#[path = "profile_dispatch.rs"]
mod profile_dispatch;
#[path = "profile_render.rs"]
mod profile_render;

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use profile_analysis::{
    compute_diff, explain_from_diff, explain_single, format_metric_value, metric_value,
    normalize_metric_value, shrink_minimize_name,
};
use profile_build::{
    build_cpu_profile, build_heap_profile, build_latency_profile, build_profile_metrics,
    build_profile_timeline, build_symbols_map,
};
use profile_render::{
    folded_to_speedscope, folded_to_svg, folded_to_text, heap_folded, timeline_html,
};

pub use profile_build::heap_budget_findings_from_trace;

use crate::{
    Config, Finding, FindingKind, FozzyError, FozzyResult, RunManifest, RunSummary, ShrinkMinimize,
    ShrinkOptions, TraceFile, TracePath, resolve_artifacts_dir, shrink_trace,
    shrink_trace_with_predicate,
};

const RUN_OR_TRACE_HELP: &str =
    "Run selector: run id, trace path (*.fozzy), or alias (latest|last-pass|last-fail).";
const RUN_OR_TRACE_LONG_HELP: &str = "Accepted forms:\n- run id directory under .fozzy/runs/<run-id>\n- direct trace path (*.fozzy)\n- aliases: latest, last-pass, last-fail\nResolution order:\n1) existing *.fozzy path\n2) .fozzy/runs/<selector>/trace.fozzy\n3) tracePath from report.json\n4) tracePath from manifest.json\n5) existing profile artifacts in the run directory";

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// Show top profiler hotspots and metrics.
    Top {
        #[arg(
            value_name = "RUN_OR_TRACE",
            help = RUN_OR_TRACE_HELP,
            long_help = RUN_OR_TRACE_LONG_HELP
        )]
        run: String,
        #[arg(long)]
        cpu: bool,
        #[arg(long)]
        heap: bool,
        #[arg(long)]
        latency: bool,
        #[arg(long)]
        io: bool,
        #[arg(long)]
        sched: bool,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Export flamegraph-compatible data.
    Flame {
        #[arg(
            value_name = "RUN_OR_TRACE",
            help = RUN_OR_TRACE_HELP,
            long_help = RUN_OR_TRACE_LONG_HELP
        )]
        run: String,
        #[arg(long)]
        cpu: bool,
        #[arg(long)]
        heap: bool,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "folded")]
        format: ProfileFlameFormat,
    },
    /// Export canonical profiler timeline.
    Timeline {
        #[arg(
            value_name = "RUN_OR_TRACE",
            help = RUN_OR_TRACE_HELP,
            long_help = RUN_OR_TRACE_LONG_HELP
        )]
        run: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "json")]
        format: ProfileTimelineFormat,
    },
    /// Compare two profiler runs/traces.
    Diff {
        #[arg(
            value_name = "LEFT_RUN_OR_TRACE",
            help = RUN_OR_TRACE_HELP,
            long_help = RUN_OR_TRACE_LONG_HELP
        )]
        left: String,
        #[arg(
            value_name = "RIGHT_RUN_OR_TRACE",
            help = RUN_OR_TRACE_HELP,
            long_help = RUN_OR_TRACE_LONG_HELP
        )]
        right: String,
        #[arg(long)]
        cpu: bool,
        #[arg(long)]
        heap: bool,
        #[arg(long)]
        latency: bool,
        #[arg(long)]
        io: bool,
        #[arg(long)]
        sched: bool,
    },
    /// Explain likely root causes for runtime behavior or regression.
    Explain {
        #[arg(
            value_name = "RUN_OR_TRACE",
            help = RUN_OR_TRACE_HELP,
            long_help = RUN_OR_TRACE_LONG_HELP
        )]
        run: String,
        #[arg(long = "diff-with")]
        diff_with: Option<String>,
    },
    /// Export profiler data into external formats.
    Export {
        #[arg(
            value_name = "RUN_OR_TRACE",
            help = RUN_OR_TRACE_HELP,
            long_help = RUN_OR_TRACE_LONG_HELP
        )]
        run: String,
        #[arg(long)]
        format: ProfileExportFormat,
        #[arg(long)]
        out: PathBuf,
    },
    /// Shrink a trace while preserving a profiler metric direction.
    Shrink {
        #[arg(
            value_name = "RUN_OR_TRACE",
            help = RUN_OR_TRACE_HELP,
            long_help = RUN_OR_TRACE_LONG_HELP
        )]
        run: String,
        #[arg(long)]
        metric: ProfileMetric,
        #[arg(long)]
        direction: ProfileDirection,
        #[arg(long)]
        budget: Option<crate::FozzyDuration>,
        #[arg(long, value_enum, default_value = "all")]
        minimize: ShrinkMinimize,
    },
    /// Show profiler capability visibility for this host/backend setup.
    Env,
    /// Run one-shot profile sanity checks for a run/trace.
    Doctor {
        #[arg(
            value_name = "RUN_OR_TRACE",
            help = RUN_OR_TRACE_HELP,
            long_help = RUN_OR_TRACE_LONG_HELP
        )]
        run: String,
        /// Include expensive shrink + metric-preservation checks.
        #[arg(long)]
        deep: bool,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileFlameFormat {
    Folded,
    Svg,
    Speedscope,
}

impl clap::ValueEnum for ProfileFlameFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Folded, Self::Svg, Self::Speedscope]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Folded => clap::builder::PossibleValue::new("folded"),
            Self::Svg => clap::builder::PossibleValue::new("svg"),
            Self::Speedscope => clap::builder::PossibleValue::new("speedscope"),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileTimelineFormat {
    Json,
    Html,
}

impl clap::ValueEnum for ProfileTimelineFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Json, Self::Html]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Json => clap::builder::PossibleValue::new("json"),
            Self::Html => clap::builder::PossibleValue::new("html"),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileExportFormat {
    Speedscope,
    Pprof,
    Otlp,
}

impl clap::ValueEnum for ProfileExportFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Speedscope, Self::Pprof, Self::Otlp]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Speedscope => clap::builder::PossibleValue::new("speedscope"),
            Self::Pprof => clap::builder::PossibleValue::new("pprof"),
            Self::Otlp => clap::builder::PossibleValue::new("otlp"),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileMetric {
    P99Latency,
    CpuTime,
    AllocBytes,
}

impl clap::ValueEnum for ProfileMetric {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::P99Latency, Self::CpuTime, Self::AllocBytes]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::P99Latency => clap::builder::PossibleValue::new("p99_latency"),
            Self::CpuTime => clap::builder::PossibleValue::new("cpu_time"),
            Self::AllocBytes => clap::builder::PossibleValue::new("alloc_bytes"),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileDirection {
    Increase,
    Decrease,
}

impl clap::ValueEnum for ProfileDirection {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Increase, Self::Decrease]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(match self {
            Self::Increase => clap::builder::PossibleValue::new("increase"),
            Self::Decrease => clap::builder::PossibleValue::new("decrease"),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ProfileEventKind {
    SpanStart,
    SpanEnd,
    Event,
    Sample,
    Alloc,
    Free,
    Io,
    Net,
    Sched,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileEvent {
    #[serde(rename = "t_virtual")]
    pub t_virtual: u64,
    #[serde(rename = "t_mono", skip_serializing_if = "Option::is_none")]
    pub t_mono: Option<u64>,
    pub kind: ProfileEventKind,
    #[serde(rename = "run_id")]
    pub run_id: String,
    pub seed: u64,
    pub thread: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(rename = "span_id")]
    pub span_id: String,
    #[serde(rename = "parent_span_id", skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
    pub cost: ProfileCost,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileTimelineArtifact {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "timeDomains")]
    pub time_domains: TimeDomains,
    pub events: Vec<ProfileEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileCost {
    #[serde(rename = "duration_ms", skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(rename = "bytes", skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    #[serde(rename = "count", skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetrics {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "timeDomains")]
    pub time_domains: TimeDomains,
    #[serde(rename = "virtualTimeMs")]
    pub virtual_time_ms: u64,
    #[serde(rename = "hostTimeMs")]
    pub host_time_ms: u64,
    #[serde(rename = "cpuTimeMs")]
    pub cpu_time_ms: u64,
    #[serde(rename = "allocBytes")]
    pub alloc_bytes: u64,
    #[serde(rename = "inUseBytes")]
    pub in_use_bytes: u64,
    #[serde(rename = "p50LatencyMs")]
    pub p50_latency_ms: u64,
    #[serde(rename = "p95LatencyMs")]
    pub p95_latency_ms: u64,
    #[serde(rename = "p99LatencyMs")]
    pub p99_latency_ms: u64,
    #[serde(rename = "maxLatencyMs")]
    pub max_latency_ms: u64,
    #[serde(rename = "ioOps")]
    pub io_ops: u64,
    #[serde(rename = "schedOps")]
    pub sched_ops: u64,
    #[serde(rename = "confidence", skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDomains {
    #[serde(rename = "virtualTime")]
    pub virtual_time: String,
    #[serde(rename = "hostMonotonicTime")]
    pub host_monotonic_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuProfile {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    pub collector: CpuCollectorInfo,
    #[serde(rename = "samplePeriodMs")]
    pub sample_period_ms: u64,
    #[serde(rename = "sampleCount")]
    pub sample_count: usize,
    pub samples: Vec<CpuSample>,
    #[serde(rename = "foldedStacks")]
    pub folded_stacks: Vec<FoldedStack>,
    #[serde(rename = "symbolsRef")]
    pub symbols_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuCollectorInfo {
    pub domain: String,
    #[serde(rename = "primaryCollector")]
    pub primary_collector: String,
    #[serde(rename = "fallbackCollector")]
    pub fallback_collector: String,
    #[serde(rename = "activeCollector")]
    pub active_collector: String,
    #[serde(rename = "hostTimeSemantics")]
    pub host_time_semantics: String,
    #[serde(rename = "linuxPerfEventOpen")]
    pub linux_perf_event_open: bool,
    pub diagnostics: Vec<String>,
    #[serde(rename = "macOsParityChecklist")]
    pub macos_parity_checklist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuSample {
    pub thread: String,
    pub stack: Vec<String>,
    #[serde(rename = "weightMs")]
    pub weight_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldedStack {
    pub stack: String,
    pub weight: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapProfile {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    #[serde(rename = "totalAllocBytes")]
    pub total_alloc_bytes: u64,
    #[serde(rename = "inUseBytes")]
    pub in_use_bytes: u64,
    #[serde(rename = "allocRatePerSec")]
    pub alloc_rate_per_sec: f64,
    #[serde(rename = "hotspots")]
    pub hotspots: Vec<HeapCallsite>,
    #[serde(rename = "lifetimeHistogram")]
    pub lifetime_histogram: Vec<HistogramBin>,
    #[serde(rename = "retentionSuspects")]
    pub retention_suspects: Vec<RetentionSuspect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeapCallsite {
    #[serde(rename = "callsiteHash")]
    pub callsite_hash: String,
    #[serde(rename = "allocCount")]
    pub alloc_count: u64,
    #[serde(rename = "allocBytes")]
    pub alloc_bytes: u64,
    #[serde(rename = "inUseBytes")]
    pub in_use_bytes: u64,
    #[serde(rename = "allocRatePerSec")]
    pub alloc_rate_per_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBin {
    pub bucket: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionSuspect {
    #[serde(rename = "allocId")]
    pub alloc_id: u64,
    #[serde(rename = "callsiteHash")]
    pub callsite_hash: String,
    pub bytes: u64,
    #[serde(rename = "ageMs")]
    pub age_ms: u64,
    #[serde(rename = "graphAnchor")]
    pub graph_anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HeapBudgetPolicy {
    #[serde(rename = "allocBytesBudget", skip_serializing_if = "Option::is_none")]
    pub alloc_bytes_budget: Option<u64>,
    #[serde(rename = "inUseBytesBudget", skip_serializing_if = "Option::is_none")]
    pub in_use_bytes_budget: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyProfile {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    pub distribution: LatencyDistribution,
    #[serde(rename = "dependencyGraph")]
    pub dependency_graph: Vec<CriticalPathEdge>,
    #[serde(rename = "criticalPath")]
    pub critical_path: Vec<CriticalPathEdge>,
    #[serde(rename = "waitReasons")]
    pub wait_reasons: Vec<ReasonCount>,
    #[serde(rename = "tailAmplificationSuspects")]
    pub tail_amplification_suspects: Vec<TailAmplificationSuspect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyDistribution {
    #[serde(rename = "count")]
    pub count: usize,
    #[serde(rename = "p50Ms")]
    pub p50_ms: u64,
    #[serde(rename = "p95Ms")]
    pub p95_ms: u64,
    #[serde(rename = "p99Ms")]
    pub p99_ms: u64,
    #[serde(rename = "maxMs")]
    pub max_ms: u64,
    #[serde(rename = "variance")]
    pub variance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriticalPathEdge {
    #[serde(rename = "fromSpan")]
    pub from_span: String,
    #[serde(rename = "toSpan")]
    pub to_span: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasonCount {
    pub reason: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailAmplificationSuspect {
    #[serde(rename = "spanId")]
    pub span_id: String,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolsMap {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    pub modules: Vec<SymbolModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolModule {
    pub name: String,
    #[serde(rename = "buildId")]
    pub build_id: String,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileExplain {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "run")]
    pub run: String,
    #[serde(rename = "regressionStatement")]
    pub regression_statement: String,
    #[serde(rename = "topShiftedPath")]
    pub top_shifted_path: String,
    #[serde(rename = "likelyCauseDomain")]
    pub likely_cause_domain: String,
    #[serde(rename = "evidencePointers")]
    pub evidence_pointers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileDiff {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub left: String,
    pub right: String,
    #[serde(rename = "leftSamples")]
    pub left_samples: usize,
    #[serde(rename = "rightSamples")]
    pub right_samples: usize,
    pub domains: Vec<String>,
    pub summary: DiffSummary,
    #[serde(rename = "regressions")]
    pub regressions: Vec<RegressionFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    #[serde(rename = "verdict")]
    pub verdict: String,
    #[serde(rename = "regressionCount")]
    pub regression_count: usize,
    #[serde(rename = "improvementCount")]
    pub improvement_count: usize,
    #[serde(rename = "significantRegressionCount")]
    pub significant_regression_count: usize,
    #[serde(
        rename = "topRegressionMetric",
        skip_serializing_if = "Option::is_none"
    )]
    pub top_regression_metric: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionFinding {
    pub domain: String,
    pub metric: String,
    #[serde(rename = "left")]
    pub left_value: f64,
    #[serde(rename = "right")]
    pub right_value: f64,
    #[serde(rename = "delta")]
    pub delta: f64,
    #[serde(rename = "deltaPct")]
    pub delta_pct: f64,
    #[serde(rename = "classification")]
    pub classification: String,
    #[serde(rename = "isRegression")]
    pub is_regression: bool,
    #[serde(rename = "isSignificant")]
    pub is_significant: bool,
    #[serde(rename = "severity")]
    pub severity: String,
    #[serde(rename = "analysis")]
    pub analysis: String,
    #[serde(rename = "timeDomain")]
    pub time_domain: String,
    #[serde(rename = "confidence")]
    pub confidence: f64,
    #[serde(
        rename = "confidenceMeta",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub confidence_meta: Option<ConfidenceMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceMeta {
    pub method: String,
    #[serde(rename = "leftSampleCount")]
    pub left_sample_count: usize,
    #[serde(rename = "rightSampleCount")]
    pub right_sample_count: usize,
    #[serde(rename = "leftStdDev")]
    pub left_std_dev: f64,
    #[serde(rename = "rightStdDev")]
    pub right_std_dev: f64,
    #[serde(rename = "pooledStdErr")]
    pub pooled_std_err: f64,
}

#[derive(Debug, Clone)]
struct ProfileBundle {
    artifacts_dir: PathBuf,
    timeline: Option<Vec<ProfileEvent>>,
    cpu: Option<CpuProfile>,
    heap: Option<HeapProfile>,
    latency: Option<LatencyProfile>,
    metrics: ProfileMetrics,
    symbols: Option<SymbolsMap>,
}

#[derive(Debug, Clone, Copy, Default)]
struct ProfileLoadSpec {
    timeline: bool,
    cpu: bool,
    heap: bool,
    latency: bool,
    symbols: bool,
}

#[derive(Debug, Clone)]
struct CpuCollectorCapability {
    primary_collector: String,
    fallback_collector: String,
    active_collector: String,
    linux_perf_event_open: bool,
    diagnostics: Vec<String>,
    sample_period_ms: u64,
}

#[derive(Debug, Clone, Default)]
struct MetricStats {
    n: usize,
    mean: f64,
    std_dev: f64,
}

pub fn profile_command(
    config: &Config,
    command: &ProfileCommand,
    strict: bool,
) -> FozzyResult<serde_json::Value> {
    profile_dispatch::dispatch_profile_command(config, command, strict)
}

fn profile_contract_or_error(
    strict: bool,
    command: &str,
    selector: &str,
    err: FozzyError,
) -> FozzyResult<serde_json::Value> {
    if strict {
        return Err(err);
    }
    match err {
        FozzyError::InvalidArgument(msg) => Ok(serde_json::json!({
            "schemaVersion": "fozzy.profile_contract_warning.v1",
            "status": "warn",
            "command": command,
            "run": selector,
            "detail": msg,
            "hint": "rerun with --strict for hard-fail contract enforcement",
        })),
        other => Err(other),
    }
}

fn relaxed_cpu_warning(_strict: bool, cpu_requested: bool) -> Option<String> {
    if !cpu_requested {
        return None;
    }
    Some(
        "cpu domain uses host-time sampling and is non-deterministic per replay; compare across repeated deterministic replays"
            .to_string(),
    )
}

pub fn write_profile_artifacts_from_trace(
    trace: &TraceFile,
    artifacts_dir: &Path,
) -> FozzyResult<()> {
    std::fs::create_dir_all(artifacts_dir)?;
    let timeline = build_profile_timeline(trace);
    let cpu = build_cpu_profile(trace, &timeline);
    let heap = build_heap_profile(trace, &timeline);
    let latency = build_latency_profile(trace, &timeline);
    let symbols = build_symbols_map(trace, &timeline, &cpu);
    let metrics = build_profile_metrics(trace, &timeline, &cpu, &heap, &latency);

    write_json(
        &artifacts_dir.join("profile.timeline.json"),
        &ProfileTimelineArtifact {
            schema_version: "fozzy.profile_timeline_artifact.v3".to_string(),
            run_id: trace.summary.identity.run_id.clone(),
            time_domains: TimeDomains {
                virtual_time: "deterministic, replay-critical timeline derived from virtual clock"
                    .to_string(),
                host_monotonic_time:
                    "non-deterministic host monotonic ordering used for performance comparison"
                        .to_string(),
            },
            events: timeline,
        },
    )?;
    write_json(&artifacts_dir.join("profile.cpu.json"), &cpu)?;
    write_json(&artifacts_dir.join("profile.heap.json"), &heap)?;
    write_json(&artifacts_dir.join("profile.latency.json"), &latency)?;
    write_json(&artifacts_dir.join("profile.metrics.json"), &metrics)?;
    write_json(&artifacts_dir.join("symbols.json"), &symbols)?;
    Ok(())
}

fn load_profile_bundle(
    config: &Config,
    selector: &str,
    spec: ProfileLoadSpec,
) -> FozzyResult<ProfileBundle> {
    let (artifacts_dir, trace_path) = resolve_profile_artifacts(config, selector)?;
    if let Some(trace_path) = trace_path {
        if profile_artifacts_stale(&artifacts_dir, &trace_path)? {
            let trace = TraceFile::read_json(&trace_path)?;
            write_profile_artifacts_from_trace(&trace, &artifacts_dir)?;
        }
    } else if !profile_artifacts_exist(&artifacts_dir) {
        return Err(FozzyError::InvalidArgument(format!(
            "no trace.fozzy found for {selector:?}; profiler requires trace artifacts"
        )));
    }

    let metrics: ProfileMetrics =
        serde_json::from_slice(&std::fs::read(artifacts_dir.join("profile.metrics.json"))?)?;
    let timeline = if spec.timeline {
        Some(
            serde_json::from_slice::<ProfileTimelineArtifact>(&std::fs::read(
                artifacts_dir.join("profile.timeline.json"),
            )?)?
            .events,
        )
    } else {
        None
    };
    let cpu = if spec.cpu {
        Some(serde_json::from_slice(&std::fs::read(
            artifacts_dir.join("profile.cpu.json"),
        )?)?)
    } else {
        None
    };
    let heap = if spec.heap {
        Some(serde_json::from_slice(&std::fs::read(
            artifacts_dir.join("profile.heap.json"),
        )?)?)
    } else {
        None
    };
    let latency = if spec.latency {
        Some(serde_json::from_slice(&std::fs::read(
            artifacts_dir.join("profile.latency.json"),
        )?)?)
    } else {
        None
    };
    let symbols = if spec.symbols {
        Some(serde_json::from_slice(&std::fs::read(
            artifacts_dir.join("symbols.json"),
        )?)?)
    } else {
        None
    };

    Ok(ProfileBundle {
        artifacts_dir,
        timeline,
        cpu,
        heap,
        latency,
        metrics,
        symbols,
    })
}

fn parse_selector_group(value: &str) -> Vec<String> {
    let selectors = value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if selectors.is_empty() {
        vec![value.to_string()]
    } else {
        selectors
    }
}

fn load_profile_bundle_group(
    config: &Config,
    selectors: &[String],
    spec: ProfileLoadSpec,
) -> FozzyResult<Vec<ProfileBundle>> {
    let mut bundles = Vec::<ProfileBundle>::new();
    for selector in selectors {
        bundles.push(load_profile_bundle(config, selector, spec)?);
    }
    Ok(bundles)
}

fn aggregate_metric_bundle(
    bundles: &[ProfileBundle],
) -> FozzyResult<(ProfileMetrics, HashMap<String, MetricStats>)> {
    let first = bundles.first().ok_or_else(|| {
        FozzyError::InvalidArgument("diff requires at least one sample".to_string())
    })?;
    let values_for = |field: fn(&ProfileMetrics) -> f64| {
        bundles
            .iter()
            .map(|b| field(&b.metrics))
            .collect::<Vec<f64>>()
    };
    let mut stats = HashMap::<String, MetricStats>::new();
    for (name, values) in [
        ("virtual_time_ms", values_for(|m| m.virtual_time_ms as f64)),
        ("cpu_time_ms", values_for(|m| m.cpu_time_ms as f64)),
        ("host_time_ms", values_for(|m| m.host_time_ms as f64)),
        ("p50_latency_ms", values_for(|m| m.p50_latency_ms as f64)),
        ("p95_latency_ms", values_for(|m| m.p95_latency_ms as f64)),
        ("p99_latency_ms", values_for(|m| m.p99_latency_ms as f64)),
        ("max_latency_ms", values_for(|m| m.max_latency_ms as f64)),
        ("alloc_bytes", values_for(|m| m.alloc_bytes as f64)),
        ("in_use_bytes", values_for(|m| m.in_use_bytes as f64)),
        ("io_ops", values_for(|m| m.io_ops as f64)),
        ("sched_ops", values_for(|m| m.sched_ops as f64)),
    ] {
        stats.insert(name.to_string(), metric_stats(&values));
    }

    let mean_u64 = |name: &str, fallback: u64| {
        stats
            .get(name)
            .map(|s| s.mean.max(0.0).round() as u64)
            .unwrap_or(fallback)
    };
    let mut out = first.metrics.clone();
    out.virtual_time_ms = mean_u64("virtual_time_ms", out.virtual_time_ms);
    out.host_time_ms = mean_u64("host_time_ms", out.host_time_ms);
    out.cpu_time_ms = mean_u64("cpu_time_ms", out.cpu_time_ms);
    out.alloc_bytes = mean_u64("alloc_bytes", out.alloc_bytes);
    out.in_use_bytes = mean_u64("in_use_bytes", out.in_use_bytes);
    out.p50_latency_ms = mean_u64("p50_latency_ms", out.p50_latency_ms);
    out.p95_latency_ms = mean_u64("p95_latency_ms", out.p95_latency_ms);
    out.p99_latency_ms = mean_u64("p99_latency_ms", out.p99_latency_ms);
    out.max_latency_ms = mean_u64("max_latency_ms", out.max_latency_ms);
    out.io_ops = mean_u64("io_ops", out.io_ops);
    out.sched_ops = mean_u64("sched_ops", out.sched_ops);
    out.confidence = Some(if bundles.len() <= 1 { 0.8 } else { 0.9 });
    Ok((out, stats))
}

fn metric_stats(values: &[f64]) -> MetricStats {
    if values.is_empty() {
        return MetricStats::default();
    }
    let n = values.len();
    let mean = values.iter().copied().sum::<f64>() / n as f64;
    let variance = values
        .iter()
        .map(|v| {
            let d = *v - mean;
            d * d
        })
        .sum::<f64>()
        / n as f64;
    MetricStats {
        n,
        mean,
        std_dev: variance.sqrt(),
    }
}

fn top_by_tag(
    timeline: &[ProfileEvent],
    kind: ProfileEventKind,
    limit: usize,
) -> Vec<serde_json::Value> {
    let mut counts = BTreeMap::<String, u64>::new();
    for event in timeline {
        if event.kind != kind {
            continue;
        }
        let name = event
            .tags
            .get("name")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        *counts.entry(name).or_insert(0) += 1;
    }
    let mut rows = counts.into_iter().collect::<Vec<_>>();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.into_iter()
        .take(limit)
        .map(|(name, count)| serde_json::json!({"name": name, "count": count}))
        .collect()
}

fn empty_domain(domain: &str, reason: &str) -> serde_json::Value {
    serde_json::json!({
        "domain": domain,
        "empty": true,
        "reason": reason,
    })
}

fn profile_env_report(config: &Config, strict: bool) -> serde_json::Value {
    let collector = detect_cpu_collector_capability();
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let cpu_quality = if collector.active_collector == "perf_event_open" {
        "high"
    } else {
        "degraded"
    };
    serde_json::json!({
        "schemaVersion": "fozzy.profile_env.v3",
        "strict": strict,
        "determinismContract": {
            "replayBoundTo": "deterministic_decisions_and_virtual_events",
            "nonDeterministicMeasurements": ["cpu_time_ms", "host_time_ms"],
        },
        "host": {
            "os": os,
            "arch": arch,
        },
        "backends": {
            "proc": format!("{:?}", config.proc_backend).to_lowercase(),
            "fs": format!("{:?}", config.fs_backend).to_lowercase(),
            "http": format!("{:?}", config.http_backend).to_lowercase(),
        },
        "domains": {
            "cpu": {
                "available": true,
                "quality": cpu_quality,
                "primaryCollector": collector.primary_collector,
                "activeCollector": collector.active_collector,
                "linuxPerfEventOpen": collector.linux_perf_event_open,
                "samplePeriodMs": collector.sample_period_ms,
                "diagnostics": collector.diagnostics,
                "notes": "host-time cpu sampling is non-deterministic; compare repeated deterministic runs statistically"
            },
            "heap": {
                "available": true,
                "quality": "high",
                "notes": "derived from memory_alloc/memory_free events in trace"
            },
            "latency": {
                "available": true,
                "quality": "high",
                "notes": "derived from deterministic trace timeline deltas"
            },
            "io": {
                "available": true,
                "quality": "high",
                "notes": "derived from io/net event counts in trace"
            },
            "sched": {
                "available": true,
                "quality": "high",
                "notes": "derived from distributed scheduler events in trace"
            }
        }
    })
}

fn profile_doctor(
    config: &Config,
    strict: bool,
    run: &str,
    deep: bool,
) -> FozzyResult<serde_json::Value> {
    let mut checks = Vec::<serde_json::Value>::new();
    let mut issues = Vec::<String>::new();
    checks.push(serde_json::json!({
        "name": "env",
        "ok": true,
        "status": "pass",
        "detail": profile_env_report(config, strict),
    }));

    let bundle = match load_profile_bundle(
        config,
        run,
        ProfileLoadSpec {
            timeline: true,
            cpu: true,
            heap: true,
            latency: true,
            symbols: false,
        },
    ) {
        Ok(bundle) => {
            checks.push(serde_json::json!({
                "name": "load_bundle",
                "ok": true,
                "status": "pass",
                "detail": "resolved run/trace and loaded profile artifacts",
            }));
            bundle
        }
        Err(err) => {
            let detail = err.to_string();
            issues.push(detail.clone());
            checks.push(serde_json::json!({
                "name": "load_bundle",
                "ok": false,
                "status": "fail",
                "detail": detail,
            }));
            return Ok(serde_json::json!({
                "schemaVersion": "fozzy.profile_doctor.v1",
                "run": run,
                "ok": false,
                "checks": checks,
                "issues": issues,
            }));
        }
    };

    let top_domains = normalize_domains(false, false, false, false, false);
    let top_has_any = !top_by_tag(
        bundle.timeline.as_ref().expect("timeline loaded"),
        ProfileEventKind::Io,
        10,
    )
    .is_empty()
        || !top_by_tag(
            bundle.timeline.as_ref().expect("timeline loaded"),
            ProfileEventKind::Sched,
            10,
        )
        .is_empty()
        || !bundle
            .heap
            .as_ref()
            .expect("heap loaded")
            .hotspots
            .is_empty()
        || !bundle
            .latency
            .as_ref()
            .expect("latency loaded")
            .critical_path
            .is_empty();
    checks.push(serde_json::json!({
        "name": "top",
        "ok": true,
        "status": if top_has_any { "pass" } else { "warn" },
        "detail": format!("default domains={top_domains:?}"),
    }));

    let heap_folded = heap_folded(bundle.heap.as_ref().expect("heap loaded"));
    checks.push(serde_json::json!({
        "name": "flame_heap",
        "ok": true,
        "status": if heap_folded.is_empty() { "warn" } else { "pass" },
        "detail": if heap_folded.is_empty() { "no heap samples in trace" } else { "heap flame data present" },
    }));
    checks.push(serde_json::json!({
        "name": "flame_cpu",
        "ok": true,
        "status": if bundle.cpu.as_ref().expect("cpu loaded").folded_stacks.is_empty() { "warn" } else { "pass" },
        "detail": if bundle.cpu.as_ref().expect("cpu loaded").folded_stacks.is_empty() { "no cpu samples in trace" } else { "cpu flame data present" },
    }));

    checks.push(serde_json::json!({
        "name": "timeline",
        "ok": true,
        "status": "pass",
        "detail": format!("events={}", bundle.timeline.as_ref().expect("timeline loaded").len()),
    }));
    let diff = compute_diff(
        run,
        run,
        &["cpu".to_string(), "heap".to_string(), "latency".to_string()],
        &bundle.metrics,
        &bundle.metrics,
        bundle.heap.as_ref(),
        bundle.heap.as_ref(),
        &HashMap::new(),
        &HashMap::new(),
        1,
        1,
    );
    checks.push(serde_json::json!({
        "name": "diff",
        "ok": true,
        "status": "pass",
        "detail": format!("regressions={}", diff.regressions.len()),
    }));
    let explain = explain_single(
        run,
        &bundle.artifacts_dir,
        &bundle.metrics,
        bundle.latency.as_ref().expect("latency loaded"),
    );
    checks.push(serde_json::json!({
        "name": "explain",
        "ok": true,
        "status": "pass",
        "detail": explain.likely_cause_domain,
    }));
    let speedscope: serde_json::Value =
        folded_to_speedscope(run, &bundle.cpu.as_ref().expect("cpu loaded").folded_stacks);
    checks.push(serde_json::json!({
        "name": "export",
        "ok": true,
        "status": "pass",
        "detail": format!("speedscope_frames={}", speedscope.get("shared").and_then(|v| v.get("frames")).and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0)),
    }));

    if deep {
        let shrink_check = match resolve_profile_trace(config, run) {
            Ok((_, trace_path)) => {
                let out = std::env::temp_dir().join(format!(
                    "fozzy-profile-doctor-{}.trace.fozzy",
                    uuid::Uuid::new_v4()
                ));
                match shrink_trace(
                    config,
                    TracePath::new(trace_path.clone()),
                    &ShrinkOptions {
                        out_trace_path: Some(out.clone()),
                        budget: Some(std::time::Duration::from_secs(2)),
                        aggressive: false,
                        minimize: ShrinkMinimize::All,
                    },
                ) {
                    Ok(s) => {
                        let shrunk_trace = TraceFile::read_json(Path::new(&s.out_trace_path))?;
                        let baseline = metric_value(
                            ProfileMetric::CpuTime,
                            &TraceFile::read_json(&trace_path)?,
                        )?;
                        let after = metric_value(ProfileMetric::CpuTime, &shrunk_trace)?;
                        let preserved = after >= baseline;
                        serde_json::json!({
                            "name": "shrink_cpu_increase",
                            "ok": true,
                            "status": if preserved { "pass" } else { "warn" },
                            "detail": if preserved {
                                format!("preserved contract baseline={} after={}", format_metric_value(baseline), format_metric_value(after))
                            } else {
                                format!("no feasible shrink found that preserves increase contract baseline={} after={}", format_metric_value(baseline), format_metric_value(after))
                            }
                        })
                    }
                    Err(err) => serde_json::json!({
                        "name": "shrink_cpu_increase",
                        "ok": false,
                        "status": "fail",
                        "detail": err.to_string(),
                    }),
                }
            }
            Err(err) => serde_json::json!({
                "name": "shrink_cpu_increase",
                "ok": false,
                "status": "fail",
                "detail": err.to_string(),
            }),
        };
        if shrink_check
            .get("ok")
            .and_then(|v| v.as_bool())
            .is_some_and(|v| !v)
        {
            if let Some(detail) = shrink_check.get("detail").and_then(|v| v.as_str()) {
                issues.push(detail.to_string());
            }
        }
        checks.push(shrink_check);
    } else {
        checks.push(serde_json::json!({
            "name": "shrink_cpu_increase",
            "ok": true,
            "status": "pass",
            "detail": "skipped (use --deep for shrink+contract checks)",
        }));
    }

    let ok = checks
        .iter()
        .all(|c| c.get("ok").and_then(|v| v.as_bool()).unwrap_or(false));
    Ok(serde_json::json!({
        "schemaVersion": "fozzy.profile_doctor.v1",
        "run": run,
        "ok": ok,
        "checks": checks,
        "issues": issues,
    }))
}

fn resolve_profile_trace(config: &Config, selector: &str) -> FozzyResult<(PathBuf, PathBuf)> {
    let (artifacts_dir, trace_path) = resolve_profile_artifacts(config, selector)?;
    if let Some(trace_path) = trace_path {
        return Ok((artifacts_dir, trace_path));
    }
    Err(FozzyError::InvalidArgument(format!(
        "no trace.fozzy found for {selector:?}; profiler requires trace artifacts"
    )))
}

fn resolve_profile_artifacts(
    config: &Config,
    selector: &str,
) -> FozzyResult<(PathBuf, Option<PathBuf>)> {
    let input = PathBuf::from(selector);
    if input.exists()
        && input.is_file()
        && input
            .extension()
            .and_then(|s| s.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("fozzy"))
    {
        let canonical = std::fs::canonicalize(&input).unwrap_or_else(|_| input.clone());
        let key = blake3::hash(canonical.to_string_lossy().as_bytes())
            .to_hex()
            .to_string();
        let dir = config.base_dir.join("profile-cache").join(key);
        return Ok((dir, Some(input)));
    }

    let artifacts_dir = resolve_artifacts_dir(config, selector)?;
    let trace_path = artifacts_dir.join("trace.fozzy");
    if trace_path.exists() {
        return Ok((artifacts_dir, Some(trace_path)));
    }

    let report_path = artifacts_dir.join("report.json");
    if report_path.exists() {
        let bytes = std::fs::read(&report_path)?;
        if let Ok(summary) = serde_json::from_slice::<RunSummary>(&bytes) {
            if let Some(path) = summary.identity.trace_path {
                let from_report = PathBuf::from(path);
                if from_report.exists() {
                    return Ok((artifacts_dir, Some(from_report)));
                }
            }
        }
    }

    let manifest_path = artifacts_dir.join("manifest.json");
    if manifest_path.exists() {
        let bytes = std::fs::read(&manifest_path)?;
        if let Ok(manifest) = serde_json::from_slice::<RunManifest>(&bytes) {
            if let Some(path) = manifest.trace_path {
                let from_manifest = PathBuf::from(path);
                if from_manifest.exists() {
                    return Ok((artifacts_dir, Some(from_manifest)));
                }
            }
        }
    }

    Ok((artifacts_dir, None))
}

fn profile_artifacts_exist(artifacts_dir: &Path) -> bool {
    for name in [
        "profile.timeline.json",
        "profile.cpu.json",
        "profile.heap.json",
        "profile.latency.json",
        "profile.metrics.json",
        "symbols.json",
    ] {
        if !artifacts_dir.join(name).exists() {
            return false;
        }
    }
    true
}

fn normalize_domains(cpu: bool, heap: bool, latency: bool, io: bool, sched: bool) -> Vec<String> {
    if !cpu && !heap && !latency && !io && !sched {
        return vec![
            "cpu".to_string(),
            "io".to_string(),
            "sched".to_string(),
            "heap".to_string(),
            "latency".to_string(),
        ];
    }
    let mut out = Vec::new();
    if cpu {
        out.push("cpu".to_string());
    }
    if heap {
        out.push("heap".to_string());
    }
    if latency {
        out.push("latency".to_string());
    }
    if io {
        out.push("io".to_string());
    }
    if sched {
        out.push("sched".to_string());
    }
    out
}

fn enforce_cpu_contract(strict: bool, cpu_requested: bool) -> FozzyResult<()> {
    let _ = (strict, cpu_requested);
    Ok(())
}

fn detect_cpu_collector_capability() -> CpuCollectorCapability {
    let fallback = "in_process_sampler".to_string();
    if cfg!(target_os = "linux") {
        let mut diagnostics = Vec::<String>::new();
        let perf_device_present = Path::new("/sys/bus/event_source/devices/cpu/type").exists();
        diagnostics.push(format!("perf_event_device_present={perf_device_present}"));

        let paranoid = read_proc_int("/proc/sys/kernel/perf_event_paranoid");
        if let Some(v) = paranoid {
            diagnostics.push(format!("perf_event_paranoid={v}"));
        } else {
            diagnostics.push("perf_event_paranoid=unknown".to_string());
        }

        let kptr = read_proc_int("/proc/sys/kernel/kptr_restrict");
        if let Some(v) = kptr {
            diagnostics.push(format!("kptr_restrict={v}"));
        }

        let perf_allowed = perf_device_present && paranoid.is_some_and(|v| v <= 2);
        let active = if perf_allowed {
            "perf_event_open".to_string()
        } else {
            fallback.clone()
        };
        if !perf_allowed {
            diagnostics.push(
                "falling back to in_process_sampler (perf_event_open unavailable for current permissions)"
                    .to_string(),
            );
        }
        CpuCollectorCapability {
            primary_collector: "perf_event_open".to_string(),
            fallback_collector: fallback,
            active_collector: active,
            linux_perf_event_open: perf_allowed,
            diagnostics,
            sample_period_ms: 10,
        }
    } else if cfg!(target_os = "macos") {
        CpuCollectorCapability {
            primary_collector: "mach_thread_sampler".to_string(),
            fallback_collector: fallback.clone(),
            active_collector: fallback,
            linux_perf_event_open: false,
            diagnostics: vec![
                "mach_thread_sampler planned; using in_process_sampler fallback".to_string(),
                "symbolization path planned via dSYM/atos parity".to_string(),
            ],
            sample_period_ms: 10,
        }
    } else {
        CpuCollectorCapability {
            primary_collector: "perf_event_open".to_string(),
            fallback_collector: fallback.clone(),
            active_collector: fallback,
            linux_perf_event_open: false,
            diagnostics: vec![
                "perf_event_open collector is Linux-only; using in_process_sampler fallback"
                    .to_string(),
            ],
            sample_period_ms: 10,
        }
    }
}

fn read_proc_int(path: &str) -> Option<i64> {
    let raw = std::fs::read_to_string(path).ok()?;
    raw.trim().parse::<i64>().ok()
}

fn write_json(path: &Path, value: &impl Serialize) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec(value)?)?;
    Ok(())
}

fn profile_artifacts_stale(artifacts_dir: &Path, trace_path: &Path) -> FozzyResult<bool> {
    if !profile_artifacts_exist(artifacts_dir) {
        return Ok(true);
    }
    let trace_mtime = std::fs::metadata(trace_path)?.modified()?;
    for name in [
        "profile.timeline.json",
        "profile.cpu.json",
        "profile.heap.json",
        "profile.latency.json",
        "profile.metrics.json",
        "symbols.json",
    ] {
        let p = artifacts_dir.join(name);
        let md = std::fs::metadata(&p)?;
        if md.modified()? < trace_mtime {
            return Ok(true);
        }
    }
    Ok(false)
}

fn write_text(path: &Path, value: &str) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, value)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExitStatus, RunIdentity, RunMode, RunSummary, ScenarioV1Steps, Step, TraceEvent};
    use std::path::PathBuf;

    fn sample_trace() -> TraceFile {
        TraceFile {
            format: "fozzy-trace".to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(ScenarioV1Steps {
                version: 1,
                name: "no-heap".to_string(),
                steps: vec![
                    Step::TraceEvent {
                        name: "setup".to_string(),
                        fields: serde_json::Map::new(),
                    },
                    Step::TraceEvent {
                        name: "teardown".to_string(),
                        fields: serde_json::Map::new(),
                    },
                ],
            }),
            fuzz: None,
            explore: None,
            memory: None,
            decisions: Vec::new(),
            events: vec![
                TraceEvent {
                    time_ms: 1,
                    name: "http_request".to_string(),
                    fields: serde_json::Map::new(),
                },
                TraceEvent {
                    time_ms: 4,
                    name: "memory_alloc".to_string(),
                    fields: serde_json::Map::from_iter([
                        ("alloc_id".to_string(), serde_json::json!(1)),
                        ("bytes".to_string(), serde_json::json!(64)),
                        (
                            "callsite_hash".to_string(),
                            serde_json::json!("step:memory_alloc"),
                        ),
                    ]),
                },
                TraceEvent {
                    time_ms: 8,
                    name: "memory_free".to_string(),
                    fields: serde_json::Map::from_iter([(
                        "alloc_id".to_string(),
                        serde_json::json!(1),
                    )]),
                },
            ],
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r1".to_string(),
                    seed: 7,
                    trace_path: None,
                    report_path: None,
                    artifacts_dir: None,
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 10,
                duration_ns: 10_000_000,
                tests: None,
                memory: None,
                findings: Vec::new(),
            },
            checksum: None,
        }
    }

    fn sample_trace_without_heap() -> TraceFile {
        TraceFile {
            format: "fozzy-trace".to_string(),
            version: crate::CURRENT_TRACE_VERSION,
            engine: crate::version_info(),
            mode: RunMode::Run,
            scenario_path: None,
            scenario: Some(ScenarioV1Steps {
                version: 1,
                name: "no-heap".to_string(),
                steps: vec![
                    Step::TraceEvent {
                        name: "setup".to_string(),
                        fields: serde_json::Map::new(),
                    },
                    Step::TraceEvent {
                        name: "teardown".to_string(),
                        fields: serde_json::Map::new(),
                    },
                ],
            }),
            fuzz: None,
            explore: None,
            memory: None,
            decisions: Vec::new(),
            events: vec![
                TraceEvent {
                    time_ms: 1,
                    name: "setup".to_string(),
                    fields: serde_json::Map::new(),
                },
                TraceEvent {
                    time_ms: 5,
                    name: "work".to_string(),
                    fields: serde_json::Map::new(),
                },
                TraceEvent {
                    time_ms: 9,
                    name: "teardown".to_string(),
                    fields: serde_json::Map::new(),
                },
            ],
            summary: RunSummary {
                status: ExitStatus::Pass,
                mode: RunMode::Run,
                identity: RunIdentity {
                    run_id: "r-no-heap".to_string(),
                    seed: 9,
                    trace_path: None,
                    report_path: None,
                    artifacts_dir: None,
                },
                started_at: "2026-01-01T00:00:00Z".to_string(),
                finished_at: "2026-01-01T00:00:00Z".to_string(),
                duration_ms: 1,
                duration_ns: 1_000_000,
                tests: None,
                memory: None,
                findings: Vec::new(),
            },
            checksum: None,
        }
    }

    #[test]
    fn timeline_builds_required_fields() {
        let trace = sample_trace();
        let timeline = build_profile_timeline(&trace);
        assert_eq!(timeline.len(), 3);
        assert_eq!(timeline[0].run_id, "r1");
        assert_eq!(timeline[0].seed, 7);
    }

    #[test]
    fn diff_is_deterministic() {
        let trace = sample_trace();
        let timeline = build_profile_timeline(&trace);
        let cpu = build_cpu_profile(&trace, &timeline);
        let heap = build_heap_profile(&trace, &timeline);
        let latency = build_latency_profile(&trace, &timeline);
        let metrics = build_profile_metrics(&trace, &timeline, &cpu, &heap, &latency);
        let diff_a = compute_diff(
            "a",
            "b",
            &["latency".to_string(), "heap".to_string()],
            &metrics,
            &metrics,
            Some(&heap),
            Some(&heap),
            &HashMap::new(),
            &HashMap::new(),
            1,
            1,
        );
        let diff_b = compute_diff(
            "a",
            "b",
            &["latency".to_string(), "heap".to_string()],
            &metrics,
            &metrics,
            Some(&heap),
            Some(&heap),
            &HashMap::new(),
            &HashMap::new(),
            1,
            1,
        );
        assert_eq!(
            serde_json::to_string(&diff_a).expect("json"),
            serde_json::to_string(&diff_b).expect("json")
        );
    }

    #[test]
    fn profile_event_schema_roundtrip_and_compatibility() {
        let trace = sample_trace();
        let timeline = build_profile_timeline(&trace);
        let event = timeline.first().expect("event");
        let encoded = serde_json::to_vec(event).expect("encode");
        let decoded: ProfileEvent = serde_json::from_slice(&encoded).expect("decode");
        assert_eq!(decoded.t_virtual, event.t_virtual);
        assert_eq!(decoded.kind, event.kind);
        assert_eq!(decoded.run_id, event.run_id);
        assert_eq!(decoded.seed, event.seed);
        assert_eq!(decoded.thread, event.thread);
        assert_eq!(decoded.span_id, event.span_id);

        let compat_json = serde_json::json!({
            "t_virtual": 42,
            "kind": "io",
            "run_id": "compat-run",
            "seed": 99,
            "thread": "main",
            "span_id": "s-1",
            "cost": {"duration_ms": 3},
            "unknown_field": "ignored"
        });
        let compat: ProfileEvent = serde_json::from_value(compat_json).expect("compat decode");
        assert_eq!(compat.t_virtual, 42);
        assert_eq!(compat.kind, ProfileEventKind::Io);
        assert_eq!(compat.t_mono, None);
        assert_eq!(compat.task, None);
        assert!(compat.tags.is_empty());
        assert_eq!(compat.cost.duration_ms, Some(3));
    }

    #[test]
    fn folded_stack_aggregation_is_correct_and_stable() {
        let mut trace = sample_trace();
        trace.events = vec![
            TraceEvent {
                time_ms: 1,
                name: "sample".to_string(),
                fields: serde_json::Map::from_iter([
                    (
                        "stack".to_string(),
                        serde_json::json!("fozzy::runtime;step::a"),
                    ),
                    ("weight_ms".to_string(), serde_json::json!(3)),
                ]),
            },
            TraceEvent {
                time_ms: 2,
                name: "sample".to_string(),
                fields: serde_json::Map::from_iter([
                    (
                        "stack".to_string(),
                        serde_json::json!("fozzy::runtime;step::a"),
                    ),
                    ("weight_ms".to_string(), serde_json::json!(2)),
                ]),
            },
            TraceEvent {
                time_ms: 3,
                name: "sample".to_string(),
                fields: serde_json::Map::from_iter([
                    (
                        "stack".to_string(),
                        serde_json::json!("fozzy::runtime;step::b"),
                    ),
                    ("weight_ms".to_string(), serde_json::json!(5)),
                ]),
            },
        ];
        let timeline = build_profile_timeline(&trace);
        let cpu = build_cpu_profile(&trace, &timeline);
        assert_eq!(cpu.folded_stacks.len(), 2);
        assert_eq!(cpu.folded_stacks[0].stack, "fozzy::runtime;step::a");
        assert_eq!(cpu.folded_stacks[0].weight, 5);
        assert_eq!(cpu.folded_stacks[1].stack, "fozzy::runtime;step::b");
        assert_eq!(cpu.folded_stacks[1].weight, 5);
    }

    #[test]
    fn latency_critical_path_extraction_is_correct() {
        let mut trace = sample_trace();
        trace.events = vec![
            TraceEvent {
                time_ms: 0,
                name: "span_start".to_string(),
                fields: serde_json::Map::from_iter([(
                    "span".to_string(),
                    serde_json::json!("root"),
                )]),
            },
            TraceEvent {
                time_ms: 1,
                name: "span_start".to_string(),
                fields: serde_json::Map::from_iter([
                    ("span".to_string(), serde_json::json!("db")),
                    ("parent_span".to_string(), serde_json::json!("root")),
                ]),
            },
            TraceEvent {
                time_ms: 4,
                name: "http_request".to_string(),
                fields: serde_json::Map::from_iter([
                    ("span".to_string(), serde_json::json!("io-1")),
                    ("parent_span".to_string(), serde_json::json!("root")),
                ]),
            },
            TraceEvent {
                time_ms: 7,
                name: "span_end".to_string(),
                fields: serde_json::Map::from_iter([("span".to_string(), serde_json::json!("db"))]),
            },
            TraceEvent {
                time_ms: 10,
                name: "span_end".to_string(),
                fields: serde_json::Map::from_iter([(
                    "span".to_string(),
                    serde_json::json!("root"),
                )]),
            },
        ];
        let timeline = build_profile_timeline(&trace);
        let latency = build_latency_profile(&trace, &timeline);
        assert!(
            !latency.critical_path.is_empty(),
            "expected critical path edges"
        );
        assert_eq!(latency.critical_path[0].to_span, "root");
        assert_eq!(latency.critical_path[0].reason, "io");
    }

    #[test]
    fn heap_callsite_and_lifetime_histogram_aggregation_is_correct() {
        let mut trace = sample_trace();
        trace.events = vec![
            TraceEvent {
                time_ms: 1,
                name: "memory_alloc".to_string(),
                fields: serde_json::Map::from_iter([
                    ("alloc_id".to_string(), serde_json::json!(1)),
                    ("bytes".to_string(), serde_json::json!(64)),
                    ("callsite_hash".to_string(), serde_json::json!("cs:A")),
                ]),
            },
            TraceEvent {
                time_ms: 2,
                name: "memory_alloc".to_string(),
                fields: serde_json::Map::from_iter([
                    ("alloc_id".to_string(), serde_json::json!(2)),
                    ("bytes".to_string(), serde_json::json!(32)),
                    ("callsite_hash".to_string(), serde_json::json!("cs:A")),
                ]),
            },
            TraceEvent {
                time_ms: 4,
                name: "memory_free".to_string(),
                fields: serde_json::Map::from_iter([(
                    "alloc_id".to_string(),
                    serde_json::json!(1),
                )]),
            },
            TraceEvent {
                time_ms: 20,
                name: "memory_free".to_string(),
                fields: serde_json::Map::from_iter([(
                    "alloc_id".to_string(),
                    serde_json::json!(2),
                )]),
            },
            TraceEvent {
                time_ms: 30,
                name: "memory_alloc".to_string(),
                fields: serde_json::Map::from_iter([
                    ("alloc_id".to_string(), serde_json::json!(3)),
                    ("bytes".to_string(), serde_json::json!(16)),
                    ("callsite_hash".to_string(), serde_json::json!("cs:B")),
                ]),
            },
        ];
        trace.summary.duration_ms = 30;
        let timeline = build_profile_timeline(&trace);
        let heap = build_heap_profile(&trace, &timeline);
        let cs_a = heap
            .hotspots
            .iter()
            .find(|h| h.callsite_hash == "cs:A")
            .expect("cs:A");
        let cs_b = heap
            .hotspots
            .iter()
            .find(|h| h.callsite_hash == "cs:B")
            .expect("cs:B");
        assert_eq!(cs_a.alloc_count, 2);
        assert_eq!(cs_a.alloc_bytes, 96);
        assert_eq!(cs_a.in_use_bytes, 0);
        assert_eq!(cs_b.alloc_count, 1);
        assert_eq!(cs_b.in_use_bytes, 16);
        assert!(
            heap.lifetime_histogram
                .iter()
                .any(|b| b.bucket == "2-10ms" && b.count == 1)
        );
        assert!(
            heap.lifetime_histogram
                .iter()
                .any(|b| b.bucket == "11-100ms" && b.count == 1)
        );
    }

    #[test]
    fn diff_tie_breaking_is_deterministic() {
        let trace = sample_trace();
        let timeline = build_profile_timeline(&trace);
        let cpu = build_cpu_profile(&trace, &timeline);
        let heap = build_heap_profile(&trace, &timeline);
        let latency = build_latency_profile(&trace, &timeline);
        let mut left = build_profile_metrics(&trace, &timeline, &cpu, &heap, &latency);
        let mut right = left.clone();
        left.io_ops = 0;
        left.sched_ops = 0;
        right.io_ops = 1;
        right.sched_ops = 1;
        let diff = compute_diff(
            "left",
            "right",
            &["io".to_string(), "sched".to_string()],
            &left,
            &right,
            None,
            None,
            &HashMap::new(),
            &HashMap::new(),
            1,
            1,
        );
        let metrics = diff
            .regressions
            .iter()
            .map(|r| r.metric.clone())
            .collect::<Vec<_>>();
        assert_eq!(metrics, vec!["io_ops".to_string(), "sched_ops".to_string()]);
    }

    fn temp_workspace(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fozzy-profile-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("workspace");
        dir
    }

    #[test]
    fn resolve_profile_trace_supports_run_with_report_trace_path() {
        let ws = temp_workspace("resolve-report-trace");
        let base_dir = ws.join(".fozzy");
        let run_id = "run-1";
        let run_dir = base_dir.join("runs").join(run_id);
        std::fs::create_dir_all(&run_dir).expect("run dir");

        let mut trace = sample_trace();
        trace.summary.identity.run_id = run_id.to_string();
        let external_trace = ws.join("external.trace.fozzy");
        std::fs::write(
            &external_trace,
            serde_json::to_vec_pretty(&trace).expect("trace bytes"),
        )
        .expect("write trace");

        let mut summary = trace.summary.clone();
        summary.identity.trace_path = Some(external_trace.to_string_lossy().to_string());
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&summary).expect("summary bytes"),
        )
        .expect("write report");

        let cfg = Config {
            base_dir: base_dir.clone(),
            ..Config::default()
        };
        let cmd = ProfileCommand::Top {
            run: run_id.to_string(),
            cpu: false,
            heap: true,
            latency: false,
            io: false,
            sched: false,
            limit: 5,
        };
        let out = profile_command(&cfg, &cmd, true).expect("profile top");
        assert_eq!(out.get("run").and_then(|v| v.as_str()), Some(run_id));
    }

    #[test]
    fn profile_commands_support_run_id_with_profile_artifacts_only() {
        let ws = temp_workspace("artifacts-only-run");
        let base_dir = ws.join(".fozzy");
        let run_id = "run-artifacts-only";
        let run_dir = base_dir.join("runs").join(run_id);
        std::fs::create_dir_all(&run_dir).expect("run dir");

        let mut trace = sample_trace();
        trace.summary.identity.run_id = run_id.to_string();
        write_profile_artifacts_from_trace(&trace, &run_dir).expect("profile artifacts");
        std::fs::write(
            run_dir.join("report.json"),
            serde_json::to_vec_pretty(&trace.summary).expect("summary bytes"),
        )
        .expect("write report");

        let cfg = Config {
            base_dir: base_dir.clone(),
            ..Config::default()
        };
        let cmd = ProfileCommand::Top {
            run: run_id.to_string(),
            cpu: false,
            heap: true,
            latency: false,
            io: false,
            sched: false,
            limit: 5,
        };
        let out = profile_command(&cfg, &cmd, true).expect("profile top");
        assert_eq!(out.get("run").and_then(|v| v.as_str()), Some(run_id));
    }

    #[test]
    fn explain_diff_keeps_primary_run_as_run_field() {
        let trace = sample_trace();
        let timeline = build_profile_timeline(&trace);
        let cpu = build_cpu_profile(&trace, &timeline);
        let heap = build_heap_profile(&trace, &timeline);
        let latency = build_latency_profile(&trace, &timeline);
        let metrics = build_profile_metrics(&trace, &timeline, &cpu, &heap, &latency);
        let explain = explain_from_diff("primary", "baseline", &metrics, &metrics);
        assert_eq!(explain.run, "primary");
    }

    #[test]
    fn timeline_json_out_matches_stdout_schema() {
        let ws = temp_workspace("timeline-schema");
        let trace = ws.join("trace.fozzy");
        std::fs::write(
            &trace,
            serde_json::to_vec_pretty(&sample_trace()).expect("trace bytes"),
        )
        .expect("write trace");
        let out_file = ws.join("timeline.json");

        let cfg = Config::default();
        let cmd = ProfileCommand::Timeline {
            run: trace.to_string_lossy().to_string(),
            out: Some(out_file.clone()),
            format: ProfileTimelineFormat::Json,
        };
        let stdout_doc = profile_command(&cfg, &cmd, true).expect("timeline");
        let file_doc: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out_file).expect("read timeline"))
                .expect("parse timeline");
        assert_eq!(stdout_doc, file_doc);
    }

    #[test]
    fn shrink_missing_trace_is_invalid_argument() {
        let cfg = Config::default();
        let cmd = ProfileCommand::Shrink {
            run: "missing.fozzy".to_string(),
            metric: ProfileMetric::P99Latency,
            direction: ProfileDirection::Increase,
            budget: None,
            minimize: ShrinkMinimize::All,
        };
        let err = profile_command(&cfg, &cmd, true).expect_err("must fail");
        match err {
            FozzyError::InvalidArgument(msg) => {
                assert!(msg.contains("no trace.fozzy found"), "message: {msg}");
            }
            other => panic!("expected invalid argument, got {other:?}"),
        }
    }

    #[test]
    fn format_metric_value_normalizes_negative_zero() {
        assert_eq!(format_metric_value(-0.0), "0");
        assert_eq!(format_metric_value(8.0), "8");
        assert_eq!(format_metric_value(8.125), "8.125");
    }

    #[test]
    fn shrink_contract_miss_returns_no_feasible_status() {
        let ws = temp_workspace("shrink-contract-miss");
        let trace = ws.join("c.trace.fozzy");
        std::fs::write(
            &trace,
            serde_json::to_vec_pretty(&sample_trace_without_heap()).expect("trace bytes"),
        )
        .expect("trace");

        let cfg = Config::default();
        let cmd = ProfileCommand::Shrink {
            run: trace.to_string_lossy().to_string(),
            metric: ProfileMetric::CpuTime,
            direction: ProfileDirection::Increase,
            budget: Some(crate::FozzyDuration(std::time::Duration::from_secs(1))),
            minimize: ShrinkMinimize::All,
        };
        let out = profile_command(&cfg, &cmd, true).expect("shrink output");
        assert_eq!(
            out.get("status").and_then(|v| v.as_str()),
            Some("no_feasible_shrink_found")
        );
        assert_eq!(out.get("preserved").and_then(|v| v.as_bool()), Some(false));
    }

    #[test]
    fn flame_reports_empty_domain_reason() {
        let ws = temp_workspace("flame-empty");
        let trace = ws.join("noheap.trace.fozzy");
        std::fs::write(
            &trace,
            serde_json::to_vec_pretty(&sample_trace_without_heap()).expect("trace bytes"),
        )
        .expect("trace");
        let out_file = ws.join("heap.folded.txt");

        let cfg = Config::default();
        let cmd = ProfileCommand::Flame {
            run: trace.to_string_lossy().to_string(),
            cpu: false,
            heap: true,
            out: Some(out_file.clone()),
            format: ProfileFlameFormat::Folded,
        };
        let out = profile_command(&cfg, &cmd, true).expect("flame");
        assert_eq!(out.get("empty").and_then(|v| v.as_bool()), Some(true));
        assert!(
            out.get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .contains("no heap samples")
        );
        let written = std::fs::read_to_string(out_file).expect("read output");
        assert!(!written.trim().is_empty());
    }

    #[test]
    fn profile_env_reports_schema_and_domains() {
        let cfg = Config::default();
        let out = profile_command(&cfg, &ProfileCommand::Env, true).expect("env");
        assert_eq!(
            out.get("schemaVersion").and_then(|v| v.as_str()),
            Some("fozzy.profile_env.v3")
        );
        assert!(out.get("domains").is_some());
    }

    #[test]
    fn strict_mode_allows_cpu_domain() {
        let ws = temp_workspace("cpu-strict");
        let trace = ws.join("trace.fozzy");
        std::fs::write(
            &trace,
            serde_json::to_vec_pretty(&sample_trace()).expect("trace bytes"),
        )
        .expect("trace");
        let cfg = Config::default();
        let cmd = ProfileCommand::Top {
            run: trace.to_string_lossy().to_string(),
            cpu: true,
            heap: false,
            latency: false,
            io: false,
            sched: false,
            limit: 5,
        };
        let out = profile_command(&cfg, &cmd, true).expect("top");
        assert!(
            out.get("cpu").is_some(),
            "cpu domain should be available in strict mode"
        );
    }

    #[test]
    fn heap_budget_findings_emitted() {
        let trace = sample_trace();
        let findings = heap_budget_findings_from_trace(
            &trace,
            &HeapBudgetPolicy {
                alloc_bytes_budget: Some(32),
                in_use_bytes_budget: Some(0),
            },
        );
        assert!(
            findings.iter().any(|f| f.title == "heap_alloc_budget"),
            "expected heap_alloc_budget finding"
        );
    }

    #[test]
    fn heap_diff_includes_callsite_metrics() {
        let left = sample_trace();
        let mut right = sample_trace();
        if let Some(event) = right.events.get_mut(1) {
            event
                .fields
                .insert("bytes".to_string(), serde_json::json!(256u64));
        }
        let left_timeline = build_profile_timeline(&left);
        let right_timeline = build_profile_timeline(&right);
        let left_cpu = build_cpu_profile(&left, &left_timeline);
        let right_cpu = build_cpu_profile(&right, &right_timeline);
        let left_heap = build_heap_profile(&left, &left_timeline);
        let right_heap = build_heap_profile(&right, &right_timeline);
        let left_latency = build_latency_profile(&left, &left_timeline);
        let right_latency = build_latency_profile(&right, &right_timeline);
        let left_metrics =
            build_profile_metrics(&left, &left_timeline, &left_cpu, &left_heap, &left_latency);
        let right_metrics = build_profile_metrics(
            &right,
            &right_timeline,
            &right_cpu,
            &right_heap,
            &right_latency,
        );
        let diff = compute_diff(
            "left",
            "right",
            &["heap".to_string()],
            &left_metrics,
            &right_metrics,
            Some(&left_heap),
            Some(&right_heap),
            &HashMap::new(),
            &HashMap::new(),
            1,
            1,
        );
        assert!(
            diff.regressions
                .iter()
                .any(|r| r.metric.contains("callsite:") && r.metric.contains("alloc_bytes")),
            "expected callsite alloc_bytes regression"
        );
    }

    #[test]
    fn profile_doctor_reports_schema() {
        let ws = temp_workspace("profile-doctor");
        let trace = ws.join("trace.fozzy");
        std::fs::write(
            &trace,
            serde_json::to_vec_pretty(&sample_trace()).expect("trace bytes"),
        )
        .expect("trace");

        let cfg = Config::default();
        let cmd = ProfileCommand::Doctor {
            run: trace.to_string_lossy().to_string(),
            deep: false,
        };
        let out = profile_command(&cfg, &cmd, true).expect("doctor");
        assert_eq!(
            out.get("schemaVersion").and_then(|v| v.as_str()),
            Some("fozzy.profile_doctor.v1")
        );
        assert!(out.get("checks").and_then(|v| v.as_array()).is_some());
    }

    #[test]
    fn relaxed_mode_returns_warning_for_missing_profile_inputs() {
        let cfg = Config::default();
        let cmd = ProfileCommand::Top {
            run: "missing.fozzy".to_string(),
            cpu: false,
            heap: true,
            latency: false,
            io: false,
            sched: false,
            limit: 5,
        };
        let out = profile_command(&cfg, &cmd, false).expect("relaxed warning");
        assert_eq!(
            out.get("schemaVersion").and_then(|v| v.as_str()),
            Some("fozzy.profile_contract_warning.v1")
        );
        assert_eq!(out.get("status").and_then(|v| v.as_str()), Some("warn"));
    }

    #[test]
    fn relaxed_mode_emits_cpu_warning() {
        let ws = temp_workspace("cpu-warn");
        let trace = ws.join("trace.fozzy");
        std::fs::write(
            &trace,
            serde_json::to_vec_pretty(&sample_trace()).expect("trace bytes"),
        )
        .expect("trace");
        let cfg = Config::default();
        let cmd = ProfileCommand::Top {
            run: trace.to_string_lossy().to_string(),
            cpu: true,
            heap: false,
            latency: false,
            io: false,
            sched: false,
            limit: 5,
        };
        let out = profile_command(&cfg, &cmd, false).expect("top");
        let warnings = out
            .get("warnings")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(!warnings.is_empty(), "expected warnings");
    }
}
