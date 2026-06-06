use rand_core::RngCore as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use uuid::Uuid;

use crate::engine::{
    RecordCollisionPolicy, RunOptions, RunResult, ScenarioRun, run_scenario_inner,
};
use crate::finalize::{
    build_run_summary, write_reporter_artifacts, write_single_scenario_trace, write_summary_report,
};
use crate::{
    Config, ExitStatus, Finding, FindingKind, FozzyError, FozzyResult, RunMode, ScenarioPath,
    wall_time_iso_utc,
};

pub fn run_tests(config: &Config, globs: &[String], opt: &RunOptions) -> FozzyResult<RunResult> {
    let patterns = if globs.is_empty() {
        vec!["tests/**/*.fozzy.json".to_string()]
    } else {
        globs.to_vec()
    };

    let resolved_inputs = crate::resolve_matching_files(&patterns)?;
    if !resolved_inputs.missing_literal_files.is_empty() {
        let missing = resolved_inputs
            .missing_literal_files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(FozzyError::InvalidArgument(format!(
            "explicit scenario path(s) not found: {missing}"
        )));
    }
    let scenario_paths = resolved_inputs.files;
    if scenario_paths.is_empty() {
        return Err(FozzyError::InvalidArgument(format!(
            "no scenario files matched (patterns={patterns:?})"
        )));
    }

    let started_at = wall_time_iso_utc();
    let started = Instant::now();
    let seed = opt.seed.unwrap_or_else(gen_seed);
    let run_id = Uuid::new_v4().to_string();

    let mut filtered_paths = Vec::new();
    let mut skipped = 0u64;
    for p in scenario_paths {
        if let Some(filter) = &opt.filter
            && !p.to_string_lossy().contains(filter)
        {
            skipped += 1;
            continue;
        }
        filtered_paths.push(p);
    }

    let mut distributed_paths = Vec::new();
    for path in &filtered_paths {
        let scenario_path = ScenarioPath::new(path.clone());
        if matches!(
            crate::Scenario::load_file(&scenario_path)?,
            crate::ScenarioFile::Distributed(_)
        ) {
            distributed_paths.push(path.display().to_string());
        }
    }
    if !distributed_paths.is_empty() {
        return Err(FozzyError::InvalidArgument(format!(
            "fozzy test discovered distributed scenario(s) that must be run with `fozzy explore`: {}",
            distributed_paths.join(", ")
        )));
    }

    let jobs = if opt.fail_fast {
        1
    } else {
        opt.jobs.unwrap_or(1).max(1)
    };
    let mut outcome = TestOutcome::new(skipped, opt.record_trace_to.is_some());
    if jobs == 1 || filtered_paths.len() <= 1 {
        run_serial_tests(config, &filtered_paths, opt, seed, &mut outcome)?;
    } else {
        run_parallel_tests(config, &filtered_paths, opt, seed, jobs, &mut outcome);
    }

    let finished_at = wall_time_iso_utc();
    let (duration_ms, duration_ns) = crate::duration_fields(started.elapsed());
    let status = if outcome.failed == 0 {
        ExitStatus::Pass
    } else {
        ExitStatus::Fail
    };

    let artifacts_dir = config.runs_dir().join(&run_id);
    std::fs::create_dir_all(&artifacts_dir)?;
    let report_path = artifacts_dir.join("report.json");

    let summary = build_run_summary(
        status,
        RunMode::Test,
        run_id,
        seed,
        None,
        Some(report_path.to_string_lossy().to_string()),
        Some(artifacts_dir.to_string_lossy().to_string()),
        started_at,
        finished_at,
        duration_ms,
        duration_ns,
        Some(crate::TestCounts {
            passed: outcome.passed,
            failed: outcome.failed,
            skipped: outcome.skipped,
        }),
        outcome.memory_summary(),
        crate::collapse_findings(outcome.findings.clone()),
    );

    if let Some(record_base) = &opt.record_trace_to {
        write_test_traces(record_base, &outcome.trace_runs, opt.record_collision)?;
    }
    write_reporter_artifacts(&summary, &artifacts_dir, opt.reporter)?;
    write_summary_report(&summary, &report_path, &artifacts_dir, None)?;

    Ok(RunResult { summary })
}

fn run_serial_tests(
    config: &Config,
    filtered_paths: &[PathBuf],
    opt: &RunOptions,
    seed: u64,
    outcome: &mut TestOutcome,
) -> FozzyResult<()> {
    for path in filtered_paths {
        let scenario_seed =
            derive_test_seed(seed, filtered_paths.len(), outcome.total_runs(), path);
        let run = run_scenario_inner(
            config,
            RunMode::Test,
            ScenarioPath::new(path.clone()),
            scenario_seed,
            opt.det,
            opt.timeout,
            opt.proc_backend,
            opt.fs_backend,
            opt.http_backend,
            opt.memory.clone(),
        )?;
        outcome.record_run(TestRunRecord {
            ordinal: outcome.total_runs(),
            seed: scenario_seed,
            run,
        });
        if opt.fail_fast && outcome.failed > 0 {
            break;
        }
    }
    Ok(())
}

fn run_parallel_tests(
    config: &Config,
    filtered_paths: &[PathBuf],
    opt: &RunOptions,
    seed: u64,
    jobs: usize,
    outcome: &mut TestOutcome,
) {
    let (tx, rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let mut in_flight = 0usize;
        let mut next = 0usize;
        while next < filtered_paths.len() || in_flight > 0 {
            while next < filtered_paths.len() && in_flight < jobs {
                let path = filtered_paths[next].clone();
                let tx = tx.clone();
                let memory = opt.memory.clone();
                let timeout = opt.timeout;
                let proc_backend = opt.proc_backend;
                let fs_backend = opt.fs_backend;
                let http_backend = opt.http_backend;
                let det = opt.det;
                let scenario_seed = derive_test_seed(seed, filtered_paths.len(), next, &path);
                let ordinal = next;
                scope.spawn(move || {
                    let result = run_scenario_inner(
                        config,
                        RunMode::Test,
                        ScenarioPath::new(path),
                        scenario_seed,
                        det,
                        timeout,
                        proc_backend,
                        fs_backend,
                        http_backend,
                        memory,
                    );
                    let _ = tx.send((ordinal, scenario_seed, result));
                });
                next += 1;
                in_flight += 1;
            }

            if in_flight > 0 {
                if let Ok((ordinal, scenario_seed, result)) = rx.recv() {
                    in_flight = in_flight.saturating_sub(1);
                    outcome
                        .parallel_results
                        .push((ordinal, scenario_seed, result));
                } else {
                    break;
                }
            }
        }
    });
    outcome
        .parallel_results
        .sort_by_key(|(ordinal, _, _)| *ordinal);
    let parallel_results = std::mem::take(&mut outcome.parallel_results);
    for (ordinal, scenario_seed, result) in parallel_results {
        match result {
            Ok(run) => outcome.record_run(TestRunRecord {
                ordinal,
                seed: scenario_seed,
                run,
            }),
            Err(err) => outcome.record_worker_error(err),
        }
    }
}

fn write_test_traces(
    record_base: &Path,
    runs: &[TestRunRecord],
    policy: RecordCollisionPolicy,
) -> FozzyResult<()> {
    if runs.is_empty() {
        return Ok(());
    }
    let mut ordered_runs = runs.to_vec();
    ordered_runs.sort_by_key(|run| run.ordinal);
    if ordered_runs.len() == 1 {
        let run = &ordered_runs[0];
        write_single_scenario_trace(
            record_base,
            &run.run,
            &Uuid::new_v4().to_string(),
            run.seed,
            policy,
            RunMode::Test,
            None,
            None,
        )?;
        return Ok(());
    }

    let parent = record_base
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let file_name = record_base
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("test-trace.fozzy");
    let base = if file_name.ends_with(".fozzy") {
        file_name.trim_end_matches(".fozzy")
    } else {
        file_name
    };
    std::fs::create_dir_all(parent)?;

    for (idx, run) in ordered_runs.iter().enumerate() {
        let out = parent.join(format!("{base}.{}.fozzy", idx + 1));
        write_single_scenario_trace(
            &out,
            &run.run,
            &Uuid::new_v4().to_string(),
            run.seed,
            policy,
            RunMode::Test,
            None,
            None,
        )?;
    }
    Ok(())
}

#[derive(Default)]
struct TestOutcome {
    passed: u64,
    failed: u64,
    skipped: u64,
    findings: Vec<Finding>,
    trace_runs: Vec<TestRunRecord>,
    memory_summary: crate::MemorySummary,
    has_memory: bool,
    record_traces: bool,
    parallel_results: Vec<(usize, u64, FozzyResult<ScenarioRun>)>,
}

impl TestOutcome {
    fn new(skipped: u64, record_traces: bool) -> Self {
        Self {
            skipped,
            record_traces,
            ..Self::default()
        }
    }

    fn total_runs(&self) -> usize {
        (self.passed + self.failed) as usize
    }

    fn record_run(&mut self, record: TestRunRecord) {
        let run = &record.run;
        self.findings.extend(run.findings.clone());
        if run.status == ExitStatus::Pass {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        if let Some(mem) = run.memory.as_ref() {
            self.has_memory = true;
            self.memory_summary.alloc_count = self
                .memory_summary
                .alloc_count
                .saturating_add(mem.summary.alloc_count);
            self.memory_summary.free_count = self
                .memory_summary
                .free_count
                .saturating_add(mem.summary.free_count);
            self.memory_summary.failed_alloc_count = self
                .memory_summary
                .failed_alloc_count
                .saturating_add(mem.summary.failed_alloc_count);
            self.memory_summary.in_use_bytes = self
                .memory_summary
                .in_use_bytes
                .saturating_add(mem.summary.in_use_bytes);
            self.memory_summary.peak_bytes =
                self.memory_summary.peak_bytes.max(mem.summary.peak_bytes);
            self.memory_summary.leaked_bytes = self
                .memory_summary
                .leaked_bytes
                .saturating_add(mem.summary.leaked_bytes);
            self.memory_summary.leaked_allocs = self
                .memory_summary
                .leaked_allocs
                .saturating_add(mem.summary.leaked_allocs);
        }
        if self.record_traces {
            self.trace_runs.push(record);
        }
    }

    fn record_worker_error(&mut self, err: FozzyError) {
        self.findings.push(Finding {
            kind: FindingKind::Checker,
            title: "test_worker_error".to_string(),
            message: err.to_string(),
            location: None,
        });
        self.failed += 1;
    }

    fn memory_summary(&self) -> Option<crate::MemorySummary> {
        if self.has_memory {
            Some(self.memory_summary.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
struct TestRunRecord {
    ordinal: usize,
    seed: u64,
    run: ScenarioRun,
}

fn derive_test_seed(suite_seed: u64, total: usize, ordinal: usize, path: &Path) -> u64 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&suite_seed.to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    bytes.extend_from_slice(&(ordinal as u64).to_le_bytes());
    bytes.extend_from_slice(path.to_string_lossy().as_bytes());
    let hash = blake3::hash(&bytes);
    let mut seed_bytes = [0u8; 8];
    seed_bytes.copy_from_slice(&hash.as_bytes()[..8]);
    u64::from_le_bytes(seed_bytes)
}

fn gen_seed() -> u64 {
    let mut seed = [0u8; 8];
    rand_core::OsRng.fill_bytes(&mut seed);
    u64::from_le_bytes(seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        FsBackend, HttpBackend, MemoryOptions, ProcBackend, ProfileCaptureLevel,
        RecordCollisionPolicy, Reporter, TraceFile,
    };

    fn write_memory_leak_scenario(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        std::fs::write(
            &path,
            r#"{
  "version": 1,
  "name": "memory-leak",
  "steps": [
    { "type": "memory_alloc", "bytes": 256, "key": "leak" }
  ]
}"#,
        )
        .expect("write scenario");
        path
    }

    fn write_multi_alloc_scenario(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        std::fs::write(
            &path,
            r#"{
  "version": 1,
  "name": "memory-multi-alloc",
  "steps": [
    { "type": "memory_alloc", "bytes": 64, "key": "a", "tag": "first" },
    { "type": "memory_alloc", "bytes": 64, "key": "b", "tag": "second" }
  ]
}"#,
        )
        .expect("write scenario");
        path
    }

    fn write_pass_scenario(root: &Path, name: &str) -> PathBuf {
        let path = root.join(name);
        let fixture = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/example.fozzy.json"),
        )
        .expect("read fixture");
        std::fs::write(&path, fixture).expect("write scenario");
        path
    }

    fn run_options(memory: MemoryOptions) -> RunOptions {
        RunOptions {
            det: true,
            seed: Some(7),
            timeout: None,
            reporter: Reporter::Json,
            record_trace_to: None,
            filter: None,
            jobs: None,
            fail_fast: false,
            record_collision: RecordCollisionPolicy::Overwrite,
            profile_capture: ProfileCaptureLevel::Baseline,
            proc_backend: ProcBackend::Scripted,
            fs_backend: FsBackend::Virtual,
            http_backend: HttpBackend::Scripted,
            memory,
        }
    }

    fn test_config(root: &Path) -> Config {
        Config {
            base_dir: root.join(".fozzy"),
            reporter: Reporter::Json,
            proc_backend: ProcBackend::Scripted,
            fs_backend: FsBackend::Virtual,
            http_backend: HttpBackend::Scripted,
            mem_track: false,
            mem_limit_mb: None,
            mem_fail_after: None,
            fail_on_leak: false,
            leak_budget: None,
            mem_artifacts: false,
            profile_heap_alloc_budget: None,
            profile_heap_in_use_budget: None,
            mem_fragmentation_seed: None,
            mem_pressure_wave: None,
        }
    }

    #[test]
    fn memory_activity_is_reported_even_without_explicit_track_flag() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-test-memory-activity-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let scenario = write_memory_leak_scenario(&root, "memory.leak.fozzy.json");
        let cfg = test_config(&root);

        let run = run_tests(
            &cfg,
            &[scenario.display().to_string()],
            &run_options(MemoryOptions {
                track: false,
                artifacts: false,
                ..MemoryOptions::default()
            }),
        )
        .expect("run tests");

        assert_eq!(run.summary.status, ExitStatus::Fail);
        assert_eq!(
            run.summary.memory.as_ref().map(|m| m.leaked_bytes),
            Some(256)
        );
        assert!(
            run.summary
                .findings
                .iter()
                .any(|f| f.title == "memory_leak")
        );
    }

    #[test]
    fn leak_budget_allows_bounded_leaks_without_warning_finding() {
        let root =
            std::env::temp_dir().join(format!("fozzy-test-memory-budget-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("mkdir");
        let scenario = write_memory_leak_scenario(&root, "memory.budget.fozzy.json");
        let cfg = test_config(&root);

        let run = run_tests(
            &cfg,
            &[scenario.display().to_string()],
            &run_options(MemoryOptions {
                track: false,
                leak_budget_bytes: Some(512),
                artifacts: false,
                ..MemoryOptions::default()
            }),
        )
        .expect("run tests");

        assert_eq!(run.summary.status, ExitStatus::Pass);
        assert_eq!(
            run.summary.memory.as_ref().map(|m| m.leaked_bytes),
            Some(256)
        );
        assert!(run.summary.findings.is_empty());
    }

    #[test]
    fn memory_alloc_callsites_are_distinct_per_step() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-test-memory-callsite-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let scenario = write_multi_alloc_scenario(&root, "memory.callsite.fozzy.json");
        let cfg = test_config(&root);

        let run = run_scenario_inner(
            &cfg,
            RunMode::Run,
            ScenarioPath::new(scenario),
            7,
            true,
            None,
            ProcBackend::Scripted,
            FsBackend::Virtual,
            HttpBackend::Scripted,
            MemoryOptions {
                track: false,
                artifacts: false,
                ..MemoryOptions::default()
            },
        )
        .expect("run scenario");

        let leaks = run.memory.expect("memory report").leaks;
        assert_eq!(leaks.len(), 2);
        assert_ne!(leaks[0].callsite_hash, leaks[1].callsite_hash);
    }

    #[test]
    fn recorded_test_traces_are_standalone_and_do_not_reuse_aggregate_identity() {
        let root = std::env::temp_dir().join(format!(
            "fozzy-test-recorded-trace-identity-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("mkdir");
        let first = write_pass_scenario(&root, "first.fozzy.json");
        let second = write_pass_scenario(&root, "second.fozzy.json");
        let cfg = test_config(&root);
        let record_base = root.join("recorded-test.fozzy");

        let run = run_tests(
            &cfg,
            &[first.display().to_string(), second.display().to_string()],
            &RunOptions {
                record_trace_to: Some(record_base.clone()),
                ..run_options(MemoryOptions::default())
            },
        )
        .expect("run tests");

        let first_trace =
            TraceFile::read_json(&root.join("recorded-test.1.fozzy")).expect("read first trace");
        let second_trace =
            TraceFile::read_json(&root.join("recorded-test.2.fozzy")).expect("read second trace");

        assert_ne!(
            first_trace.summary.identity.run_id, run.summary.identity.run_id,
            "recorded trace must not reuse aggregate test run id"
        );
        assert_ne!(
            second_trace.summary.identity.run_id, run.summary.identity.run_id,
            "recorded trace must not reuse aggregate test run id"
        );
        assert_ne!(
            first_trace.summary.identity.run_id, second_trace.summary.identity.run_id,
            "each recorded test trace must have its own execution identity"
        );
        assert!(first_trace.summary.identity.report_path.is_none());
        assert!(first_trace.summary.identity.artifacts_dir.is_none());
        assert!(second_trace.summary.identity.report_path.is_none());
        assert!(second_trace.summary.identity.artifacts_dir.is_none());
    }
}
