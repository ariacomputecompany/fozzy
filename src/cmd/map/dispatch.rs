use std::path::PathBuf;

use crate::{Config, FozzyResult};

use super::{
    MapCommand, MapHotspotsReport, MapServicesReport, MapSuitesOptions, MapSuitesReport,
    ScenarioCoverageIndex, SuiteRecommendation, build_scenario_facts, covered_suites_for_hotspot,
    discover_scenarios, effective_min_risk, hotspot_hints, recommended_suites_for_hotspot,
    required_suites_for_hotspot, scan_repo, why_required,
};

pub fn map_command(config: &Config, command: &MapCommand) -> FozzyResult<serde_json::Value> {
    match command {
        MapCommand::Hotspots {
            root,
            min_risk,
            limit,
        } => {
            let facts = scan_repo(root)?;
            let mut hotspots = facts
                .hotspots
                .into_iter()
                .filter(|hotspot| hotspot.risk_score >= *min_risk)
                .collect::<Vec<_>>();
            hotspots.sort_by(|a, b| {
                b.risk_score
                    .cmp(&a.risk_score)
                    .then_with(|| a.path.cmp(&b.path))
            });
            hotspots.truncate(*limit);
            Ok(serde_json::to_value(MapHotspotsReport {
                schema_version: "fozzy.map_hotspots.v2".to_string(),
                root: facts.root.display().to_string(),
                scanned_files: facts.scanned_files,
                min_risk: *min_risk,
                hotspots,
            })?)
        }
        MapCommand::Services { root } => {
            let facts = scan_repo(root)?;
            Ok(serde_json::to_value(MapServicesReport {
                schema_version: "fozzy.map_services.v2".to_string(),
                root: facts.root.display().to_string(),
                scanned_files: facts.scanned_files,
                services: facts.services,
            })?)
        }
        MapCommand::Suites {
            root,
            scenario_root,
            min_risk,
            profile,
            shrink_policy,
            limit,
            offset,
            max_matched_scenarios,
        } => {
            let report = map_suites_with_cache(
                &MapSuitesOptions {
                    root: root.clone(),
                    scenario_root: scenario_root.clone(),
                    min_risk: *min_risk,
                    profile: *profile,
                    shrink_policy: *shrink_policy,
                    limit: *limit,
                    offset: *offset,
                    max_matched_scenarios: *max_matched_scenarios,
                },
                Some(config.base_dir.join("cache")),
            )?;
            Ok(serde_json::to_value(report)?)
        }
    }
}

pub fn map_suites(opt: &MapSuitesOptions) -> FozzyResult<MapSuitesReport> {
    map_suites_with_cache(opt, None)
}

fn map_suites_with_cache(
    opt: &MapSuitesOptions,
    cache_dir: Option<PathBuf>,
) -> FozzyResult<MapSuitesReport> {
    let scenario_files = discover_scenarios(&opt.scenario_root)?;
    let facts = scan_repo(&opt.root)?;
    let scenario_build = build_scenario_facts(&scenario_files, cache_dir.as_deref());
    let scenario_facts = scenario_build.facts;
    let coverage_index = ScenarioCoverageIndex::new(&scenario_facts);
    let has_known_shrink_failure = scenario_facts
        .iter()
        .any(|scenario| scenario.has_shrink && scenario.has_failure);

    let effective_min_risk = effective_min_risk(opt.min_risk, opt.profile);
    let mut suites = Vec::<SuiteRecommendation>::new();
    let mut required_hotspot_count = 0usize;
    let mut covered_hotspot_count = 0usize;

    for hotspot in facts.hotspots {
        let hints = hotspot_hints(&hotspot);
        let required_by_policy = hotspot.risk_score >= effective_min_risk;
        if required_by_policy {
            required_hotspot_count += 1;
        }

        let required_suites = required_suites_for_hotspot(
            opt.profile,
            opt.shrink_policy,
            &hotspot.signals,
            has_known_shrink_failure,
        );
        let coverage_evidence = covered_suites_for_hotspot(
            &required_suites,
            &hints,
            &scenario_facts,
            &coverage_index,
            opt.max_matched_scenarios.max(1),
        );
        let covered_suites = coverage_evidence
            .iter()
            .map(|evidence| evidence.suite.clone())
            .collect::<Vec<_>>();
        let missing_required_suites = required_suites
            .iter()
            .filter(|suite| !covered_suites.contains(*suite))
            .cloned()
            .collect::<Vec<_>>();
        let covered = !required_by_policy || missing_required_suites.is_empty();
        if required_by_policy && covered {
            covered_hotspot_count += 1;
        }

        let why_required = why_required(hotspot.risk_score, effective_min_risk, &hotspot.signals);
        let mut recommended = required_suites.clone();
        for extra in recommended_suites_for_hotspot(&hotspot.signals) {
            if !recommended.contains(&extra) {
                recommended.push(extra);
            }
        }

        suites.push(SuiteRecommendation {
            hotspot_id: hotspot.id,
            component: hotspot.component,
            path: hotspot.path,
            risk_score: hotspot.risk_score,
            required_by_policy,
            covered,
            coverage_hints: hints,
            required_suites,
            covered_suites,
            coverage_evidence,
            missing_required_suites,
            why_required,
            reasons: hotspot.reasons,
            recommended_suites: recommended,
        });
    }

    suites.sort_by(|a, b| {
        b.risk_score
            .cmp(&a.risk_score)
            .then_with(|| a.path.cmp(&b.path))
    });
    let total_suites = suites.len();
    let suites = suites
        .into_iter()
        .skip(opt.offset)
        .take(opt.limit)
        .collect::<Vec<_>>();
    let returned_suites = suites.len();
    let truncated = opt.offset.saturating_add(returned_suites) < total_suites;
    let uncovered_hotspot_count = required_hotspot_count.saturating_sub(covered_hotspot_count);

    Ok(MapSuitesReport {
        schema_version: "fozzy.map_suites.v5".to_string(),
        root: facts.root.display().to_string(),
        scenario_root: opt.scenario_root.display().to_string(),
        scanned_files: facts.scanned_files,
        profile: opt.profile,
        shrink_policy: opt.shrink_policy,
        base_min_risk: opt.min_risk,
        effective_min_risk,
        scenario_count: scenario_files.len(),
        skipped_source_files: facts.skipped_source_files.clone(),
        unreadable_scenarios: scenario_build.unreadable_scenarios.clone(),
        warnings: map_warnings(
            &facts.skipped_source_files,
            &scenario_build.unreadable_scenarios,
        ),
        required_hotspot_count,
        covered_hotspot_count,
        uncovered_hotspot_count,
        total_suites,
        returned_suites,
        offset: opt.offset,
        limit: opt.limit,
        truncated,
        suites,
    })
}

pub(crate) fn map_warnings(
    skipped_source_files: &[String],
    unreadable_scenarios: &[String],
) -> Vec<String> {
    let mut warnings = Vec::new();
    if !skipped_source_files.is_empty() {
        warnings.push(format!(
            "map scan skipped {} source file(s); hotspot coverage is incomplete",
            skipped_source_files.len()
        ));
    }
    if !unreadable_scenarios.is_empty() {
        warnings.push(format!(
            "map suites skipped {} unreadable scenario file(s); suite attribution confidence is reduced",
            unreadable_scenarios.len()
        ));
    }
    warnings
}
