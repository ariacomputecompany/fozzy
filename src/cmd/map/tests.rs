use super::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fozzy-map-{name}-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    dir
}

#[test]
fn map_suites_reports_uncovered_hotspots() {
    let root = temp_dir("coverage");
    let src = root.join("services/payments");
    let tests = root.join("tests");
    std::fs::create_dir_all(&src).expect("src");
    std::fs::create_dir_all(&tests).expect("tests");
    std::fs::write(
        src.join("handler.rs"),
        r#"
            async fn handle() {
                if retry { tokio::spawn(async move {}); }
                let _ = std::fs::read("x");
                if timeout { panic!("boom"); }
            }
            "#,
    )
    .expect("write source");

    let report = map_suites(&MapSuitesOptions {
        root: root.clone(),
        scenario_root: tests.clone(),
        min_risk: 10,
        profile: TopologyProfile::Pedantic,
        shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
        limit: 50,
        offset: 0,
        max_matched_scenarios: 25,
    })
    .expect("map suites");
    assert!(report.required_hotspot_count > 0);
    assert!(report.uncovered_hotspot_count > 0);
}

#[test]
fn profiles_are_progressively_stricter() {
    let signals = HotspotSignals {
        line_count: 300,
        branch_signals: 10,
        concurrency_signals: 1,
        external_signals: 1,
        failure_signals: 1,
        memory_signals: 1,
        entrypoint_signals: 1,
    };
    let balanced = required_suites_for_hotspot(
        TopologyProfile::Balanced,
        ShrinkCoveragePolicy::FailureOnly,
        &signals,
        true,
    )
    .len();
    let pedantic = required_suites_for_hotspot(
        TopologyProfile::Pedantic,
        ShrinkCoveragePolicy::FailureOnly,
        &signals,
        true,
    )
    .len();
    let overkill = required_suites_for_hotspot(
        TopologyProfile::Overkill,
        ShrinkCoveragePolicy::FailureOnly,
        &signals,
        true,
    )
    .len();
    assert!(balanced <= pedantic, "balanced should be least strict");
    assert!(pedantic <= overkill, "overkill should be most strict");
}

#[test]
fn no_known_failures_policy_downgrades_shrink_failure_requirement() {
    let signals = HotspotSignals {
        line_count: 50,
        branch_signals: 4,
        concurrency_signals: 0,
        external_signals: 0,
        failure_signals: 1,
        memory_signals: 0,
        entrypoint_signals: 0,
    };
    let required = required_suites_for_hotspot(
        TopologyProfile::Pedantic,
        ShrinkCoveragePolicy::NoKnownFailures,
        &signals,
        false,
    );
    assert!(required.iter().any(|s| s == SUITE_SHRINK_EXERCISED));
    assert!(!required.iter().any(|s| s == SUITE_SHRINK_FAILURE));
}

#[test]
fn no_known_failures_policy_requires_failure_if_failure_trace_exists() {
    let signals = HotspotSignals {
        line_count: 50,
        branch_signals: 4,
        concurrency_signals: 0,
        external_signals: 0,
        failure_signals: 1,
        memory_signals: 0,
        entrypoint_signals: 0,
    };
    let required = required_suites_for_hotspot(
        TopologyProfile::Pedantic,
        ShrinkCoveragePolicy::NoKnownFailures,
        &signals,
        true,
    );
    assert!(required.iter().any(|s| s == SUITE_SHRINK_FAILURE));
}

#[test]
fn candidate_file_filter_excludes_dependency_and_generated_artifacts() {
    assert!(!is_candidate_file(Path::new("/repo/package-lock.json")));
    assert!(!is_candidate_file(Path::new("/repo/Cargo.lock")));
    assert!(!is_candidate_file(Path::new("/repo/dist/app.min.js")));
    assert!(!is_candidate_file(Path::new(
        "/repo/src/types.generated.ts"
    )));
    assert!(!is_candidate_file(Path::new("/repo/config/runtime.json")));
    assert!(is_candidate_file(Path::new("/repo/src/main.rs")));
}

#[test]
fn should_skip_repo_local_tmp_outputs() {
    assert!(should_skip_path(Path::new("./.tmp/map.suites.json")));
    assert!(should_skip_path(Path::new("/repo/.tmp/report.json")));
    assert!(should_skip_path(Path::new("/repo/tmp/generated/map.rs")));
    assert!(!should_skip_path(Path::new("./src/map.rs")));
}

#[test]
fn discover_scan_roots_prefers_source_trees_over_repo_bulk() {
    let root = temp_dir("scan-roots");
    for dir in [
        "src",
        "crates/core/src",
        "tests",
        "examples/demo",
        "docs/reference",
        "tmp/cache",
    ] {
        std::fs::create_dir_all(root.join(dir)).expect("mkdir");
    }

    let roots = discover_scan_roots(&root)
        .into_iter()
        .map(|path| {
            path.strip_prefix(&root)
                .unwrap_or(&path)
                .display()
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(roots, vec!["crates".to_string(), "src".to_string()]);
}

#[test]
fn scan_repo_prunes_non_source_trees_before_file_walk() {
    let root = temp_dir("scan-prune");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::create_dir_all(root.join("tmp/generated")).expect("tmp");
    std::fs::create_dir_all(root.join("examples/demo")).expect("examples");
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() { if true { std::thread::spawn(|| {}); } }",
    )
    .expect("write src");
    std::fs::write(
        root.join("tmp/generated/ignored.rs"),
        "fn ignored() { panic!(\"nope\"); }",
    )
    .expect("write tmp");
    std::fs::write(
        root.join("examples/demo/demo.rs"),
        "fn demo() { if retry { panic!(\"demo\"); } }",
    )
    .expect("write example");

    let facts = scan_repo(&root).expect("scan repo");

    assert_eq!(
        facts.scanned_files, 1,
        "expected only source trees to be scanned"
    );
    assert_eq!(
        facts.hotspots.len(),
        1,
        "expected only src hotspot to remain"
    );
    assert_eq!(facts.hotspots[0].path, "src/main.rs");
}

#[test]
fn map_suites_credits_natural_fetch_host_scenario() {
    let root = temp_dir("fetch-host-credit");
    let src = root.join("crates/cli/src/cmd");
    let tests = root.join("tests");
    std::fs::create_dir_all(&src).expect("src");
    std::fs::create_dir_all(&tests).expect("tests");
    std::fs::write(
        src.join("fetch.rs"),
        r#"
            pub fn fetch() {
                let _ = std::fs::read("config.json");
            }
            "#,
    )
    .expect("write source");
    std::fs::write(
        tests.join("fetch.host.fozzy.json"),
        r#"
            {
              "version": 1,
              "name": "fetch-host",
              "steps": [
                { "type": "fs_write", "path": "tmp/fetch.txt", "data": "ok" },
                { "type": "fs_read_assert", "path": "tmp/fetch.txt", "equals": "ok" }
              ]
            }
            "#,
    )
    .expect("write scenario");

    let report = map_suites(&MapSuitesOptions {
        root: root.clone(),
        scenario_root: tests,
        min_risk: 1,
        profile: TopologyProfile::Pedantic,
        shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
        limit: 50,
        offset: 0,
        max_matched_scenarios: 25,
    })
    .expect("map suites");
    let suite = report.suites.first().expect("suite");
    assert!(
        suite.covered_suites.iter().any(|s| s == SUITE_HOST),
        "expected host suite to be credited from natural fetch.host scenario: {:?}",
        suite.coverage_evidence
    );
}

#[test]
fn map_suites_reports_unreadable_scenarios() {
    let root = temp_dir("unreadable-scenarios");
    let src = root.join("src");
    let tests = root.join("tests");
    std::fs::create_dir_all(&src).expect("src");
    std::fs::create_dir_all(&tests).expect("tests");
    std::fs::write(
        src.join("main.rs"),
        "fn main() { if true { std::thread::spawn(|| {}); } }",
    )
    .expect("write source");
    std::fs::write(
        tests.join("good.fozzy.json"),
        r#"{"version":1,"name":"good","steps":[{"type":"assert_ok","value":true}]}"#,
    )
    .expect("write good");
    std::fs::write(tests.join("bad.fozzy.json"), [0_u8, 159, 146, 150]).expect("write bad");

    let report = map_suites(&MapSuitesOptions {
        root: root.clone(),
        scenario_root: tests,
        min_risk: 1,
        profile: TopologyProfile::Pedantic,
        shrink_policy: ShrinkCoveragePolicy::NoKnownFailures,
        limit: 50,
        offset: 0,
        max_matched_scenarios: 25,
    })
    .expect("map suites");

    assert_eq!(report.unreadable_scenarios.len(), 1);
    assert!(
        !report.warnings.is_empty(),
        "expected degraded-confidence warning when scenarios are unreadable"
    );
}

#[test]
fn suite_specific_attribution_ignores_generic_token_only_overlap() {
    let hints = AttributionHints::from_hotspot_hints(&hotspot_hints(&MapHotspot {
        id: "src:src/runtime/memory.rs".to_string(),
        component: "src".to_string(),
        path: "src/runtime/memory.rs".to_string(),
        risk_score: 10,
        reasons: Vec::new(),
        signals: HotspotSignals {
            memory_signals: 1,
            ..HotspotSignals::default()
        },
        recommended_suites: vec![SUITE_MEMORY.to_string()],
    }));
    let scenario_tokens =
        tokenize("tests/memory.pass.fozzy.json memory-pass memory_alloc memory_free");

    assert!(
        !suite_allows_attribution_match(SUITE_MEMORY, &hints, &scenario_tokens),
        "generic suite words alone should not count as hotspot attribution"
    );
}

#[test]
fn suite_specific_attribution_accepts_exact_three_letter_stem_matches() {
    let hints = AttributionHints::from_hotspot_hints(&hotspot_hints(&MapHotspot {
        id: "cli:crates/cli/src/cmd/tag.rs".to_string(),
        component: "cli".to_string(),
        path: "crates/cli/src/cmd/tag.rs".to_string(),
        risk_score: 10,
        reasons: Vec::new(),
        signals: HotspotSignals {
            branch_signals: 10,
            ..HotspotSignals::default()
        },
        recommended_suites: vec![SUITE_FUZZ.to_string()],
    }));

    assert!(suite_allows_attribution_match(
        SUITE_FUZZ,
        &hints,
        &tokenize("tests/tag.fuzz.fozzy.json name=tag-fuzz")
    ));
    assert!(suite_allows_attribution_match(
        SUITE_EXPLORE,
        &hints,
        &tokenize("tests/tag.explore.fozzy.json name=tag-explore")
    ));
    assert!(suite_allows_attribution_match(
        SUITE_SHRINK_EXERCISED,
        &hints,
        &tokenize("tests/tag.shrink.fozzy.json name=tag-shrink")
    ));
}

#[test]
fn map_suites_credits_short_natural_hotspot_names() {
    let root = temp_dir("short-natural-hotspot-credit");
    let src_cmd = root.join("crates/cli/src/cmd");
    let src_cli = root.join("crates/cli/src");
    let tests = root.join("tests");
    std::fs::create_dir_all(&src_cmd).expect("src cmd");
    std::fs::create_dir_all(&src_cli).expect("src cli");
    std::fs::create_dir_all(&tests).expect("tests");

    std::fs::write(
        src_cmd.join("tag.rs"),
        r#"
            pub fn tag() {
                if true {}
                if true {}
                if true {}
                if true {}
                if true {}
                if true {}
                if true {}
                if true {}
                if std::env::var("FOZZY_FAIL").is_err() { panic!("tag failure"); }
                if "retry".contains("retry") { let _ = "error"; }
                if "timeout".contains("timeout") { let _ = "fail"; }
                if "backoff".contains("backoff") { let _ = "error"; }
            }
            "#,
    )
    .expect("write tag source");
    std::fs::write(
        src_cli.join("ipc.rs"),
        r#"
            pub fn ipc() {
                std::thread::spawn(|| {
                    if true {
                        let _ = 1 + 1;
                    }
                });
            }
            "#,
    )
    .expect("write ipc source");
    std::fs::write(
        src_cmd.join("log.rs"),
        r#"
            pub fn log_output() {
                let first_error = std::env::var("FOZZY_FAIL").is_err();
                if first_error { panic!("boom"); }
                if "retry".contains("retry") { let _ = "error"; }
                if "timeout".contains("timeout") { let _ = "fail"; }
                if "backoff".contains("backoff") { let _ = "error"; }
            }
            "#,
    )
    .expect("write log source");

    std::fs::write(
        tests.join("tag.explore.fozzy.json"),
        r#"{ "version": 1, "name": "tag-explore", "distributed": true }"#,
    )
    .expect("write tag explore");
    std::fs::write(
        tests.join("tag.fuzz.fozzy.json"),
        r#"{ "version": 1, "name": "tag-fuzz", "mode": "fuzz" }"#,
    )
    .expect("write tag fuzz");
    std::fs::write(
        tests.join("tag.shrink.fozzy.json"),
        r#"{ "version": 1, "name": "tag-shrink", "shrink_trace": true }"#,
    )
    .expect("write tag shrink");
    std::fs::write(
        tests.join("ipc.explore.fozzy.json"),
        r#"{ "version": 1, "name": "ipc-explore", "distributed": true }"#,
    )
    .expect("write ipc explore");
    std::fs::write(
        tests.join("log.explore.fozzy.json"),
        r#"{ "version": 1, "name": "log-explore", "distributed": true }"#,
    )
    .expect("write log explore");

    let report = map_suites(&MapSuitesOptions {
        root: root.clone(),
        scenario_root: tests,
        min_risk: 1,
        profile: TopologyProfile::Pedantic,
        shrink_policy: ShrinkCoveragePolicy::ExercisedOk,
        limit: 50,
        offset: 0,
        max_matched_scenarios: 25,
    })
    .expect("map suites");

    let by_path = report
        .suites
        .iter()
        .map(|suite| (suite.path.as_str(), suite))
        .collect::<BTreeMap<_, _>>();

    let tag = by_path.get("crates/cli/src/cmd/tag.rs").expect("tag suite");
    assert!(
        tag.covered_suites
            .iter()
            .any(|suite| suite == SUITE_EXPLORE)
    );
    assert!(tag.covered_suites.iter().any(|suite| suite == SUITE_FUZZ));
    assert!(
        tag.covered_suites
            .iter()
            .any(|suite| suite == SUITE_SHRINK_EXERCISED)
    );

    let ipc = by_path.get("crates/cli/src/ipc.rs").expect("ipc suite");
    assert!(
        ipc.covered_suites
            .iter()
            .any(|suite| suite == SUITE_EXPLORE)
    );

    let log = by_path.get("crates/cli/src/cmd/log.rs").expect("log suite");
    assert!(
        log.covered_suites
            .iter()
            .any(|suite| suite == SUITE_EXPLORE)
    );
}
