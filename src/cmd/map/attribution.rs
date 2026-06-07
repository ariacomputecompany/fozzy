use std::collections::BTreeSet;

use super::{
    SUITE_RUN_REPLAY_CI, SUITE_TEST_DET, ScenarioCoverageIndex, ScenarioFact, SuiteCoverageEvidence,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct AttributionHints {
    pub(crate) tokens: BTreeSet<String>,
    pub(crate) exact_stems: BTreeSet<String>,
}

impl AttributionHints {
    pub(crate) fn from_hotspot_hints(hints: &[String]) -> Self {
        let tokens = hints
            .iter()
            .flat_map(|hint| tokenize(hint))
            .collect::<BTreeSet<_>>();
        let exact_stems = hints
            .iter()
            .filter(|hint| hint.chars().all(|c| c.is_ascii_alphanumeric()))
            .cloned()
            .collect::<BTreeSet<_>>();
        Self {
            tokens,
            exact_stems,
        }
    }
}

pub(crate) fn covered_suites_for_hotspot(
    required: &[String],
    hints: &[String],
    scenarios: &[ScenarioFact],
    index: &ScenarioCoverageIndex,
    max_matched_scenarios: usize,
) -> Vec<SuiteCoverageEvidence> {
    let attribution_hints = AttributionHints::from_hotspot_hints(hints);

    let mut out = Vec::new();
    for suite in required {
        let matches = index
            .candidates(suite)
            .iter()
            .filter_map(|idx| scenarios.get(*idx))
            .filter(|scenario| {
                suite_allows_attribution_match(suite, &attribution_hints, &scenario.tokens)
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            continue;
        }
        let total_matches = matches.len();
        let mut matched_scenarios = matches
            .iter()
            .take(max_matched_scenarios)
            .map(|scenario| scenario.path.clone())
            .collect::<Vec<_>>();
        if total_matches > matched_scenarios.len() {
            matched_scenarios.push(format!(
                "... {} more scenario(s) omitted",
                total_matches - matched_scenarios.len()
            ));
        }
        let shared = matches
            .iter()
            .flat_map(|scenario| {
                attribution_hints
                    .tokens
                    .intersection(&scenario.tokens)
                    .cloned()
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(6)
            .collect::<Vec<_>>();
        let reason = if shared.is_empty() {
            "suite signal matched".to_string()
        } else {
            format!(
                "suite signal matched; shared attribution tokens: {}",
                shared.join(", ")
            )
        };
        out.push(SuiteCoverageEvidence {
            suite: suite.clone(),
            matched_scenarios,
            reason,
        });
    }
    out
}

pub(crate) fn suite_allows_attribution_match(
    suite: &str,
    hints: &AttributionHints,
    scenario_tokens: &BTreeSet<String>,
) -> bool {
    if suite == SUITE_TEST_DET || suite == SUITE_RUN_REPLAY_CI {
        return true;
    }

    let shared = hints
        .tokens
        .intersection(scenario_tokens)
        .collect::<Vec<_>>();
    if shared.len() >= 2 {
        return true;
    }

    if shared
        .iter()
        .any(|token| is_strong_attribution_token(token))
    {
        return true;
    }

    hints
        .exact_stems
        .iter()
        .any(|stem| is_exact_hotspot_stem(stem) && scenario_tokens.contains(stem))
}

fn is_strong_attribution_token(token: &str) -> bool {
    token.len() >= 4 && !is_generic_attribution_token(token)
}

fn is_exact_hotspot_stem(token: &str) -> bool {
    token.len() == 3 && !is_generic_attribution_token(token)
}

fn is_generic_attribution_token(token: &str) -> bool {
    matches!(
        token,
        "app"
            | "apps"
            | "artifact"
            | "artifacts"
            | "bin"
            | "ci"
            | "cli"
            | "cmd"
            | "config"
            | "crate"
            | "crates"
            | "dist"
            | "engine"
            | "example"
            | "examples"
            | "explore"
            | "fail"
            | "file"
            | "files"
            | "fuzz"
            | "host"
            | "json"
            | "main"
            | "memory"
            | "mode"
            | "modes"
            | "module"
            | "modules"
            | "package"
            | "packages"
            | "pass"
            | "proc"
            | "replay"
            | "root"
            | "run"
            | "scenario"
            | "scenarios"
            | "service"
            | "services"
            | "shrink"
            | "src"
            | "test"
            | "tests"
            | "timeout"
            | "trace"
    )
}

pub(crate) fn tokenize(input: &str) -> BTreeSet<String> {
    input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|segment| segment.len() >= 3)
        .map(|segment| segment.to_ascii_lowercase())
        .collect()
}
