//! Deterministic profiler commands (`fozzy profile ...`).

use clap::Subcommand;
use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::{
    Config, FozzyError, FozzyResult, RunManifest, RunSummary, ShrinkMinimize, ShrinkOptions,
    TraceFile, TracePath, resolve_artifacts_dir, shrink_trace,
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
    #[serde(rename = "hostTimeSemantics")]
    pub host_time_semantics: String,
    #[serde(rename = "linuxPerfEventOpen")]
    pub linux_perf_event_open: bool,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyProfile {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    #[serde(rename = "runId")]
    pub run_id: String,
    pub distribution: LatencyDistribution,
    #[serde(rename = "criticalPath")]
    pub critical_path: Vec<CriticalPathEdge>,
    #[serde(rename = "waitReasons")]
    pub wait_reasons: Vec<ReasonCount>,
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
    pub domains: Vec<String>,
    #[serde(rename = "regressions")]
    pub regressions: Vec<RegressionFinding>,
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
    #[serde(rename = "confidence")]
    pub confidence: f64,
}

#[derive(Debug, Clone)]
struct ProfileBundle {
    artifacts_dir: PathBuf,
    timeline: Vec<ProfileEvent>,
    cpu: CpuProfile,
    heap: HeapProfile,
    latency: LatencyProfile,
    metrics: ProfileMetrics,
    symbols: SymbolsMap,
}

pub fn profile_command(
    config: &Config,
    command: &ProfileCommand,
    strict: bool,
) -> FozzyResult<serde_json::Value> {
    match command {
        ProfileCommand::Top {
            run,
            cpu,
            heap,
            latency,
            io,
            sched,
            limit,
        } => {
            let domains = normalize_domains(*cpu, *heap, *latency, *io, *sched);
            let bundle = load_profile_bundle(config, run)?;
            enforce_cpu_contract(strict, domains.contains(&"cpu".to_string()))?;
            let mut out = serde_json::Map::new();
            let mut empty_domains = Vec::<serde_json::Value>::new();
            out.insert(
                "schemaVersion".to_string(),
                serde_json::json!("fozzy.profile_top.v1"),
            );
            out.insert("run".to_string(), serde_json::json!(run));
            out.insert("limit".to_string(), serde_json::json!(limit));
            if domains.iter().any(|d| d == "cpu") {
                let top = bundle
                    .cpu
                    .folded_stacks
                    .iter()
                    .take(*limit)
                    .map(|s| {
                        serde_json::json!({
                            "stack": s.stack,
                            "weight": s.weight
                        })
                    })
                    .collect::<Vec<_>>();
                if top.is_empty() {
                    empty_domains.push(empty_domain("cpu", "no cpu samples in trace"));
                }
                out.insert("cpu".to_string(), serde_json::json!(top));
            }
            if domains.iter().any(|d| d == "heap") {
                let heap_rows = bundle
                    .heap
                    .hotspots
                    .iter()
                    .take(*limit)
                    .cloned()
                    .collect::<Vec<_>>();
                if heap_rows.is_empty() {
                    empty_domains.push(empty_domain("heap", "no heap samples in trace"));
                }
                out.insert("heap".to_string(), serde_json::to_value(heap_rows)?);
            }
            if domains.iter().any(|d| d == "latency") {
                let latency_rows = bundle
                    .latency
                    .critical_path
                    .iter()
                    .take(*limit)
                    .cloned()
                    .collect::<Vec<_>>();
                if latency_rows.is_empty() {
                    empty_domains.push(empty_domain("latency", "no latency edges in trace"));
                }
                out.insert("latency".to_string(), serde_json::to_value(latency_rows)?);
            }
            if domains.iter().any(|d| d == "io") {
                let io_top = top_by_tag(&bundle.timeline, ProfileEventKind::Io, *limit);
                if io_top.is_empty() {
                    empty_domains.push(empty_domain("io", "no io events in trace"));
                }
                out.insert("io".to_string(), serde_json::to_value(io_top)?);
            }
            if domains.iter().any(|d| d == "sched") {
                let sched_top = top_by_tag(&bundle.timeline, ProfileEventKind::Sched, *limit);
                if sched_top.is_empty() {
                    empty_domains.push(empty_domain("sched", "no scheduler events in trace"));
                }
                out.insert("sched".to_string(), serde_json::to_value(sched_top)?);
            }
            out.insert(
                "emptyDomains".to_string(),
                serde_json::to_value(empty_domains)?,
            );
            out.insert("metrics".to_string(), serde_json::to_value(bundle.metrics)?);
            Ok(serde_json::Value::Object(out))
        }
        ProfileCommand::Flame {
            run,
            cpu,
            heap,
            out,
            format,
        } => {
            let use_heap = *heap || !*cpu;
            let bundle = load_profile_bundle(config, run)?;
            if *cpu {
                enforce_cpu_contract(strict, true)?;
            }
            let folded = if use_heap {
                heap_folded(&bundle.heap)
            } else {
                bundle.cpu.folded_stacks.clone()
            };
            let domain = if use_heap { "heap" } else { "cpu" };
            let empty_reason = match domain {
                "heap" => "no heap samples in trace",
                _ => "no cpu samples in trace",
            };
            let payload = match format {
                ProfileFlameFormat::Folded => folded_to_text(&folded),
                ProfileFlameFormat::Svg => folded_to_svg(&folded),
                ProfileFlameFormat::Speedscope => {
                    serde_json::to_string_pretty(&folded_to_speedscope(run, &folded))?
                }
            };
            if let Some(path) = out {
                write_text(path, &payload)?;
            }
            Ok(serde_json::json!({
                "schemaVersion": "fozzy.profile_flame.v1",
                "run": run,
                "domain": domain,
                "empty": folded.is_empty(),
                "reason": if folded.is_empty() { Some(empty_reason) } else { None::<&str> },
                "format": format,
                "content": payload
            }))
        }
        ProfileCommand::Timeline { run, out, format } => {
            let bundle = load_profile_bundle(config, run)?;
            match format {
                ProfileTimelineFormat::Json => {
                    let payload = serde_json::json!({
                        "schemaVersion": "fozzy.profile_timeline.v1",
                        "run": run,
                        "format": "json",
                        "events": bundle.timeline
                    });
                    if let Some(path) = out {
                        write_json(path, &payload)?;
                    }
                    Ok(payload)
                }
                ProfileTimelineFormat::Html => {
                    let html = timeline_html(&bundle.timeline);
                    if let Some(path) = out {
                        write_text(path, &html)?;
                    }
                    Ok(serde_json::json!({
                        "schemaVersion": "fozzy.profile_timeline.v1",
                        "run": run,
                        "format": "html",
                        "content": html
                    }))
                }
            }
        }
        ProfileCommand::Diff {
            left,
            right,
            cpu,
            heap,
            latency,
            io,
            sched,
        } => {
            let domains = normalize_domains(*cpu, *heap, *latency, *io, *sched);
            if domains.iter().any(|d| d == "cpu") {
                enforce_cpu_contract(strict, true)?;
            }
            let l = load_profile_bundle(config, left)?;
            let r = load_profile_bundle(config, right)?;
            let diff = compute_diff(left, right, &domains, &l.metrics, &r.metrics);
            Ok(serde_json::to_value(diff)?)
        }
        ProfileCommand::Explain { run, diff_with } => {
            let base = load_profile_bundle(config, run)?;
            let explain = if let Some(right) = diff_with {
                let other = load_profile_bundle(config, right)?;
                explain_from_diff(run, right, &base.metrics, &other.metrics)
            } else {
                explain_single(run, &base)
            };
            Ok(serde_json::to_value(explain)?)
        }
        ProfileCommand::Export { run, format, out } => {
            let bundle = load_profile_bundle(config, run)?;
            let value = match format {
                ProfileExportFormat::Speedscope => {
                    serde_json::to_value(folded_to_speedscope(run, &bundle.cpu.folded_stacks))?
                }
                ProfileExportFormat::Pprof => serde_json::json!({
                    "schemaVersion": "fozzy.profile_pprof.v1",
                    "run": run,
                    "sampleType": "cpu",
                    "samples": bundle.cpu.samples,
                    "symbols": bundle.symbols,
                }),
                ProfileExportFormat::Otlp => serde_json::json!({
                    "schemaVersion": "fozzy.profile_otlp.v1",
                    "run": run,
                    "resource": {
                        "service.name": "fozzy",
                        "run.id": bundle.metrics.run_id,
                    },
                    "metrics": bundle.metrics,
                    "spans": bundle.timeline,
                }),
            };
            write_json(out, &value)?;
            Ok(serde_json::json!({
                "schemaVersion": "fozzy.profile_export_result.v1",
                "run": run,
                "format": format,
                "out": out,
            }))
        }
        ProfileCommand::Shrink {
            run,
            metric,
            direction,
            budget,
        } => {
            let (artifacts_dir, trace_path) = resolve_profile_trace(config, run)?;
            let input = TraceFile::read_json(&trace_path)?;
            let baseline = metric_value(*metric, &input)?;
            let shrunk = shrink_trace(
                config,
                TracePath::new(trace_path.clone()),
                &ShrinkOptions {
                    out_trace_path: None,
                    budget: budget.map(|b| b.0),
                    aggressive: false,
                    minimize: ShrinkMinimize::All,
                },
            )?;
            let shrunk_trace = TraceFile::read_json(Path::new(&shrunk.out_trace_path))?;
            let after = metric_value(*metric, &shrunk_trace)?;
            let preserved = match direction {
                ProfileDirection::Increase => after >= baseline,
                ProfileDirection::Decrease => after <= baseline,
            };
            let out_parent = Path::new(&shrunk.out_trace_path)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(artifacts_dir);
            write_profile_artifacts_from_trace(&shrunk_trace, &out_parent)?;
            let direction_name = match direction {
                ProfileDirection::Increase => "increase",
                ProfileDirection::Decrease => "decrease",
            };
            let comparator = match direction {
                ProfileDirection::Increase => ">=",
                ProfileDirection::Decrease => "<=",
            };
            let status = if preserved {
                "ok"
            } else {
                "no_feasible_shrink_found"
            };
            let baseline_out = normalize_metric_value(baseline);
            let after_out = normalize_metric_value(after);
            Ok(serde_json::json!({
                "schemaVersion": "fozzy.profile_shrink.v1",
                "status": status,
                "run": run,
                "trace": trace_path,
                "outTrace": shrunk.out_trace_path,
                "metric": metric,
                "direction": direction,
                "baseline": baseline_out,
                "after": after_out,
                "preserved": preserved,
                "contract": {
                    "expected": format!("after {comparator} baseline"),
                    "direction": direction_name,
                },
                "reason": if preserved {
                    None::<String>
                } else {
                    Some(format!(
                        "no feasible shrink found that preserves metric direction: expected after {comparator} baseline for direction={direction_name} (baseline={}, after={})",
                        format_metric_value(baseline),
                        format_metric_value(after)
                    ))
                },
            }))
        }
        ProfileCommand::Env => Ok(profile_env_report(config, strict)),
        ProfileCommand::Doctor { run } => profile_doctor(config, strict, run),
    }
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
    let symbols = build_symbols_map(trace, &timeline);
    let metrics = build_profile_metrics(trace, &timeline, &cpu, &heap, &latency);

    write_json(&artifacts_dir.join("profile.timeline.json"), &timeline)?;
    write_json(&artifacts_dir.join("profile.cpu.json"), &cpu)?;
    write_json(&artifacts_dir.join("profile.heap.json"), &heap)?;
    write_json(&artifacts_dir.join("profile.latency.json"), &latency)?;
    write_json(&artifacts_dir.join("profile.metrics.json"), &metrics)?;
    write_json(&artifacts_dir.join("symbols.json"), &symbols)?;
    Ok(())
}

fn load_profile_bundle(config: &Config, selector: &str) -> FozzyResult<ProfileBundle> {
    let (artifacts_dir, trace_path) = resolve_profile_artifacts(config, selector)?;
    if let Some(trace_path) = trace_path {
        let trace = TraceFile::read_json(&trace_path)?;
        write_profile_artifacts_from_trace(&trace, &artifacts_dir)?;
    } else if !profile_artifacts_exist(&artifacts_dir) {
        return Err(FozzyError::InvalidArgument(format!(
            "no trace.fozzy found for {selector:?}; profiler requires trace artifacts"
        )));
    }

    let timeline: Vec<ProfileEvent> =
        serde_json::from_slice(&std::fs::read(artifacts_dir.join("profile.timeline.json"))?)?;
    let cpu: CpuProfile =
        serde_json::from_slice(&std::fs::read(artifacts_dir.join("profile.cpu.json"))?)?;
    let heap: HeapProfile =
        serde_json::from_slice(&std::fs::read(artifacts_dir.join("profile.heap.json"))?)?;
    let latency: LatencyProfile =
        serde_json::from_slice(&std::fs::read(artifacts_dir.join("profile.latency.json"))?)?;
    let metrics: ProfileMetrics =
        serde_json::from_slice(&std::fs::read(artifacts_dir.join("profile.metrics.json"))?)?;
    let symbols: SymbolsMap =
        serde_json::from_slice(&std::fs::read(artifacts_dir.join("symbols.json"))?)?;

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

fn build_profile_timeline(trace: &TraceFile) -> Vec<ProfileEvent> {
    let run_id = trace.summary.identity.run_id.clone();
    let seed = trace.summary.identity.seed;
    let mut out = Vec::new();
    for (idx, event) in trace.events.iter().enumerate() {
        let kind = map_event_kind(&event.name);
        let t_next = trace.events.get(idx + 1).map(|n| n.time_ms);
        let duration = t_next.and_then(|n| n.checked_sub(event.time_ms));
        let mut tags = BTreeMap::new();
        tags.insert("name".to_string(), event.name.clone());
        for (k, v) in &event.fields {
            match v {
                serde_json::Value::String(s) => {
                    tags.insert(k.clone(), s.clone());
                }
                serde_json::Value::Number(n) => {
                    tags.insert(k.clone(), n.to_string());
                }
                serde_json::Value::Bool(b) => {
                    tags.insert(k.clone(), b.to_string());
                }
                _ => {}
            }
        }
        let bytes = event
            .fields
            .get("bytes")
            .and_then(|v| v.as_u64())
            .or_else(|| event.fields.get("payload_size").and_then(|v| v.as_u64()));
        let task = event
            .fields
            .get("task")
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        out.push(ProfileEvent {
            t_virtual: event.time_ms,
            t_mono: Some(idx as u64),
            kind,
            run_id: run_id.clone(),
            seed,
            thread: event
                .fields
                .get("thread")
                .and_then(|v| v.as_str())
                .unwrap_or("main")
                .to_string(),
            task,
            span_id: format!("e-{idx}"),
            parent_span_id: if idx > 0 {
                Some(format!("e-{}", idx - 1))
            } else {
                None
            },
            tags,
            cost: ProfileCost {
                duration_ms: duration,
                bytes,
                count: Some(1),
            },
        });
    }
    out
}

fn map_event_kind(name: &str) -> ProfileEventKind {
    match name {
        "memory_alloc" => ProfileEventKind::Alloc,
        "memory_free" => ProfileEventKind::Free,
        "http_request" | "proc_spawn" => ProfileEventKind::Io,
        "net_drop" | "net_deliver" => ProfileEventKind::Net,
        "deliver" | "partition" | "heal" | "crash" | "restart" => ProfileEventKind::Sched,
        _ => ProfileEventKind::Event,
    }
}

fn build_cpu_profile(trace: &TraceFile, timeline: &[ProfileEvent]) -> CpuProfile {
    let mut stacks = HashMap::<String, u64>::new();
    let mut samples = Vec::new();
    for event in timeline {
        let stack_parts = vec![
            "fozzy::runtime".to_string(),
            format!(
                "event::{}",
                event.tags.get("name").cloned().unwrap_or_default()
            ),
        ];
        let stack = stack_parts.join(";");
        let weight = event.cost.duration_ms.unwrap_or(1).max(1);
        *stacks.entry(stack.clone()).or_insert(0) += weight;
        samples.push(CpuSample {
            thread: event.thread.clone(),
            stack: stack_parts,
            weight_ms: weight,
        });
    }

    let mut folded_stacks: Vec<FoldedStack> = stacks
        .into_iter()
        .map(|(stack, weight)| FoldedStack { stack, weight })
        .collect();
    folded_stacks.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.stack.cmp(&b.stack)));

    CpuProfile {
        schema_version: "fozzy.profile_cpu.v1".to_string(),
        run_id: trace.summary.identity.run_id.clone(),
        collector: CpuCollectorInfo {
            domain: "host_time".to_string(),
            primary_collector: "perf_event_open".to_string(),
            fallback_collector: "in_process_sampler".to_string(),
            host_time_semantics: "host-time CPU samples are not replay-deterministic; compare across repeated deterministic replays".to_string(),
            linux_perf_event_open: cfg!(target_os = "linux"),
        },
        sample_period_ms: 1,
        sample_count: samples.len(),
        samples,
        folded_stacks,
        symbols_ref: "symbols.json".to_string(),
    }
}

fn build_heap_profile(trace: &TraceFile, timeline: &[ProfileEvent]) -> HeapProfile {
    #[derive(Clone)]
    struct LiveAlloc {
        bytes: u64,
        callsite_hash: String,
        start: u64,
        end: Option<u64>,
    }

    let mut live = HashMap::<u64, LiveAlloc>::new();
    let mut completed: Vec<LiveAlloc> = Vec::new();

    for event in timeline {
        if event.kind == ProfileEventKind::Alloc {
            let alloc_id = event
                .tags
                .get("alloc_id")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let failed = event
                .tags
                .get("failed_reason")
                .is_some_and(|r| !r.is_empty() && r != "null");
            if failed || alloc_id == 0 {
                continue;
            }
            let callsite = event
                .tags
                .get("callsite_hash")
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            let bytes = event.cost.bytes.unwrap_or(0);
            live.insert(
                alloc_id,
                LiveAlloc {
                    bytes,
                    callsite_hash: callsite,
                    start: event.t_virtual,
                    end: None,
                },
            );
        } else if event.kind == ProfileEventKind::Free {
            let alloc_id = event
                .tags
                .get("alloc_id")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            if let Some(mut alloc) = live.remove(&alloc_id) {
                alloc.end = Some(event.t_virtual);
                completed.push(alloc);
            }
        }
    }

    let mut hotspots = HashMap::<String, HeapCallsite>::new();
    let mut total_alloc_bytes = 0u64;
    for alloc in live.values().chain(completed.iter()) {
        total_alloc_bytes = total_alloc_bytes.saturating_add(alloc.bytes);
        let entry = hotspots
            .entry(alloc.callsite_hash.clone())
            .or_insert(HeapCallsite {
                callsite_hash: alloc.callsite_hash.clone(),
                alloc_count: 0,
                alloc_bytes: 0,
                in_use_bytes: 0,
            });
        entry.alloc_count = entry.alloc_count.saturating_add(1);
        entry.alloc_bytes = entry.alloc_bytes.saturating_add(alloc.bytes);
        if alloc.end.is_none() {
            entry.in_use_bytes = entry.in_use_bytes.saturating_add(alloc.bytes);
        }
    }

    let mut hotspot_list: Vec<HeapCallsite> = hotspots.into_values().collect();
    hotspot_list.sort_by(|a, b| {
        b.in_use_bytes
            .cmp(&a.in_use_bytes)
            .then_with(|| b.alloc_bytes.cmp(&a.alloc_bytes))
            .then_with(|| a.callsite_hash.cmp(&b.callsite_hash))
    });

    let end_t = timeline.last().map(|e| e.t_virtual).unwrap_or(0);
    let mut bins = BTreeMap::<String, u64>::new();
    let mut suspects = Vec::<RetentionSuspect>::new();

    for (alloc_id, alloc) in &live {
        let age = end_t.saturating_sub(alloc.start);
        suspects.push(RetentionSuspect {
            alloc_id: *alloc_id,
            callsite_hash: alloc.callsite_hash.clone(),
            bytes: alloc.bytes,
            age_ms: age,
        });
    }
    suspects.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| b.age_ms.cmp(&a.age_ms)));

    for alloc in completed {
        let d = alloc.end.unwrap_or(alloc.start).saturating_sub(alloc.start);
        let bucket = if d <= 1 {
            "0-1ms"
        } else if d <= 10 {
            "2-10ms"
        } else if d <= 100 {
            "11-100ms"
        } else {
            "101ms+"
        };
        *bins.entry(bucket.to_string()).or_insert(0) += 1;
    }

    let lifetime_histogram = bins
        .into_iter()
        .map(|(bucket, count)| HistogramBin { bucket, count })
        .collect::<Vec<_>>();

    let in_use_bytes = live
        .values()
        .fold(0u64, |acc, a| acc.saturating_add(a.bytes));
    let span_s = (end_t.max(1) as f64) / 1000.0;
    let alloc_rate_per_sec = (total_alloc_bytes as f64) / span_s;

    let trace_memory_in_use = trace
        .memory
        .as_ref()
        .map(|m| m.summary.in_use_bytes)
        .unwrap_or(0);

    HeapProfile {
        schema_version: "fozzy.profile_heap.v1".to_string(),
        run_id: trace.summary.identity.run_id.clone(),
        total_alloc_bytes,
        in_use_bytes: in_use_bytes.max(trace_memory_in_use),
        alloc_rate_per_sec,
        hotspots: hotspot_list,
        lifetime_histogram,
        retention_suspects: suspects,
    }
}

fn build_latency_profile(trace: &TraceFile, timeline: &[ProfileEvent]) -> LatencyProfile {
    let mut deltas = Vec::<u64>::new();
    let mut critical_path = Vec::<CriticalPathEdge>::new();
    let mut reasons = BTreeMap::<String, u64>::new();

    for pair in timeline.windows(2) {
        let left = &pair[0];
        let right = &pair[1];
        let d = right.t_virtual.saturating_sub(left.t_virtual);
        deltas.push(d);
        let reason = match right.kind {
            ProfileEventKind::Io => "io",
            ProfileEventKind::Sched => "sched",
            ProfileEventKind::Alloc | ProfileEventKind::Free => "heap",
            ProfileEventKind::Net => "payload",
            ProfileEventKind::Sample => "cpu",
            _ => "other",
        }
        .to_string();
        *reasons.entry(reason.clone()).or_insert(0) += 1;
        critical_path.push(CriticalPathEdge {
            from_span: left.span_id.clone(),
            to_span: right.span_id.clone(),
            duration_ms: d,
            reason,
        });
    }

    critical_path.sort_by(|a, b| {
        b.duration_ms
            .cmp(&a.duration_ms)
            .then_with(|| a.from_span.cmp(&b.from_span))
    });

    let distribution = if deltas.is_empty() {
        LatencyDistribution {
            count: 0,
            p50_ms: 0,
            p95_ms: 0,
            p99_ms: 0,
            max_ms: 0,
            variance: 0.0,
        }
    } else {
        deltas.sort_unstable();
        let max_ms = *deltas.last().unwrap_or(&0);
        let p50_ms = percentile(&deltas, 0.50);
        let p95_ms = percentile(&deltas, 0.95);
        let p99_ms = percentile(&deltas, 0.99);
        let mean = deltas.iter().copied().map(|v| v as f64).sum::<f64>() / (deltas.len() as f64);
        let variance = deltas
            .iter()
            .map(|v| {
                let d = (*v as f64) - mean;
                d * d
            })
            .sum::<f64>()
            / (deltas.len() as f64);
        LatencyDistribution {
            count: deltas.len(),
            p50_ms,
            p95_ms,
            p99_ms,
            max_ms,
            variance,
        }
    };

    let wait_reasons = reasons
        .into_iter()
        .map(|(reason, count)| ReasonCount { reason, count })
        .collect();

    LatencyProfile {
        schema_version: "fozzy.profile_latency.v1".to_string(),
        run_id: trace.summary.identity.run_id.clone(),
        distribution,
        critical_path,
        wait_reasons,
    }
}

fn build_symbols_map(trace: &TraceFile, timeline: &[ProfileEvent]) -> SymbolsMap {
    let mut symbols = timeline
        .iter()
        .filter_map(|e| e.tags.get("name").cloned())
        .collect::<Vec<_>>();
    symbols.sort();
    symbols.dedup();
    SymbolsMap {
        schema_version: "fozzy.profile_symbols.v1".to_string(),
        run_id: trace.summary.identity.run_id.clone(),
        modules: vec![SymbolModule {
            name: "fozzy-runtime".to_string(),
            build_id: format!(
                "{}-{}",
                trace.engine.version,
                trace.engine.commit.as_deref().unwrap_or("dev")
            ),
            symbols,
        }],
    }
}

fn build_profile_metrics(
    trace: &TraceFile,
    timeline: &[ProfileEvent],
    cpu: &CpuProfile,
    heap: &HeapProfile,
    latency: &LatencyProfile,
) -> ProfileMetrics {
    let virtual_time_ms = timeline.last().map(|e| e.t_virtual).unwrap_or(0);
    let host_time_ms = trace.summary.duration_ms;
    let cpu_time_ms = cpu
        .folded_stacks
        .iter()
        .fold(0u64, |acc, s| acc.saturating_add(s.weight));
    let io_ops = timeline
        .iter()
        .filter(|e| e.kind == ProfileEventKind::Io || e.kind == ProfileEventKind::Net)
        .count() as u64;
    let sched_ops = timeline
        .iter()
        .filter(|e| e.kind == ProfileEventKind::Sched)
        .count() as u64;
    ProfileMetrics {
        schema_version: "fozzy.profile_metrics.v1".to_string(),
        run_id: trace.summary.identity.run_id.clone(),
        virtual_time_ms,
        host_time_ms,
        cpu_time_ms,
        alloc_bytes: heap.total_alloc_bytes,
        in_use_bytes: heap.in_use_bytes,
        p50_latency_ms: latency.distribution.p50_ms,
        p95_latency_ms: latency.distribution.p95_ms,
        p99_latency_ms: latency.distribution.p99_ms,
        max_latency_ms: latency.distribution.max_ms,
        io_ops,
        sched_ops,
        confidence: if host_time_ms == 0 {
            Some(0.0)
        } else {
            Some(0.8)
        },
    }
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
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

fn heap_folded(heap: &HeapProfile) -> Vec<FoldedStack> {
    let mut out = heap
        .hotspots
        .iter()
        .map(|h| FoldedStack {
            stack: format!("fozzy::heap;callsite::{}", h.callsite_hash),
            weight: h.alloc_bytes.max(1),
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.stack.cmp(&b.stack)));
    out
}

fn folded_to_text(folded: &[FoldedStack]) -> String {
    if folded.is_empty() {
        return "# empty profile: no samples in trace".to_string();
    }
    let mut out = String::new();
    for row in folded {
        out.push_str(&format!("{} {}\n", row.stack, row.weight));
    }
    out.trim_end().to_string()
}

fn folded_to_svg(folded: &[FoldedStack]) -> String {
    let width = 900;
    let bar_h = 18;
    let gap = 4;
    let max = folded.iter().map(|f| f.weight).max().unwrap_or(1) as f64;
    let height = (folded.len() as i32) * (bar_h + gap) + 40;
    let mut out = String::new();
    out.push_str(&format!(
        r#"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\">"#
    ));
    out.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#111827\"/>");
    if folded.is_empty() {
        out.push_str(
            "<text x=\"24\" y=\"36\" fill=\"#e5e7eb\" font-size=\"13\">empty profile: no samples in trace</text>",
        );
    }
    for (i, row) in folded.iter().enumerate() {
        let y = 20 + (i as i32) * (bar_h + gap);
        let w = ((row.weight as f64 / max) * 820.0).round() as i32;
        out.push_str(&format!(
            "<rect x=\"20\" y=\"{y}\" width=\"{w}\" height=\"{bar_h}\" fill=\"#2563eb\"/>"
        ));
        out.push_str(&format!(
            "<text x=\"{x}\" y=\"{ty}\" fill=\"#e5e7eb\" font-size=\"12\">{label}</text>",
            x = 24,
            ty = y + 13,
            label = escape_xml(&format!("{} ({})", row.stack, row.weight)),
        ));
    }
    out.push_str("</svg>");
    out
}

fn folded_to_speedscope(run: &str, folded: &[FoldedStack]) -> serde_json::Value {
    let mut frames: Vec<serde_json::Value> = vec![];
    let mut frame_index = BTreeMap::<String, usize>::new();
    let mut samples = Vec::<Vec<usize>>::new();
    let mut weights = Vec::<u64>::new();

    for row in folded {
        let mut stack = Vec::<usize>::new();
        for frame in row.stack.split(';') {
            let idx = if let Some(i) = frame_index.get(frame) {
                *i
            } else {
                let i = frames.len();
                frames.push(serde_json::json!({"name": frame}));
                frame_index.insert(frame.to_string(), i);
                i
            };
            stack.push(idx);
        }
        samples.push(stack);
        weights.push(row.weight);
    }

    serde_json::json!({
        "$schema": "https://www.speedscope.app/file-format-schema.json",
        "shared": {"frames": frames},
        "profiles": [{
            "type": "sampled",
            "name": format!("fozzy profile {run}"),
            "unit": "milliseconds",
            "startValue": 0,
            "endValue": weights.iter().copied().sum::<u64>(),
            "samples": samples,
            "weights": weights,
        }],
        "activeProfileIndex": 0,
        "exporter": "fozzy",
    })
}

fn timeline_html(events: &[ProfileEvent]) -> String {
    let mut rows = String::new();
    for e in events {
        rows.push_str(&format!(
            "<tr><td>{}</td><td>{:?}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            e.t_virtual,
            e.kind,
            e.thread,
            escape_xml(&e.span_id),
            escape_xml(e.tags.get("name").map(|s| s.as_str()).unwrap_or("")),
        ));
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Fozzy Profile Timeline</title><style>body{{font-family:ui-monospace,Menlo,monospace;background:#0b1020;color:#e5e7eb;padding:20px}}table{{border-collapse:collapse;width:100%}}th,td{{padding:6px 8px;border-bottom:1px solid #1f2937;text-align:left}}</style></head><body><h1>Fozzy Profile Timeline</h1><table><thead><tr><th>t_virtual</th><th>kind</th><th>thread</th><th>span_id</th><th>name</th></tr></thead><tbody>{rows}</tbody></table></body></html>"
    )
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn compute_diff(
    left: &str,
    right: &str,
    domains: &[String],
    l: &ProfileMetrics,
    r: &ProfileMetrics,
) -> ProfileDiff {
    let mut regressions = Vec::<RegressionFinding>::new();

    for domain in domains {
        let pairs: Vec<(&str, f64, f64)> = match domain.as_str() {
            "cpu" => vec![("cpu_time_ms", l.cpu_time_ms as f64, r.cpu_time_ms as f64)],
            "heap" => vec![
                ("alloc_bytes", l.alloc_bytes as f64, r.alloc_bytes as f64),
                ("in_use_bytes", l.in_use_bytes as f64, r.in_use_bytes as f64),
            ],
            "latency" => vec![
                (
                    "p95_latency_ms",
                    l.p95_latency_ms as f64,
                    r.p95_latency_ms as f64,
                ),
                (
                    "p99_latency_ms",
                    l.p99_latency_ms as f64,
                    r.p99_latency_ms as f64,
                ),
                (
                    "max_latency_ms",
                    l.max_latency_ms as f64,
                    r.max_latency_ms as f64,
                ),
            ],
            "io" => vec![("io_ops", l.io_ops as f64, r.io_ops as f64)],
            "sched" => vec![("sched_ops", l.sched_ops as f64, r.sched_ops as f64)],
            _ => Vec::new(),
        };
        for (metric, lv, rv) in pairs {
            let delta = rv - lv;
            let delta_pct = if lv.abs() < f64::EPSILON {
                if rv.abs() < f64::EPSILON { 0.0 } else { 100.0 }
            } else {
                (delta / lv) * 100.0
            };
            regressions.push(RegressionFinding {
                domain: domain.clone(),
                metric: metric.to_string(),
                left_value: lv,
                right_value: rv,
                delta,
                delta_pct,
                confidence: 0.8,
            });
        }
    }

    regressions.sort_by(|a, b| {
        b.delta
            .abs()
            .partial_cmp(&a.delta.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.metric.cmp(&b.metric))
    });

    ProfileDiff {
        schema_version: "fozzy.profile_diff.v1".to_string(),
        left: left.to_string(),
        right: right.to_string(),
        domains: domains.to_vec(),
        regressions,
    }
}

fn explain_single(run: &str, bundle: &ProfileBundle) -> ProfileExplain {
    let top_path = bundle
        .latency
        .critical_path
        .first()
        .map(|p| format!("{} -> {} ({}ms)", p.from_span, p.to_span, p.duration_ms))
        .unwrap_or_else(|| "no critical path edges".to_string());

    let domain = if bundle.metrics.p99_latency_ms > 0 {
        "latency"
    } else if bundle.metrics.alloc_bytes > 0 {
        "heap"
    } else {
        "io"
    };

    ProfileExplain {
        schema_version: "fozzy.profile_explain.v1".to_string(),
        run: run.to_string(),
        regression_statement: format!(
            "run {} shows p99={}ms, alloc_bytes={}, io_ops={}, sched_ops={}",
            bundle.metrics.run_id,
            bundle.metrics.p99_latency_ms,
            bundle.metrics.alloc_bytes,
            bundle.metrics.io_ops,
            bundle.metrics.sched_ops
        ),
        top_shifted_path: top_path,
        likely_cause_domain: domain.to_string(),
        evidence_pointers: vec![
            format!("{}/profile.metrics.json", bundle.artifacts_dir.display()),
            format!("{}/profile.latency.json", bundle.artifacts_dir.display()),
            format!("{}/profile.heap.json", bundle.artifacts_dir.display()),
        ],
    }
}

fn explain_from_diff(
    left: &str,
    right: &str,
    l: &ProfileMetrics,
    r: &ProfileMetrics,
) -> ProfileExplain {
    let diff = compute_diff(
        left,
        right,
        &[
            "cpu".to_string(),
            "heap".to_string(),
            "latency".to_string(),
            "io".to_string(),
            "sched".to_string(),
        ],
        l,
        r,
    );
    let top = diff.regressions.first();
    let (statement, path, domain) = if let Some(top) = top {
        (
            format!(
                "{} {} changed from {:.2} to {:.2} ({:+.2}%)",
                top.domain, top.metric, top.left_value, top.right_value, top.delta_pct
            ),
            format!("metric::{}", top.metric),
            top.domain.clone(),
        )
    } else {
        (
            "no measurable regression shift found".to_string(),
            "n/a".to_string(),
            "unknown".to_string(),
        )
    };

    ProfileExplain {
        schema_version: "fozzy.profile_explain.v1".to_string(),
        run: left.to_string(),
        regression_statement: statement,
        top_shifted_path: path,
        likely_cause_domain: domain,
        evidence_pointers: vec![
            "profile.metrics.json".to_string(),
            "profile.latency.json".to_string(),
            "profile.cpu.json".to_string(),
            "profile.heap.json".to_string(),
        ],
    }
}

fn metric_value(metric: ProfileMetric, trace: &TraceFile) -> FozzyResult<f64> {
    let timeline = build_profile_timeline(trace);
    let cpu = build_cpu_profile(trace, &timeline);
    let heap = build_heap_profile(trace, &timeline);
    let latency = build_latency_profile(trace, &timeline);
    let value = match metric {
        ProfileMetric::P99Latency => latency.distribution.p99_ms as f64,
        ProfileMetric::CpuTime => cpu.folded_stacks.iter().map(|f| f.weight as f64).sum(),
        ProfileMetric::AllocBytes => heap.total_alloc_bytes as f64,
    };
    Ok(value)
}

fn format_metric_value(value: f64) -> String {
    let normalized = if value == 0.0 { 0.0 } else { value };
    let mut out = format!("{normalized:.6}");
    while out.contains('.') && out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    out
}

fn normalize_metric_value(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn empty_domain(domain: &str, reason: &str) -> serde_json::Value {
    serde_json::json!({
        "domain": domain,
        "empty": true,
        "reason": reason,
    })
}

fn profile_env_report(config: &Config, strict: bool) -> serde_json::Value {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let cpu_quality = if cfg!(target_os = "linux") {
        "high"
    } else {
        "degraded"
    };
    serde_json::json!({
        "schemaVersion": "fozzy.profile_env.v1",
        "strict": strict,
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
                "notes": if cfg!(target_os = "linux") {
                    "linux perf_event_open available in collector metadata"
                } else {
                    "non-Linux uses fallback synthetic/in-process sampling semantics"
                }
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

fn profile_doctor(config: &Config, strict: bool, run: &str) -> FozzyResult<serde_json::Value> {
    let mut checks = Vec::<serde_json::Value>::new();
    let mut issues = Vec::<String>::new();
    checks.push(serde_json::json!({
        "name": "env",
        "ok": true,
        "status": "pass",
        "detail": profile_env_report(config, strict),
    }));

    let bundle = match load_profile_bundle(config, run) {
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
    let top_has_any = !top_by_tag(&bundle.timeline, ProfileEventKind::Io, 10).is_empty()
        || !top_by_tag(&bundle.timeline, ProfileEventKind::Sched, 10).is_empty()
        || !bundle.heap.hotspots.is_empty()
        || !bundle.latency.critical_path.is_empty();
    checks.push(serde_json::json!({
        "name": "top",
        "ok": true,
        "status": if top_has_any { "pass" } else { "warn" },
        "detail": format!("default domains={top_domains:?}"),
    }));

    let heap_folded = heap_folded(&bundle.heap);
    checks.push(serde_json::json!({
        "name": "flame_heap",
        "ok": true,
        "status": if heap_folded.is_empty() { "warn" } else { "pass" },
        "detail": if heap_folded.is_empty() { "no heap samples in trace" } else { "heap flame data present" },
    }));
    checks.push(serde_json::json!({
        "name": "flame_cpu",
        "ok": true,
        "status": if bundle.cpu.folded_stacks.is_empty() { "warn" } else { "pass" },
        "detail": if bundle.cpu.folded_stacks.is_empty() { "no cpu samples in trace" } else { "cpu flame data present" },
    }));

    checks.push(serde_json::json!({
        "name": "timeline",
        "ok": true,
        "status": "pass",
        "detail": format!("events={}", bundle.timeline.len()),
    }));
    let diff = compute_diff(
        run,
        run,
        &["cpu".to_string(), "heap".to_string(), "latency".to_string()],
        &bundle.metrics,
        &bundle.metrics,
    );
    checks.push(serde_json::json!({
        "name": "diff",
        "ok": true,
        "status": "pass",
        "detail": format!("regressions={}", diff.regressions.len()),
    }));
    let explain = explain_single(run, &bundle);
    checks.push(serde_json::json!({
        "name": "explain",
        "ok": true,
        "status": "pass",
        "detail": explain.likely_cause_domain,
    }));
    let speedscope = folded_to_speedscope(run, &bundle.cpu.folded_stacks);
    checks.push(serde_json::json!({
        "name": "export",
        "ok": true,
        "status": "pass",
        "detail": format!("speedscope_frames={}", speedscope.get("shared").and_then(|v| v.get("frames")).and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0)),
    }));

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
                    let baseline =
                        metric_value(ProfileMetric::CpuTime, &TraceFile::read_json(&trace_path)?)?;
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
        let dir = input
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
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
    if strict && cpu_requested {
        return Err(FozzyError::InvalidArgument(
            "strict profile contract forbids CPU domain because host-time CPU samples are not replay-deterministic; rerun with --unsafe to opt out"
                .to_string(),
        ));
    }
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
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
        );
        let diff_b = compute_diff(
            "a",
            "b",
            &["latency".to_string(), "heap".to_string()],
            &metrics,
            &metrics,
        );
        assert_eq!(
            serde_json::to_string(&diff_a).expect("json"),
            serde_json::to_string(&diff_b).expect("json")
        );
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
            Some("fozzy.profile_env.v1")
        );
        assert!(out.get("domains").is_some());
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
        };
        let out = profile_command(&cfg, &cmd, true).expect("doctor");
        assert_eq!(
            out.get("schemaVersion").and_then(|v| v.as_str()),
            Some("fozzy.profile_doctor.v1")
        );
        assert!(out.get("checks").and_then(|v| v.as_array()).is_some());
    }
}
