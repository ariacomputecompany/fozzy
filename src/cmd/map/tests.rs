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
        all: false,
        only_required: false,
        only_uncovered: false,
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
fn discover_scan_roots_scans_repo_root() {
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

    assert_eq!(roots, vec!["".to_string()]);
}

#[test]
fn scan_repo_skips_known_non_product_trees_but_keeps_repo_surface() {
    let root = temp_dir("scan-prune");
    std::fs::create_dir_all(root.join("src")).expect("src");
    std::fs::create_dir_all(root.join("vendor/vllm")).expect("vendor");
    std::fs::create_dir_all(root.join("tmp/generated")).expect("tmp");
    std::fs::create_dir_all(root.join("examples/demo")).expect("examples");
    std::fs::write(
        root.join("src/main.rs"),
        "fn main() { if true { std::thread::spawn(|| {}); } }",
    )
    .expect("write src");
    std::fs::write(
        root.join("vendor/vllm/cache.rs"),
        "fn cache() { if retry { std::fs::read(\"x\").ok(); } }",
    )
    .expect("write vendor");
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
        facts.scanned_files, 2,
        "expected repo scan to keep src + vendored code while skipping tmp/examples"
    );
    assert_eq!(
        facts.hotspots.len(),
        2,
        "expected src + vendored hotspots to remain"
    );
    let paths = facts
        .hotspots
        .iter()
        .map(|hotspot| hotspot.path.as_str())
        .collect::<Vec<_>>();
    assert!(paths.contains(&"src/main.rs"));
    assert!(paths.contains(&"vendor/vllm/cache.rs"));
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
        all: false,
        only_required: false,
        only_uncovered: false,
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
        all: false,
        only_required: false,
        only_uncovered: false,
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
        all: false,
        only_required: false,
        only_uncovered: false,
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

#[test]
fn map_suites_applies_filters_before_pagination() {
    let root = temp_dir("filters-before-pagination");
    let src = root.join("src");
    let tests = root.join("tests");
    std::fs::create_dir_all(&src).expect("src");
    std::fs::create_dir_all(&tests).expect("tests");

    std::fs::write(
        src.join("alpha.rs"),
        r#"
            pub fn main() {
                let values = [1, 2, 3, 4, 5, 6, 7, 8];
                let _total: i32 = values.iter().sum();
            }
        "#,
    )
    .expect("write alpha");
    std::fs::write(
        src.join("bravo.rs"),
        r#"
            pub fn bravo() {
                if retry { panic!("bravo"); }
                if timeout { panic!("bravo"); }
                if backoff { panic!("bravo"); }
                if true {}
                if true {}
                if true {}
                if true {}
                if true {}
            }
        "#,
    )
    .expect("write bravo");
    std::fs::write(
        tests.join("alpha.fozzy.json"),
        r#"{"version":1,"name":"alpha","steps":[{"type":"assert_ok","value":true}]}"#,
    )
    .expect("write alpha scenario");

    let base = MapSuitesOptions {
        root: root.clone(),
        scenario_root: tests.clone(),
        min_risk: 1,
        profile: TopologyProfile::Balanced,
        shrink_policy: ShrinkCoveragePolicy::ExercisedOk,
        limit: 10,
        offset: 0,
        all: false,
        only_required: false,
        only_uncovered: false,
        max_matched_scenarios: 25,
    };

    let full = map_suites(&base).expect("full report");
    assert_eq!(full.total_suites, 2);

    let uncovered = map_suites(&MapSuitesOptions {
        only_uncovered: true,
        ..base.clone()
    })
    .expect("uncovered report");
    assert_eq!(uncovered.total_suites, 1);
    assert_eq!(uncovered.returned_suites, 1);
    assert_eq!(uncovered.suites[0].path, "src/bravo.rs");

    let paged = map_suites(&MapSuitesOptions {
        only_uncovered: true,
        limit: 1,
        offset: 0,
        ..base.clone()
    })
    .expect("paged uncovered report");
    assert_eq!(paged.total_suites, 1);
    assert_eq!(paged.returned_suites, 1);
    assert_eq!(paged.suites[0].path, "src/bravo.rs");
}

#[test]
fn map_suites_all_returns_full_filtered_set_without_truncation() {
    let root = temp_dir("all-returns-full-set");
    let src = root.join("src");
    let tests = root.join("tests");
    std::fs::create_dir_all(&src).expect("src");
    std::fs::create_dir_all(&tests).expect("tests");

    for name in ["alpha", "bravo", "charlie"] {
        std::fs::write(
            src.join(format!("{name}.rs")),
            format!(
                r#"
                pub fn {name}() {{
                    if retry {{ panic!("{name}"); }}
                    if timeout {{ panic!("{name}"); }}
                    if backoff {{ panic!("{name}"); }}
                    if true {{}}
                    if true {{}}
                    if true {{}}
                    if true {{}}
                    if true {{}}
                }}
            "#
            ),
        )
        .expect("write source");
    }

    let report = map_suites(&MapSuitesOptions {
        root,
        scenario_root: tests,
        min_risk: 1,
        profile: TopologyProfile::Pedantic,
        shrink_policy: ShrinkCoveragePolicy::ExercisedOk,
        limit: 1,
        offset: 99,
        all: true,
        only_required: true,
        only_uncovered: true,
        max_matched_scenarios: 25,
    })
    .expect("all report");

    assert_eq!(report.total_suites, 3);
    assert_eq!(report.returned_suites, 3);
    assert_eq!(report.offset, 0);
    assert_eq!(report.limit, 3);
    assert!(!report.truncated);
}

#[test]
fn build_scenario_facts_infers_named_host_memory_and_shrink_signals() {
    let root = temp_dir("scenario-facts-signals");
    let scenario = root.join("memory.cache_root_churn.shrink.host.fozzy.json");
    std::fs::write(
        &scenario,
        r#"
        {
          "version": 1,
          "name": "memory-cache-root-churn-shrink-host",
          "steps": [
            {
              "type": "proc_when",
              "cmd": "sh",
              "args": ["-c", "echo ok"],
              "exit_code": 0,
              "stdout": "ok\n",
              "stderr": ""
            },
            {
              "type": "proc_spawn",
              "cmd": "sh",
              "args": ["-c", "echo ok"],
              "expect_exit": 0,
              "expect_stdout": "ok\n"
            }
          ]
        }
        "#,
    )
    .expect("write scenario");

    let facts = build_scenario_facts(&[scenario], None);
    assert!(facts.unreadable_scenarios.is_empty());
    let fact = facts.facts.first().expect("scenario fact");
    assert!(fact.has_host);
    assert!(fact.has_memory);
    assert!(fact.has_shrink);
}

#[test]
fn build_scenario_facts_ignores_contract_only_scenarios() {
    let root = temp_dir("scenario-facts-contract-only");
    let scenario = root.join("proc-when-only.fozzy.json");
    std::fs::write(
        &scenario,
        r#"
        {
          "version": 1,
          "name": "proc-when-only",
          "steps": [
            {
              "type": "proc_when",
              "cmd": "sh",
              "args": ["-c", "echo ok"],
              "exit_code": 0,
              "stdout": "ok\n",
              "stderr": ""
            }
          ]
        }
        "#,
    )
    .expect("write scenario");

    let facts = build_scenario_facts(&[scenario], None);
    assert!(facts.unreadable_scenarios.is_empty());
    assert!(facts.facts.is_empty());
    assert_eq!(facts.contract_only_scenarios.len(), 1);
}
