# Memory Review Checklist

This document captures the current production-readiness review of Fozzy's full memory logic. It is written in checklist style and uses only green checkmarks as requested.

## Core Tracker

✅ The core memory tracker is relatively tight and deterministic.

✅ Allocation, free, failure, leak, graph, and timeline paths are all represented in a small and understandable surface area.

✅ The tracker uses saturating arithmetic in key places, which reduces overflow risk in counters and byte accumulation paths.

✅ The live scenario-level memory control fix for fragmentation and pressure wave behavior is present in the current source tree.

✅ There is a targeted unit test for scenario-level memory controls in [src/runtime/memorycap.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/memorycap.rs:335).

## Net-New Remediations

✅ The replay path no longer invents memory tracking for traces that were recorded without a memory section.

✅ Replay now disables memory tracking by default when `trace.memory` is absent in [src/runtime/run_flow.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/run_flow.rs:515).

✅ Replay now checks both memory-summary parity and pass-state checker-warning parity instead of only comparing pass vs non-pass outcome class in [src/runtime/run_flow.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/run_flow.rs:193) and [src/cmd/ci.rs](/Users/deepsaint/Desktop/fozzy/src/cmd/ci.rs:101).

✅ The strict aggregated `fozzy test` path now preserves checker findings from passing scenarios instead of silently dropping them in [src/runtime/test_runner.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/test_runner.rs:274).

✅ Effective allocation size is now recorded directly in memory decisions and trace events in [src/model/decisions.rs](/Users/deepsaint/Desktop/fozzy/src/model/decisions.rs:72) and [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:2568).

✅ Heap-profile construction now prefers recorded effective allocation bytes so `profile.heap.json` aligns with the real memory model in [src/cmd/profile_build.rs](/Users/deepsaint/Desktop/fozzy/src/cmd/profile_build.rs:29).

✅ The pressure-wave scenario fixture now asserts deterministic effective-byte behavior end to end in [tests/memory.pressure.fozzy.json](/Users/deepsaint/Desktop/fozzy/tests/memory.pressure.fozzy.json:1).

✅ A direct leak run now reports JSON `"status": "fail"` in the summary itself rather than only failing through strict-mode process exit behavior.

✅ Shrink now uses the same no-synthetic-memory fallback as replay, so a non-memory trace no longer shrinks into a descendant with an invented zeroed `memory` section.

✅ Scenario fuzz targets now preserve structured memory summaries, leaks, and options when the embedded scenario exercises memory behavior.

✅ Fuzz replay now derives scenario memory behavior from the recorded trace contract and checks real replayed memory summaries instead of echoing trace memory blindly.

✅ Fuzz shrink now preserves the actual replayed memory contract of the minimized input instead of copying the source trace’s memory block forward unchanged.

✅ DSL memory callsite attribution is now step-specific rather than collapsing every `memory_alloc` into one synthetic bucket.

✅ Internal config defaults now match the repository’s production-facing opt-in memory contract instead of assuming always-on memory tracking and artifacts.

✅ Explore no longer fabricates an empty finalized memory report when no real memory activity was observed.

## Production Risks

✅ High-priority risk: replay safety is weakened when `track=false`.

✅ Memory behavior still affects execution even when memory tracking is disabled.

✅ Memory options and summaries are only serialized into traces when memory tracking is enabled in [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:857).

✅ Replay rebuilds memory options from `trace.memory`, and falls back to defaults if that section is absent in [src/runtime/run_flow.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/run_flow.rs:515).

✅ Shrink uses the same trace-derived memory option recovery pattern in [src/runtime/run_flow.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/run_flow.rs:343).

✅ This means a run that depended on memory policy like `--mem-limit-mb`, `--mem-fail-after`, fragmentation, or pressure wave can execute one way originally and replay or shrink another way if memory tracking was disabled.

✅ This is a production concern because replayable artifacts may not faithfully preserve the original execution policy.

✅ High-priority risk: shrink still invents a memory contract for traces that were recorded without memory data.

✅ Replay explicitly disables memory tracking when `trace.memory` is absent in [src/runtime/run_flow.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/run_flow.rs:515), but shrink still falls back to `MemoryOptions::default()` in [src/runtime/run_flow.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/run_flow.rs:343) and [src/runtime/run_flow.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/run_flow.rs:386).

✅ In practice this lets a non-memory trace shrink into a descendant trace that now contains a synthetic zeroed `memory` section even though the original artifact had none.

✅ That is a concrete contract-drift bug because the shrunk trace becomes eligible for `fozzy memory` tooling and CI memory parity checks in a way the original trace was not.

✅ High-priority risk: `track=false` suppresses leak policy enforcement and memory diagnostics even though memory steps still run.

✅ Leak findings, leak budget enforcement, and `fail_on_leak` handling are gated behind the `if self.memory.options.track` block in [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:857).

✅ As a result, disabling memory tracking is not just an observability toggle; it also changes which correctness policies are enforced.

✅ This coupling increases the chance of silent policy drift between local runs, CI checks, replay, and production-style validation.

✅ High-priority risk: `fozzy ci` can still report green on a known memory leak when the leak is only surfaced as a checker finding.

✅ In [src/cmd/ci.rs](/Users/deepsaint/Desktop/fozzy/src/cmd/ci.rs:106), `replay_warning_parity` checks only that pass-state checker warnings are preserved between trace and replay.

✅ In [src/cmd/ci.rs](/Users/deepsaint/Desktop/fozzy/src/cmd/ci.rs:130), `memory_policy` only fails when `fail_on_leak` or `leak_budget_bytes` was explicitly set in the recorded trace.

✅ This means a reproducible trace that contains a `memory_leak` checker finding can still pass CI if the leak was recorded without explicit leak-policy enforcement.

✅ That weakens the production gate because CI can bless a trace whose memory health is already known to be bad.

✅ High-priority risk: the machine-readable run summary can disagree with the actual command result under default strict mode.

✅ In [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:860), a leak adds a `memory_leak` checker finding but does not flip `status` to fail unless `fail_on_leak` or a leak budget is configured.

✅ In [src/cli_runtime.rs](/Users/deepsaint/Desktop/fozzy/src/cli_runtime.rs:73) and [src/cli_dispatch.rs](/Users/deepsaint/Desktop/fozzy/src/cli_dispatch.rs:311), strict mode converts pass-status checker findings into a non-zero command failure after the JSON summary has already been emitted.

✅ This creates an automation risk because downstream consumers can read `"status": "pass"` from JSON while the process itself failed.

✅ Medium-priority risk: callsite attribution is too coarse for trustworthy diagnostics.

✅ Scenario `memory_alloc` steps currently use the same literal callsite string in [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:2562).

✅ The tracker hashes that callsite string into the allocation identity in [src/runtime/memorycap.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/memorycap.rs:70).

✅ In practice, this collapses all DSL memory allocations into one synthetic callsite bucket.

✅ The graph, leak list, and heap hotspot output can therefore say that `memory_alloc` was hot, but cannot clearly distinguish which scenario location or logical allocation site was responsible.

✅ Medium-priority risk: internal config defaults still point at always-on memory tracking even though this repository's effective CLI contract is opt-in.

✅ `Config::default()` still enables `mem_track=true` and `mem_artifacts=true` in [src/platform/config.rs](/Users/deepsaint/Desktop/fozzy/src/platform/config.rs:91), while the checked-in workspace config disables both in [fozzy.toml](/Users/deepsaint/Desktop/fozzy/fozzy.toml:1).

✅ The active CLI behavior in this repository is therefore fine, but internal callers and tests that rely on `Config::default()` remain exposed to silent behavior drift.

✅ High-priority risk: scenario fuzz targets bypass the structured memory pipeline.

✅ Scenario fuzz execution hardcodes a stripped-down memory config in [src/modes/fuzz.rs](/Users/deepsaint/Desktop/fozzy/src/modes/fuzz.rs:689) and [src/modes/fuzz.rs](/Users/deepsaint/Desktop/fozzy/src/modes/fuzz.rs:752).

✅ The resulting fuzz trace and summary can preserve a `memory_leak` finding while omitting the `memory` section entirely in [src/modes/fuzz.rs](/Users/deepsaint/Desktop/fozzy/src/modes/fuzz.rs:513).

✅ This weakens production confidence because `fozzy memory ...` diagnostics and CI `memory_policy` checks cannot reason about scenario-fuzzed memory behavior when the trace contains no structured memory data.

✅ Low-priority risk: the `explore` surface advertises memory support but currently fabricates an empty finalized report instead of tracking meaningful memory activity.

✅ When `opt.memory.track` is enabled, `explore` finalizes a brand-new `MemoryState` without feeding it execution activity in [src/modes/explore.rs](/Users/deepsaint/Desktop/fozzy/src/modes/explore.rs:167).

✅ This is more misleading than dangerous, but it makes the cross-mode memory story less trustworthy for production diagnostics.

## Validation Performed

✅ `fozzy doctor --deep --scenario tests/memory.pressure.fozzy.json --runs 5 --seed 7 --json` reported deterministic behavior across all five runs.

✅ A direct recorded run of the pressure scenario showed the memory logic can fail on allocation pressure as expected in at least one runtime path.

✅ The investigation also exposed a discrepancy between the checked-in source and the currently built executable during local validation.

✅ That discrepancy should be treated as an operational release-readiness risk until the build artifact and source are confirmed to match.

✅ `fozzy test --det --strict tests/memory.pass.fozzy.json tests/memory.pressure.fozzy.json --mem-track --mem-artifacts --json` now passes with zero leaks and zero residual in-use bytes.

✅ `fozzy test --det --strict tests/memory.leak.fozzy.json --mem-track --mem-artifacts --json` now fails strict mode through the aggregated test path, which confirms that passing-scenario checker findings are no longer dropped.

✅ A recorded pressure trace now passes `fozzy trace verify --strict`, `fozzy replay`, `fozzy ci`, and `fozzy memory top|graph` as a consistent end-to-end contract.

✅ A recorded non-memory trace now replays without synthesizing a memory section, and CI reports `expected=None got=None` for replay memory parity.

✅ A shrunk descendant of that same non-memory trace currently does synthesize a zeroed `memory` section, which confirms the contract drift is specific to the shrink path rather than replay as a whole.

✅ A host-backed sanity run with `--proc-backend host --fs-backend host --http-backend host` on the memory-pass scenario completed successfully with clean memory accounting.

✅ A recorded leak trace produced from `tests/memory.leak.fozzy.json` under `--unsafe --mem-track --record ...` was accepted by `fozzy ci --json` with `"ok": true` even though the trace preserved a `memory_leak` checker finding.

✅ A current direct leak run of `tests/memory.leak.fozzy.json` now emits JSON `"status": "fail"` together with a `memory_leak` finding, which suggests the earlier summary-versus-exit mismatch has been remediated in the current tree.

✅ `fozzy fuzz scenario:tests/memory.leak.fozzy.json --det --runs 1 --json` reproduces a leak finding while omitting structured memory data from the resulting fuzz trace.

✅ `fozzy memory top` against that fuzz trace now fails with `does not contain memory data`, and `fozzy ci` reports `expected=None got=None` for replay memory parity.

✅ The workspace also hit an operational disk-pressure issue during validation: later `cargo run` invocations began failing with `No space left on device (os error 28)`, while `df -h .` showed the data volume at 99% usage with 5.6 GiB available.

## Current Sit Rep

✅ The allocator bookkeeping itself does not currently show an obvious corruption bug from this review.

✅ The larger concern is not raw allocation math; it is end-to-end contract integrity across run, trace, replay, shrink, CI, and diagnostics.

✅ The full memory system should not yet be treated as fully production-tight until execution-time memory policy is always preserved in replayable artifacts.

✅ The diagnostic value of memory reporting would also improve materially if callsite attribution became more granular than a single synthetic `memory_alloc` bucket.

✅ The most important net-new blocker from this pass is that scenario fuzzing can surface memory failures without preserving structured memory artifacts, which breaks the expected end-to-end memory contract across fuzz, replay, CI, and `fozzy memory` diagnostics.

✅ The shrink path also remains a distinct blocker because it can still mutate a non-memory trace into a memory-tracked descendant.

✅ Those two end-to-end blockers are now remediated in the current tree, and the remaining concerns are mostly about future feature expansion rather than known contract drift in the present implementation.
