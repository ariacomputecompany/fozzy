use super::state::{FullRunState, ScenarioSelection};
use super::*;

pub(super) fn run_extended_surface(
    state: &mut FullRunState,
    config: &Config,
    selection: &ScenarioSelection,
    seed: Option<u64>,
    fuzz_time: std::time::Duration,
    explore_steps: u64,
    explore_nodes: usize,
    strict: bool,
) {
    run_fuzz(state, config, selection, seed, fuzz_time, strict);
    if state.should_abort() {
        return;
    }
    run_explore(
        state,
        config,
        selection,
        seed,
        explore_steps,
        explore_nodes,
        strict,
    );
    if state.should_abort() {
        return;
    }
    run_corpus(state, config);
    run_host_backends(state, config, selection, seed, strict);
    if state.should_abort() {
        return;
    }

    let env = fozzy::env_info(config);
    let (env_status, env_detail) = env_step_status(&env);
    state.push("env", env_status, env_detail);
}

fn run_fuzz(
    state: &mut FullRunState,
    config: &Config,
    selection: &ScenarioSelection,
    seed: Option<u64>,
    fuzz_time: std::time::Duration,
    strict: bool,
) {
    let Some(primary) = selection.step.as_ref() else {
        state.push_skipped("fuzz", "no step scenario found for scenario-backed fuzz");
        return;
    };

    let fuzz_trace = state.register_temp(std::env::temp_dir().join(format!(
        "fozzy-full-fuzz-{}.trace.fozzy",
        uuid::Uuid::new_v4()
    )));
    let fuzz_corpus_dir = state.register_temp(
        std::env::temp_dir().join(format!("fozzy-full-fuzz-corpus-{}", uuid::Uuid::new_v4())),
    );
    if let Err(err) = std::fs::create_dir_all(&fuzz_corpus_dir) {
        state.push("fuzz", FullStepStatus::Failed, err.to_string());
        return;
    }
    state.start_step(
        "fuzz",
        format!(
            "scenario={} corpus_dir={} time={}ms",
            primary.display(),
            fuzz_corpus_dir.display(),
            fuzz_time.as_millis()
        ),
    );
    match run_with_timeout(
        "fuzz",
        fuzz_time + EXECUTION_TIMEOUT,
        {
            let config = config.clone();
            let primary = primary.clone();
            let fuzz_trace = fuzz_trace.clone();
            let fuzz_corpus_dir = fuzz_corpus_dir.clone();
            let memory = selection.memory.clone();
            move || {
                Ok(fozzy::fuzz(
                    &config,
                    &FuzzTarget::Scenario { path: primary },
                    &FuzzOptions {
                        det: false,
                        mode: FuzzMode::Coverage,
                        seed,
                        time: Some(fuzz_time),
                        runs: None,
                        max_input_bytes: 4096,
                        corpus_dir: Some(fuzz_corpus_dir),
                        mutator: None,
                        shrink: true,
                        record_trace_to: Some(fuzz_trace),
                        reporter: Reporter::Json,
                        crash_only: false,
                        minimize: true,
                        record_collision: RecordCollisionPolicy::Overwrite,
                        profile_capture: ProfileCaptureLevel::Baseline,
                        memory,
                    },
                )?)
            }
        },
    ) {
        Ok(fuzz_run) => {
            let (status, detail) = run_summary_pass_status(
                &fuzz_run.summary,
                strict,
                resolved_workflow_seed(seed),
                RunMode::Fuzz,
            );
            state.push(
                "fuzz",
                status,
                format!(
                    "{detail} run_id={} scenario={}",
                    fuzz_run.summary.identity.run_id,
                    primary.display()
                ),
            );
        }
        Err(err) => state.abort_due_to_timeout(
            "fuzz",
            err.to_string(),
            "Coverage fuzzing did not finish in time; `fozzy full` uses an isolated temporary corpus now and aborts with a structured failure if fuzz still stalls."
                .to_string(),
        ),
    }
}

fn run_explore(
    state: &mut FullRunState,
    config: &Config,
    selection: &ScenarioSelection,
    seed: Option<u64>,
    explore_steps: u64,
    explore_nodes: usize,
    strict: bool,
) {
    let Some(distributed) = selection.distributed.as_ref() else {
        state.push_skipped(
            "explore",
            "no distributed scenario found; add tests/*.fozzy.json with `distributed` schema",
        );
        return;
    };

    state.start_step("explore", format!("scenario={}", distributed.display()));
    match run_with_timeout("explore", EXECUTION_TIMEOUT, {
        let config = config.clone();
        let distributed = distributed.clone();
        let memory = selection.memory.clone();
        move || {
            Ok(fozzy::explore(
                &config,
                ScenarioPath::new(distributed),
                &ExploreOptions {
                    seed,
                    time: None,
                    steps: Some(explore_steps),
                    nodes: Some(explore_nodes),
                    faults: None,
                    schedule: ScheduleStrategy::CoverageGuided,
                    checker: None,
                    record_trace_to: None,
                    shrink: true,
                    minimize: true,
                    reporter: Reporter::Json,
                    record_collision: RecordCollisionPolicy::Error,
                    profile_capture: ProfileCaptureLevel::Baseline,
                    memory,
                },
            )?)
        }
    }) {
        Ok(explore) => {
            let (status, detail) = run_summary_pass_status(
                &explore.summary,
                strict,
                resolved_workflow_seed(seed),
                RunMode::Explore,
            );
            state.push(
                "explore",
                status,
                format!("{detail} scenario={}", distributed.display()),
            );
        }
        Err(err) => state.abort_due_to_timeout(
            "explore",
            err.to_string(),
            "Distributed exploration did not finish in time; `fozzy full` stopped with a structured failure."
                .to_string(),
        ),
    }
}

fn run_corpus(state: &mut FullRunState, config: &Config) {
    let corpus_dir = state.register_temp(
        std::env::temp_dir().join(format!("fozzy-full-corpus-{}", uuid::Uuid::new_v4())),
    );
    let seed_file = corpus_dir.join("seed.bin");
    let corpus_zip = state.register_temp(
        std::env::temp_dir().join(format!("fozzy-full-corpus-{}.zip", uuid::Uuid::new_v4())),
    );
    let corpus_import_dir = state.register_temp(
        std::env::temp_dir().join(format!("fozzy-full-corpus-import-{}", uuid::Uuid::new_v4())),
    );

    if let Err(err) = (|| -> anyhow::Result<()> {
        std::fs::create_dir_all(&corpus_dir)?;
        std::fs::write(&seed_file, b"fozzy-corpus-seed")?;
        Ok(())
    })() {
        for name in [
            "corpus_add",
            "corpus_list",
            "corpus_minimize",
            "corpus_export",
            "corpus_import",
        ] {
            state.push(name, FullStepStatus::Failed, err.to_string());
        }
        return;
    }

    match fozzy::corpus_command(
        config,
        &CorpusCommand::Add {
            dir: corpus_dir.clone(),
            file: seed_file,
        },
    ) {
        Ok(value) => {
            let (status, detail) = corpus_add_status(&value);
            state.push("corpus_add", status, detail);
        }
        Err(err) => state.push("corpus_add", FullStepStatus::Failed, err.to_string()),
    }
    match fozzy::corpus_command(
        config,
        &CorpusCommand::List {
            dir: corpus_dir.clone(),
        },
    ) {
        Ok(value) => {
            let (status, detail) = corpus_list_status(&value);
            state.push("corpus_list", status, detail);
        }
        Err(err) => state.push("corpus_list", FullStepStatus::Failed, err.to_string()),
    }
    match fozzy::corpus_command(
        config,
        &CorpusCommand::Minimize {
            dir: corpus_dir.clone(),
            budget: None,
        },
    ) {
        Ok(value) => {
            let (status, detail) = corpus_minimize_status(&value);
            state.push("corpus_minimize", status, detail);
        }
        Err(err) => state.push("corpus_minimize", FullStepStatus::Failed, err.to_string()),
    }
    match fozzy::corpus_command(
        config,
        &CorpusCommand::Export {
            dir: corpus_dir,
            out: corpus_zip.clone(),
        },
    ) {
        Ok(_) => {
            let (status, detail) = zip_artifact_status(&corpus_zip);
            state.push("corpus_export", status, detail);
        }
        Err(err) => state.push("corpus_export", FullStepStatus::Failed, err.to_string()),
    }
    match fozzy::corpus_command(
        config,
        &CorpusCommand::Import {
            zip: corpus_zip,
            out: corpus_import_dir,
        },
    ) {
        Ok(value) => {
            let (status, detail) = corpus_import_status(&value);
            state.push("corpus_import", status, detail);
        }
        Err(err) => state.push("corpus_import", FullStepStatus::Failed, err.to_string()),
    }
}

fn run_host_backends(
    state: &mut FullRunState,
    config: &Config,
    selection: &ScenarioSelection,
    seed: Option<u64>,
    strict: bool,
) {
    let Some(primary) = selection.host_step.as_ref() else {
        state.push_skipped("host_backends_run", "no host-backed step scenario found");
        return;
    };

    let host_trace = state.register_temp(std::env::temp_dir().join(format!(
        "fozzy-full-host-{}.trace.fozzy",
        uuid::Uuid::new_v4()
    )));
    state.start_step(
        "host_backends_run",
        format!("scenario={}", primary.display()),
    );
    match run_with_timeout("host_backends_run", EXECUTION_TIMEOUT, {
        let config = config.clone();
        let primary = primary.clone();
        let host_trace = host_trace.clone();
        let memory = selection.memory.clone();
        move || {
            Ok(fozzy::run_scenario(
                &config,
                ScenarioPath::new(primary),
                &RunOptions {
                    det: false,
                    seed,
                    timeout: None,
                    reporter: Reporter::Json,
                    record_trace_to: Some(host_trace.clone()),
                    filter: None,
                    jobs: None,
                    fail_fast: false,
                    record_collision: RecordCollisionPolicy::Error,
                    profile_capture: ProfileCaptureLevel::Baseline,
                    proc_backend: fozzy::ProcBackend::Host,
                    fs_backend: fozzy::FsBackend::Host,
                    http_backend: fozzy::HttpBackend::Host,
                    memory,
                },
            )?)
        }
    }) {
        Ok(host_run) => {
            let (run_status, run_detail) = run_summary_pass_status(
                &host_run.summary,
                strict,
                resolved_workflow_seed(seed),
                RunMode::Run,
            );
            let (trace_status, trace_detail) = host_backed_trace_status(&host_trace);
            let status = if matches!(run_status, FullStepStatus::Passed)
                && matches!(trace_status, FullStepStatus::Passed)
            {
                FullStepStatus::Passed
            } else {
                FullStepStatus::Failed
            };
            state.push(
                "host_backends_run",
                status,
                format!("{run_detail}; {trace_detail}"),
            );
        }
        Err(err) => state.abort_due_to_timeout(
            "host_backends_run",
            err.to_string(),
            "Host-backed execution did not finish in time; `fozzy full` stopped with a structured failure."
                .to_string(),
        ),
    }
}
