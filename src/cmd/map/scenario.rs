use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::{DistributedStep, FozzyError, FozzyResult, ScenarioFile, ScenarioPath, Step};

use super::{
    SUITE_EXPLORE, SUITE_FUZZ, SUITE_HOST, SUITE_MEMORY, SUITE_RUN_REPLAY_CI,
    SUITE_SHRINK_EXERCISED, SUITE_SHRINK_FAILURE, SUITE_TEST_DET, ScenarioFact, ScenarioFactBuild,
    ScenarioFactCache, ScenarioFactCacheEntry, tokenize,
};

const SCENARIO_FACT_CACHE_SCHEMA_VERSION: &str = "fozzy.map_scenario_facts.v2";

pub(crate) struct ScenarioCoverageIndex {
    by_suite: BTreeMap<String, Vec<usize>>,
}

impl ScenarioCoverageIndex {
    pub(crate) fn new(scenarios: &[ScenarioFact]) -> Self {
        let mut by_suite: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (idx, scenario) in scenarios.iter().enumerate() {
            for suite in [
                SUITE_TEST_DET,
                SUITE_RUN_REPLAY_CI,
                SUITE_FUZZ,
                SUITE_EXPLORE,
                SUITE_HOST,
                SUITE_MEMORY,
                SUITE_SHRINK_EXERCISED,
                SUITE_SHRINK_FAILURE,
            ] {
                if matches_suite_signal(scenario, suite) {
                    by_suite.entry(suite.to_string()).or_default().push(idx);
                }
            }
        }
        Self { by_suite }
    }

    pub(crate) fn candidates(&self, suite: &str) -> &[usize] {
        self.by_suite.get(suite).map(Vec::as_slice).unwrap_or(&[])
    }
}

pub(crate) fn matches_suite_signal(scenario: &ScenarioFact, suite: &str) -> bool {
    match suite {
        SUITE_TEST_DET => true,
        SUITE_RUN_REPLAY_CI => true,
        SUITE_FUZZ => scenario.has_fuzz,
        SUITE_EXPLORE => scenario.has_explore,
        SUITE_HOST => scenario.has_host,
        SUITE_MEMORY => scenario.has_memory,
        SUITE_SHRINK_EXERCISED => scenario.has_shrink,
        SUITE_SHRINK_FAILURE => scenario.has_failure && scenario.has_shrink,
        _ => false,
    }
}

pub(crate) fn build_scenario_facts(
    paths: &[PathBuf],
    cache_dir: Option<&Path>,
) -> ScenarioFactBuild {
    let mut facts = Vec::new();
    let mut unreadable_scenarios = Vec::new();
    let mut contract_only_scenarios = Vec::new();
    let cache_path = cache_dir.map(|dir| dir.join("map-suites-scenarios.v2.json"));
    let mut cache = cache_path
        .as_ref()
        .and_then(|path| load_scenario_fact_cache(path).ok())
        .unwrap_or_else(empty_scenario_fact_cache);
    let mut next_entries = BTreeMap::new();
    for path in paths {
        match scenario_fact(path, &cache) {
            Ok(Some((cache_entry, fact))) => {
                next_entries.insert(path_key(path), cache_entry);
                facts.push(fact);
            }
            Ok(None) => contract_only_scenarios.push(path.display().to_string()),
            Err(err) => unreadable_scenarios.push(format!("{}: {err}", path.display())),
        }
    }
    cache.entries = next_entries;
    if let Some(path) = cache_path.as_ref() {
        let _ = save_scenario_fact_cache(path, &cache);
    }
    ScenarioFactBuild {
        facts,
        unreadable_scenarios,
        contract_only_scenarios,
    }
}

pub(crate) fn discover_scenarios(root: &Path) -> FozzyResult<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::<PathBuf>::new();
    for entry in WalkDir::new(root).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".fozzy.json"))
        {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    Ok(out)
}

fn scenario_fact(
    path: &Path,
    cache: &ScenarioFactCache,
) -> FozzyResult<Option<(ScenarioFactCacheEntry, ScenarioFact)>> {
    let metadata = std::fs::metadata(path)?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);
    let size_bytes = metadata.len();
    let cache_key = path_key(path);
    if let Some(entry) = cache.entries.get(&cache_key)
        && entry.modified_ms == modified_ms
        && entry.size_bytes == size_bytes
    {
        return Ok(Some((entry.clone(), entry.fact.clone())));
    }

    let name = path
        .file_name()
        .and_then(|segment| segment.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut tokens = tokenize(&name);
    tokens.extend(tokenize(&path.to_string_lossy().to_ascii_lowercase()));
    let bytes = std::fs::read(path)?;
    let fact = match crate::Scenario::load_file(&ScenarioPath::new(path.to_path_buf())) {
        Ok(scenario) => {
            if scenario_is_contract_only(&scenario) {
                return Ok(None);
            }
            scenario_fact_from_parsed(path, &mut tokens, scenario)
        }
        Err(_) => scenario_fact_from_metadata(path, &mut tokens, &bytes)?,
    };
    let cache_entry = ScenarioFactCacheEntry {
        modified_ms,
        size_bytes,
        fact: fact.clone(),
    };
    Ok(Some((cache_entry, fact)))
}

fn scenario_fact_from_parsed(
    path: &Path,
    tokens: &mut std::collections::BTreeSet<String>,
    scenario: ScenarioFile,
) -> ScenarioFact {
    let (has_explore, has_fuzz, has_host, has_memory, has_failure, has_shrink) = match scenario {
        ScenarioFile::Steps(steps) => {
            tokens.extend(tokenize(&steps.name.to_ascii_lowercase()));
            let inferred = infer_named_suite_signals(tokens);
            let has_host = steps.steps.iter().any(step_uses_host_surface) || inferred.host;
            let has_memory = steps.steps.iter().any(step_uses_memory_surface) || inferred.memory;
            let has_failure = steps.steps.iter().any(step_has_failure_contract);
            (
                inferred.explore,
                inferred.fuzz,
                has_host,
                has_memory,
                has_failure,
                inferred.shrink,
            )
        }
        ScenarioFile::Distributed(distributed) => {
            tokens.extend(tokenize(&distributed.name.to_ascii_lowercase()));
            let has_failure = distributed
                .distributed
                .steps
                .iter()
                .any(distributed_step_has_failure_contract);
            (true, false, false, false, has_failure, true)
        }
        ScenarioFile::Suites(suites) => {
            tokens.extend(tokenize(&suites.name.to_ascii_lowercase()));
            let inferred = infer_named_suite_signals(tokens);
            (
                inferred.explore,
                inferred.fuzz,
                inferred.host,
                inferred.memory,
                false,
                inferred.shrink,
            )
        }
    };

    ScenarioFact {
        path: path.display().to_string(),
        tokens: tokens.clone(),
        has_explore,
        has_fuzz,
        has_host,
        has_memory,
        has_failure,
        has_shrink,
    }
}

#[derive(Debug, Deserialize)]
struct ScenarioMetadataFallback {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    distributed: Option<serde_json::Value>,
    #[serde(default)]
    shrink_trace: Option<bool>,
}

fn scenario_fact_from_metadata(
    path: &Path,
    tokens: &mut std::collections::BTreeSet<String>,
    bytes: &[u8],
) -> FozzyResult<ScenarioFact> {
    let metadata: ScenarioMetadataFallback = serde_json::from_slice(bytes).map_err(|err| {
        FozzyError::Scenario(format!(
            "failed to parse scenario metadata {}: {err}",
            path.display()
        ))
    })?;
    if let Some(name) = metadata.name.as_deref() {
        tokens.extend(tokenize(&name.to_ascii_lowercase()));
    }

    let has_explore = metadata.distributed.is_some_and(|value| match value {
        serde_json::Value::Bool(flag) => flag,
        serde_json::Value::Object(_) => true,
        _ => false,
    });
    let inferred = infer_named_suite_signals(tokens);
    let has_fuzz = metadata
        .mode
        .as_deref()
        .is_some_and(|mode| mode.eq_ignore_ascii_case("fuzz"))
        || inferred.fuzz;
    let has_shrink = metadata.shrink_trace.unwrap_or(false) || inferred.shrink;

    Ok(ScenarioFact {
        path: path.display().to_string(),
        tokens: tokens.clone(),
        has_explore: has_explore || inferred.explore,
        has_fuzz,
        has_host: inferred.host,
        has_memory: inferred.memory,
        has_failure: false,
        has_shrink,
    })
}

fn step_uses_host_surface(step: &Step) -> bool {
    match step {
        Step::ProcWhen { .. }
        | Step::ProcSpawn { .. }
        | Step::FsWrite { .. }
        | Step::FsReadAssert { .. }
        | Step::FsSnapshot { .. }
        | Step::FsRestore { .. }
        | Step::HttpRequest { .. } => true,
        Step::AssertThrows { steps } | Step::AssertRejects { steps } => {
            steps.iter().any(step_uses_host_surface)
        }
        _ => false,
    }
}

fn step_uses_memory_surface(step: &Step) -> bool {
    match step {
        Step::MemoryAlloc { .. }
        | Step::MemoryFree { .. }
        | Step::MemoryLimitMb { .. }
        | Step::MemoryFailAfterAllocs { .. }
        | Step::MemoryFragmentation { .. }
        | Step::MemoryPressureWave { .. }
        | Step::MemoryCheckpoint { .. }
        | Step::MemoryAssertInUseBytes { .. } => true,
        Step::AssertThrows { steps } | Step::AssertRejects { steps } => {
            steps.iter().any(step_uses_memory_surface)
        }
        _ => false,
    }
}

fn step_has_failure_contract(step: &Step) -> bool {
    match step {
        Step::Fail { .. } | Step::Panic { .. } => true,
        Step::AssertThrows { .. } | Step::AssertRejects { .. } => true,
        _ => false,
    }
}

fn distributed_step_has_failure_contract(step: &DistributedStep) -> bool {
    matches!(
        step,
        DistributedStep::Crash { .. } | DistributedStep::Partition { .. }
    )
}

fn scenario_is_contract_only(scenario: &ScenarioFile) -> bool {
    match scenario {
        ScenarioFile::Steps(steps) => {
            !steps.steps.is_empty() && steps.steps.iter().all(step_is_contract_only)
        }
        ScenarioFile::Suites(_) | ScenarioFile::Distributed(_) => false,
    }
}

fn step_is_contract_only(step: &Step) -> bool {
    matches!(step, Step::ProcWhen { .. } | Step::HttpWhen { .. })
}

#[derive(Clone, Copy, Default)]
struct InferredSuiteSignals {
    explore: bool,
    fuzz: bool,
    host: bool,
    memory: bool,
    shrink: bool,
}

fn infer_named_suite_signals(tokens: &std::collections::BTreeSet<String>) -> InferredSuiteSignals {
    let has = |needle: &str| tokens.iter().any(|token| token == needle);
    InferredSuiteSignals {
        explore: has("explore") || has("distributed"),
        fuzz: has("fuzz"),
        host: has("host") || has("proc") || has("http") || has("fs"),
        memory: has("memory"),
        shrink: has("shrink"),
    }
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn empty_scenario_fact_cache() -> ScenarioFactCache {
    ScenarioFactCache {
        schema_version: SCENARIO_FACT_CACHE_SCHEMA_VERSION.to_string(),
        entries: BTreeMap::new(),
    }
}

fn load_scenario_fact_cache(path: &Path) -> FozzyResult<ScenarioFactCache> {
    let bytes = std::fs::read(path)?;
    let cache: ScenarioFactCache = serde_json::from_slice(&bytes).map_err(|err| {
        FozzyError::Scenario(format!("failed to parse {}: {err}", path.display()))
    })?;
    if cache.schema_version != SCENARIO_FACT_CACHE_SCHEMA_VERSION {
        return Ok(empty_scenario_fact_cache());
    }
    Ok(cache)
}

fn save_scenario_fact_cache(path: &Path, cache: &ScenarioFactCache) -> FozzyResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(cache)?)?;
    Ok(())
}
