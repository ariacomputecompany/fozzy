use super::*;

pub(super) struct FullRunState {
    temp_paths: Vec<PathBuf>,
    steps: Vec<FullStepResult>,
    pub(super) guidance: Vec<String>,
    pub(super) shrink_classification: Option<String>,
    pub(crate) strict: bool,
    unsafe_mode: bool,
    scenario_root: PathBuf,
    stream_json_events: bool,
    abort_requested: bool,
}

impl FullRunState {
    pub(super) fn new(
        strict: bool,
        unsafe_mode: bool,
        scenario_root: &Path,
        stream_json_events: bool,
    ) -> Self {
        Self {
            temp_paths: Vec::new(),
            steps: Vec::new(),
            guidance: vec![
                "Use the entire command surface by default; skip only when required inputs for a command are genuinely missing.".to_string(),
                "Keep strict mode enabled (default) so warning-class signals fail fast; use --unsafe only for intentional relaxed passes.".to_string(),
                "Place executable scenarios under tests/**/*.fozzy.json; distributed scenarios should use the `distributed` schema.".to_string(),
            ],
            shrink_classification: None,
            strict,
            unsafe_mode,
            scenario_root: scenario_root.to_path_buf(),
            stream_json_events,
            abort_requested: false,
        }
    }

    pub(super) fn register_temp(&mut self, path: PathBuf) -> PathBuf {
        self.temp_paths.push(path.clone());
        path
    }

    pub(super) fn push(&mut self, name: &str, status: FullStepStatus, detail: impl Into<String>) {
        let detail = detail.into();
        self.emit_json_event("step_finished", name, Some(status.clone()), &detail);
        self.steps.push(FullStepResult {
            name: name.to_string(),
            status,
            detail,
        });
    }

    pub(super) fn push_skipped(&mut self, name: &str, detail: impl Into<String>) {
        self.push(name, FullStepStatus::Skipped, detail);
    }

    pub(super) fn push_skipped_many(&mut self, names: &[&str], detail: &str) {
        for name in names {
            self.push_skipped(name, detail.to_string());
        }
    }

    pub(super) fn start_phase(&self, name: &str, detail: impl Into<String>) {
        self.emit_json_event("phase_started", name, None, &detail.into());
    }

    pub(super) fn start_step(&self, name: &str, detail: impl Into<String>) {
        self.emit_json_event("step_started", name, None, &detail.into());
    }

    pub(super) fn abort_due_to_timeout(
        &mut self,
        name: &str,
        detail: impl Into<String>,
        guidance: impl Into<String>,
    ) {
        self.abort_requested = true;
        self.push(name, FullStepStatus::Failed, detail.into());
        self.guidance.push(guidance.into());
    }

    pub(super) fn should_abort(&self) -> bool {
        self.abort_requested
    }

    pub(super) fn finish(mut self, skip_steps: &[String], required_steps: &[String]) -> FullReport {
        apply_full_policy_filters(&mut self.steps, skip_steps, required_steps);
        let report = FullReport {
            schema_version: "fozzy.full_report.v1".to_string(),
            strict: self.strict,
            unsafe_mode: self.unsafe_mode,
            scenario_root: self.scenario_root.display().to_string(),
            guidance: self.guidance,
            shrink_classification: self.shrink_classification,
            steps: self.steps,
        };
        for path in self.temp_paths {
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
        }
        report
    }

    fn emit_json_event(
        &self,
        event: &'static str,
        step: &str,
        status: Option<FullStepStatus>,
        detail: &str,
    ) {
        if !self.stream_json_events {
            return;
        }
        let out = serde_json::json!({
            "schemaVersion": "fozzy.full_progress.v1",
            "event": event,
            "step": step,
            "status": status,
            "detail": detail,
        });
        eprintln!("{out}");
    }
}

#[derive(Debug, Clone)]
pub(super) struct ScenarioSelection {
    pub(super) discovered: FullScenarioDiscovery,
    pub(super) step: Option<PathBuf>,
    pub(super) host_step: Option<PathBuf>,
    pub(super) distributed: Option<PathBuf>,
    pub(super) memory: MemoryOptions,
}

#[derive(Debug, Default, Clone)]
pub(super) struct PrimaryRunState {
    pub(super) primary_trace: Option<PathBuf>,
    pub(super) shrunk_trace: Option<PathBuf>,
    pub(super) primary_status: Option<ExitStatus>,
    pub(super) shrunk_status: Option<ExitStatus>,
}
