use clap::{Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TopologyProfile {
    Balanced,
    Pedantic,
    Overkill,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShrinkCoveragePolicy {
    FailureOnly,
    ExercisedOk,
    NoKnownFailures,
}

#[derive(Debug, Subcommand)]
pub enum MapCommand {
    /// Analyze repository hotspots and risk-ranked candidate areas for granular suites
    Hotspots {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 60)]
        min_risk: u8,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Discover service/module boundaries from language-agnostic repo signals
    Services {
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Build suite recommendations and scenario-coverage gaps for high-risk hotspots
    Suites {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value = "tests")]
        scenario_root: PathBuf,
        #[arg(long, default_value_t = 60)]
        min_risk: u8,
        #[arg(long, default_value = "pedantic")]
        profile: TopologyProfile,
        #[arg(long, default_value = "no-known-failures")]
        shrink_policy: ShrinkCoveragePolicy,
        #[arg(long, default_value_t = 100)]
        limit: usize,
        #[arg(long, default_value_t = 0)]
        offset: usize,
        #[arg(long, default_value_t = 25)]
        max_matched_scenarios: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapHotspotsReport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub root: String,
    #[serde(rename = "scannedFiles")]
    pub scanned_files: usize,
    #[serde(rename = "minRisk")]
    pub min_risk: u8,
    pub hotspots: Vec<MapHotspot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapServicesReport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub root: String,
    #[serde(rename = "scannedFiles")]
    pub scanned_files: usize,
    pub services: Vec<ServiceBoundary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSuitesReport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub root: String,
    #[serde(rename = "scenarioRoot")]
    pub scenario_root: String,
    #[serde(rename = "scannedFiles")]
    pub scanned_files: usize,
    pub profile: TopologyProfile,
    #[serde(rename = "shrinkPolicy")]
    pub shrink_policy: ShrinkCoveragePolicy,
    #[serde(rename = "baseMinRisk")]
    pub base_min_risk: u8,
    #[serde(rename = "effectiveMinRisk")]
    pub effective_min_risk: u8,
    #[serde(rename = "scenarioCount")]
    pub scenario_count: usize,
    #[serde(
        rename = "skippedSourceFiles",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub skipped_source_files: Vec<String>,
    #[serde(
        rename = "unreadableScenarios",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub unreadable_scenarios: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(rename = "requiredHotspotCount")]
    pub required_hotspot_count: usize,
    #[serde(rename = "coveredHotspotCount")]
    pub covered_hotspot_count: usize,
    #[serde(rename = "uncoveredHotspotCount")]
    pub uncovered_hotspot_count: usize,
    #[serde(rename = "totalSuites")]
    pub total_suites: usize,
    #[serde(rename = "returnedSuites")]
    pub returned_suites: usize,
    pub offset: usize,
    pub limit: usize,
    pub truncated: bool,
    pub suites: Vec<SuiteRecommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapHotspot {
    pub id: String,
    pub component: String,
    pub path: String,
    #[serde(rename = "riskScore")]
    pub risk_score: u8,
    pub reasons: Vec<String>,
    pub signals: HotspotSignals,
    #[serde(rename = "recommendedSuites")]
    pub recommended_suites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HotspotSignals {
    #[serde(rename = "lineCount")]
    pub line_count: usize,
    #[serde(rename = "branchSignals")]
    pub branch_signals: usize,
    #[serde(rename = "concurrencySignals")]
    pub concurrency_signals: usize,
    #[serde(rename = "externalSignals")]
    pub external_signals: usize,
    #[serde(rename = "failureSignals")]
    pub failure_signals: usize,
    #[serde(rename = "memorySignals")]
    pub memory_signals: usize,
    #[serde(rename = "entrypointSignals")]
    pub entrypoint_signals: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceBoundary {
    pub name: String,
    pub path: String,
    pub kind: String,
    #[serde(rename = "fileCount")]
    pub file_count: usize,
    #[serde(rename = "entrypointSignals")]
    pub entrypoint_signals: usize,
    #[serde(rename = "externalSignals")]
    pub external_signals: usize,
    #[serde(rename = "concurrencySignals")]
    pub concurrency_signals: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteRecommendation {
    #[serde(rename = "hotspotId")]
    pub hotspot_id: String,
    pub component: String,
    pub path: String,
    #[serde(rename = "riskScore")]
    pub risk_score: u8,
    #[serde(rename = "requiredByPolicy")]
    pub required_by_policy: bool,
    pub covered: bool,
    #[serde(rename = "coverageHints")]
    pub coverage_hints: Vec<String>,
    #[serde(rename = "requiredSuites")]
    pub required_suites: Vec<String>,
    #[serde(rename = "coveredSuites")]
    pub covered_suites: Vec<String>,
    #[serde(rename = "coverageEvidence")]
    pub coverage_evidence: Vec<SuiteCoverageEvidence>,
    #[serde(rename = "missingRequiredSuites")]
    pub missing_required_suites: Vec<String>,
    #[serde(rename = "whyRequired")]
    pub why_required: Vec<String>,
    pub reasons: Vec<String>,
    #[serde(rename = "recommendedSuites")]
    pub recommended_suites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteCoverageEvidence {
    pub suite: String,
    #[serde(rename = "matchedScenarios")]
    pub matched_scenarios: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct MapSuitesOptions {
    pub root: PathBuf,
    pub scenario_root: PathBuf,
    pub min_risk: u8,
    pub profile: TopologyProfile,
    pub shrink_policy: ShrinkCoveragePolicy,
    pub limit: usize,
    pub offset: usize,
    pub max_matched_scenarios: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct RepoFacts {
    pub(crate) root: PathBuf,
    pub(crate) scanned_files: usize,
    pub(crate) skipped_source_files: Vec<String>,
    pub(crate) hotspots: Vec<MapHotspot>,
    pub(crate) services: Vec<ServiceBoundary>,
}

#[derive(Debug, Clone)]
pub(crate) struct ScanRecord {
    pub(crate) rel: PathBuf,
    pub(crate) component: String,
    pub(crate) signal: HotspotSignals,
    pub(crate) risk_score: u8,
    pub(crate) reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScenarioFact {
    pub(crate) path: String,
    pub(crate) tokens: BTreeSet<String>,
    pub(crate) has_explore: bool,
    pub(crate) has_fuzz: bool,
    pub(crate) has_host: bool,
    pub(crate) has_memory: bool,
    pub(crate) has_failure: bool,
    pub(crate) has_shrink: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ScenarioFactCache {
    #[serde(rename = "schemaVersion")]
    pub(crate) schema_version: String,
    pub(crate) entries: BTreeMap<String, ScenarioFactCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScenarioFactCacheEntry {
    #[serde(rename = "modifiedMs")]
    pub(crate) modified_ms: u64,
    #[serde(rename = "sizeBytes")]
    pub(crate) size_bytes: u64,
    pub(crate) fact: ScenarioFact,
}

#[derive(Debug, Clone)]
pub(crate) struct ScenarioFactBuild {
    pub(crate) facts: Vec<ScenarioFact>,
    pub(crate) unreadable_scenarios: Vec<String>,
}
