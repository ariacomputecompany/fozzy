//! Core engine: scenario execution, deterministic runtime, record/replay, shrinking.

#[path = "engine/drivers.rs"]
mod drivers;
#[path = "engine/exec.rs"]
mod exec;
#[path = "engine/helpers.rs"]
mod helpers;
#[path = "engine/types.rs"]
mod types;

pub use drivers::should_emit_profile_artifacts;
pub(crate) use drivers::{
    run_embedded_scenario_inner, run_embedded_steps_for_fuzz, run_scenario_inner,
    run_scenario_replay_inner, shrink_status_matches,
};
pub(crate) use helpers::proc_unmatched_hint;
pub(crate) use types::ScenarioRun;
pub use types::{
    FsBackend, HttpBackend, InitTemplate, InitTestType, ProcBackend, ProfileCaptureLevel,
    RecordCollisionPolicy, ReplayOptions, RunOptions, RunResult, ShrinkMinimize, ShrinkOptions,
    ShrinkResult,
};
