//! Fuzzing engine (v0.2): mutation + simple coverage feedback + crash recording.
//!
//! This is intentionally self-contained so fuzz targets can evolve without
//! entangling the core scenario runner.

#[path = "fuzz/corpus.rs"]
mod corpus;
#[path = "fuzz/exec.rs"]
mod exec;
#[path = "fuzz/report.rs"]
mod report;
#[path = "fuzz/run.rs"]
mod run;
#[path = "fuzz/types.rs"]
mod types;
#[path = "fuzz/util.rs"]
mod util;

pub use run::{fuzz, replay_fuzz_trace, shrink_fuzz_trace};
pub use types::{FuzzCoverageStats, FuzzMode, FuzzOptions, FuzzTarget, FuzzTrace};

pub(crate) use corpus::{
    crash_trace_output_path, load_corpus, persist_corpus_input, persist_crash_input,
    persist_crash_min_input,
};
pub(crate) use exec::{
    execute_target, fuzz_exec_memory, fuzz_trace_memory_options, target_string,
};
pub(crate) use report::{heap_budget_policy, should_emit_heavy_artifacts};
pub(crate) use util::{
    gen_seed, hex_decode, minimize_input, mutate_bytes, rng_from_seed, seed_from_input,
    stable_edge,
};

#[cfg(test)]
pub(crate) use corpus::with_numeric_suffix;

#[cfg(test)]
#[path = "fuzz/tests.rs"]
mod tests;
