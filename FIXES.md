# FIXES

Cleaned backlog from the full source audit.

Structure:
- Quick wins first: smaller, high-signal fixes that improve correctness, UX, and trust quickly.
- Architectural issues second: deeper system changes that need broader design work.

## Quick Wins First

### CLI And Artifact Correctness

- ✅ Fix trace metadata drift so recorded traces always embed the real final `tracePath`.
  Why:
  Recorded traces can be written successfully while the summary inside the trace omits or misstates the final output path.
  Impact:
  This creates misleading artifact metadata and can confuse downstream reporting and tooling.
  Evidence:
  [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:1008), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:1022), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:818), [src/runtime/tracefile.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/tracefile.rs:278)
  Done when:
  - ✅ `run --record` writes a trace whose embedded summary path matches the actual file written.
  - ✅ `test --record` does the same in both overwrite and append modes.
  - ✅ Regression tests cover append-mode collision handling.

- ✅ Make `fozzy test` fail clearly when the caller explicitly supplies a nonexistent scenario path.
  Why:
  Today a mixed invocation can pass if one input exists and another explicit file path is wrong.
  Impact:
  This silently narrows the executed test set and can produce a false-green result in CI or release gating.
  Evidence:
  Discovery and empty-match handling live in [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:522), with file matching in [src/platform/fsutil.rs](/Users/deepsaint/Desktop/fozzy/src/platform/fsutil.rs:12).
  Done when:
  - ✅ Explicit missing paths cause a hard failure with a clear error.
  - ✅ Glob patterns still preserve normal glob semantics.
  - ✅ Mixed literal-path and glob invocations are covered by tests.

- ✅ Make `fozzy init` honor `--config <path>` instead of always writing `fozzy.toml`.
  Why:
  The CLI exposes a custom config path but initialization still writes the default filename.
  Impact:
  This breaks user expectations and makes scripted bootstrapping unreliable.
  Evidence:
  [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:327), [src/main.rs](/Users/deepsaint/Desktop/fozzy/src/main.rs:29)
  Done when:
  - ✅ Default init still creates `fozzy.toml`.
  - ✅ `--config custom.toml init` writes `custom.toml`.
  - ✅ Force/non-force behavior is tested for custom config paths.

- ✅ Stop default `fozzy test` runs from silently skipping distributed scenarios while still reporting overall success.
  Why:
  The default test discovery can find distributed scenarios, but the runner skips them and can still return `status=pass`.
  Impact:
  This is easy to misread as “all discovered tests passed” when some were never executed.
  Evidence:
  [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:523), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:580), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:721)
  Done when:
  - ✅ Mixed regular/distributed discovery no longer produces a misleading false-green result.
  - ✅ The final contract is explicit: fail, opt-in skip, or separate discovery domains.
  - ✅ Docs and init scaffolding match the final behavior.

### Validation And Contract Drift

- ✅ Make distributed-scenario validation consistent across `fozzy validate` and `fozzy explore`.
  Why:
  A distributed scenario can be rejected by validation and still execute successfully through explore.
  Impact:
  “Valid” and “runnable” do not currently mean the same thing, which weakens trust in both commands.
  Evidence:
  Validator in [src/model/scenario.rs](/Users/deepsaint/Desktop/fozzy/src/model/scenario.rs:502), validation call site in [src/main.rs](/Users/deepsaint/Desktop/fozzy/src/main.rs:1284), explore loading in [src/modes/explore.rs](/Users/deepsaint/Desktop/fozzy/src/modes/explore.rs:546)
  Done when:
  - ✅ A scenario rejected by `validate` is also rejected by `explore`, unless an explicit permissive mode exists.
  - ✅ Missing topology declarations are not silently synthesized during execution.
  - ✅ Regression tests cover malformed distributed scenarios.

- ✅ Make scenario validation recurse into nested `assert_throws` and `assert_rejects` blocks.
  Why:
  Top-level validation does not currently validate the nested step programs inside these wrappers.
  Impact:
  Malformed nested DSL can slip through preflight and then be treated as an “expected” failure, producing a false-green scenario result.
  Evidence:
  Top-level validation in [src/model/scenario.rs](/Users/deepsaint/Desktop/fozzy/src/model/scenario.rs:445), nested execution in [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:3973)
  Done when:
  - ✅ Nested invalid steps fail validation before runtime.
  - ✅ The same validation rules apply at top level and in nested blocks.
  - ✅ Tests cover nested invalid durations and invalid field combinations.

- ✅ Stop silently discarding source/scenario read failures in topology mapping.
  Why:
  `fozzy map` currently skips unreadable files and dropped scenarios without surfacing that the report is incomplete.
  Impact:
  The command can overstate coverage confidence and under-report risk.
  Evidence:
  [src/cmd/map_cmd.rs](/Users/deepsaint/Desktop/fozzy/src/cmd/map_cmd.rs:747), [src/cmd/map_cmd.rs](/Users/deepsaint/Desktop/fozzy/src/cmd/map_cmd.rs:753), [src/cmd/map_cmd.rs](/Users/deepsaint/Desktop/fozzy/src/cmd/map_cmd.rs:685)
  Done when:
  - ✅ Unreadable source files are reported explicitly.
  - ✅ Invalid or unreadable scenario files are reported explicitly.
  - ✅ JSON output includes structured degraded-confidence metadata.

### SDK And Local Developer Experience

- ✅ Harden the TypeScript SDK `stream()` path so spawn failures become normal SDK errors.
  Why:
  `stream()` does not currently install an `error` handler on the child process.
  Impact:
  Missing binaries or spawn failures can crash the consumer’s Node process instead of surfacing a catchable SDK error.
  Evidence:
  [sdk-ts/src/index.ts](/Users/deepsaint/Desktop/fozzy/sdk-ts/src/index.ts:292), reference behavior in [sdk-ts/src/index.ts](/Users/deepsaint/Desktop/fozzy/sdk-ts/src/index.ts:470)
  Done when:
  - ✅ Missing binary errors are catchable.
  - ✅ Permission-denied spawn errors are catchable.
  - ✅ Normal streaming behavior still works.

- ✅ Consolidate duplicated run/test summary finalization and artifact-writing logic.
  Why:
  Similar mechanics are implemented in multiple places with slightly different behavior.
  Impact:
  This is how metadata drift and subtle CLI inconsistency happen.
  Evidence:
  [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:522), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:809), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:1008)
  Done when:
  - ✅ Run/test/replay/shrink flows use the same summary and artifact conventions where applicable.
  - ✅ Collision-policy handling is consistent.
  - ✅ Regression coverage exists for the shared logic.

- ✅ Clean up checked-in runtime artifacts and profiling outputs at repo root.
  Why:
  The repo currently contains many trace/profile outputs alongside source files.
  Impact:
  This adds noise to audits and reviews and can interfere with tooling signal quality.
  Done when:
  - ✅ Incidental runtime outputs are ignored or moved out of the repo root.
  - ✅ Intentional fixtures remain documented and clearly separated.

- ✅ Clarify or remove legacy config-loading pathways that no longer match CLI behavior.
  Why:
  The CLI now exits on config parse/read errors, but a fallback helper still exists that warns and silently defaults.
  Impact:
  This can confuse future contributors about the intended config contract.
  Evidence:
  [src/platform/config.rs](/Users/deepsaint/Desktop/fozzy/src/platform/config.rs:122), [src/main.rs](/Users/deepsaint/Desktop/fozzy/src/main.rs:674)
  Done when:
  - ✅ The intended config-loading contract is explicit.
  - ✅ Library and CLI behavior are documented or unified.

## Deeper Architectural Issues Second

### Runtime Safety And Resource Control

- [x] Make host-backed runtime operations respect Fozzy timeouts while the host call is actually in flight.
  Why:
  Host HTTP and host process steps block inside the step itself, while timeout checks happen only before and after step execution.
  Impact:
  A hung host process or slow endpoint can stall a run indefinitely despite `--timeout`.
  Evidence:
  [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:1644), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:1718), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:3049), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:3351)
  Done when:
  - [x] Hung host proc calls time out promptly.
  - [x] Hung host HTTP calls time out promptly.
  - [x] Timeout behavior is recorded and replayed coherently.

- [x] Enforce host stdout/stderr and HTTP body limits during streaming, not after full buffering.
  Why:
  Current size checks happen after the whole payload has already been loaded into memory.
  Impact:
  The limits look protective but do not actually prevent memory spikes.
  Evidence:
  [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:3363), [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:4257)
  Done when:
  - [x] Oversized host proc output is cut off safely during read.
  - [x] Oversized host HTTP bodies are aborted during read.
  - [x] The failure mode remains debuggable.

### Caching And Lifecycle Semantics

- [x] Rework process-global scenario caches so they do not serve stale content forever and do not grow without bound.
  Why:
  Parsed scenarios and compiled fuzz targets are cached globally for the lifetime of the process.
  Impact:
  Long-lived embeddings can observe stale scenarios and unbounded cache growth.
  Evidence:
  [src/model/scenario.rs](/Users/deepsaint/Desktop/fozzy/src/model/scenario.rs:393), [src/modes/fuzz.rs](/Users/deepsaint/Desktop/fozzy/src/modes/fuzz.rs:737)
  Done when:
  - [x] Cache lifecycle is explicit.
  - [x] Scenario edits can be observed correctly, or the cache semantics are deliberately bounded and documented.
  - [x] Repeated unique temp paths do not cause unbounded growth.

### Codebase Structure

- [ ] Break up oversized control-center modules.
  Why:
  Several core files are extremely large and mix unrelated responsibilities.
  Impact:
  This increases review cost, onboarding cost, and the likelihood that fixes land in one path but not its sibling path.
  Primary hotspots:
  - [src/runtime/engine.rs](/Users/deepsaint/Desktop/fozzy/src/runtime/engine.rs:1)
  - [src/main.rs](/Users/deepsaint/Desktop/fozzy/src/main.rs:1)
  - [src/cmd/profile_cmd.rs](/Users/deepsaint/Desktop/fozzy/src/cmd/profile_cmd.rs:1)
  Done when:
  - [x] Host backend logic is separated cleanly.
  - [x] Trace/summary finalization helpers are separated cleanly.
  - [x] CLI dispatch has clearer module boundaries.
  - [ ] Profile subcommands have clearer module boundaries.
  - [ ] Behavior stays stable under regression tests.

## Suggested Order

### First Pass

- ✅ Trace metadata consistency
- ✅ Explicit missing-path failures in `fozzy test`
- ✅ `init --config` path handling
- ✅ False-green distributed-scenario handling in `fozzy test`
- ✅ Distributed validation parity
- ✅ Recursive nested-step validation
- ✅ Topology mapper degraded-read reporting
- ✅ SDK `stream()` error handling
- ✅ Shared summary/artifact finalization cleanup
- ✅ Repo artifact cleanup
- ✅ Config-loading contract cleanup

### Second Pass

- [x] Scenario cache lifecycle redesign
- [x] Host timeout enforcement
- [x] Streaming resource limits for host I/O
- [ ] Large-module refactors

## Validation Expectations

- ✅ New behavior is covered by focused regression tests.
- ✅ Runtime-impacting fixes are validated with Fozzy-first flows:
  - `fozzy doctor --deep --scenario <scenario> --runs 5 --seed <seed> --json`
  - `fozzy test --det --strict <scenarios...> --json`
  - `fozzy run ... --det --record <trace.fozzy> --json`
  - `fozzy trace verify <trace.fozzy> --strict --json`
  - `fozzy replay <trace.fozzy> --json`
  - `fozzy ci <trace.fozzy> --json`
