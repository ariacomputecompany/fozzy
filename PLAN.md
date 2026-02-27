# Fozzy Production Execution Checklist

Fozzy is a deterministic full-stack testing platform built from first principles in Rust.

## Core Promise
- Deterministic execution universe (scheduler + time + RNG + capabilities)
- Replayable failures from artifacts
- Minimal shrinking for fast debugging
- Unified CLI for test/fuzz/explore/replay/shrink

## Non-Negotiables
- No shelling out to external test runners (Bun/Node/Jest/Mocha/etc) for engine execution.
- CLI commands execute via the Rust engine.
- SDKs are thin wrappers over the binary, never engine logic.
- Determinism and replay correctness outrank feature count.

## Current Production Readiness Snapshot (2026-02-18)
- ✅ Rust-native engine and CLI are in place.
- ✅ `fozzy usage` command exists for quick command selection guidance.
- ✅ Deterministic replay works for run/fuzz/explore traces.
- ✅ Core capabilities cover time/rng/fs/http/proc/network with deterministic replay decisions.
- ✅ Fuzzing and distributed exploration are partially implemented (remaining depth noted below).
- ✅ Hardening wave landed: checksum-backed traces, collision-safe recording, schema warnings on replay.
- ⬜ Full hardening/performance/audit requirements are still pending.

## Milestone Checklist

### M0 Foundations
- ✅ Single Rust crate + build pipeline
- ✅ Binary name `fozzy`
- ✅ CLI scaffold from `CLI.md`
- ✅ JSON output surfaces
- ✅ Semver baseline (`0.1.0`)

### M1 Deterministic Core
- ✅ Seeded deterministic RNG
- ✅ Virtual time with freeze/advance/sleep behavior
- ✅ Decision logging for replay
- ✅ Deterministic scheduler core (task queue + deterministic picks + schedule recording)
- ✅ Replay drift detection for scheduler decisions (run/explore)

### M2 Test Framework
- ✅ Scenario discovery and execution (`fozzy test`, `fozzy run`)
- ✅ Assertions (`ok/eq/ne/throws/rejects/eventually/never` + `fail`, KV assertions)
- ✅ Deterministic mode (`--det`) and seed controls
- ✅ Async-style assertion semantics (`eventually`/`never`) in deterministic polling form

### M3 Capability Virtualization
- ✅ Time capability (virtual clock)
- ✅ RNG capability (seeded + replayable)
- ✅ Filesystem overlay capability (write/read/snapshot/restore)
- ✅ Scripted HTTP mocking capability (`http_when` / `http_request`)
- ✅ Scripted process virtualization capability (`proc_when` / `proc_spawn`)
- ✅ Network simulation capability in both run/explore flows with delivery/drop replay decisions

### M4 Replay + Artifacts
- ✅ `.fozzy` trace format with versioning + metadata
- ✅ Replay command (`fozzy replay`)
- ✅ Artifact emission: `trace`, `events`, `report`
- ✅ Timeline artifact emission (`timeline.json`)
- ✅ Artifact diff depth (`fozzy artifacts diff`) includes file/report/trace deltas

### M5 Fuzzing Engine
- ✅ Mutation-based loop (`fozzy fuzz`)
- ✅ Coverage feedback loop + persisted `coverage.json` accounting
- ✅ Corpus storage + crash persistence
- ✅ Crash trace replay/shrink path
- ✅ `fuzz --record` now emits requested trace path for both pass and fail outcomes
- ✅ Target plugin registry wired (`fn:kv`, `fn:utf8`) with extensible dispatch
- ✅ Property mode wiring exists; richer property APIs pending
- ✅ Crash dedup/minimization is basic, not full production-grade
- ✅ Generalized target ecosystem has started (multiple built-ins), broader ecosystem still pending

### M6 Distributed Exploration
- ✅ Single-host multi-node deterministic simulation
- ✅ Message scheduling (`fifo`, `random`, `pct`)
- ✅ Partition/heal/crash/restart scripting
- ✅ Invariant checks + trace replay + schedule shrink
- ✅ CLI fault/checker presets wired (`--faults`, `--checker`)
- ✅ `--checker` now truly overrides scenario invariants (does not append)
- ✅ Additional invariant checkers (`kv_present_on_all`, `kv_node_equals`)
- ✅ Fault/search strategy depth is partial
- ✅ Expanded strategy suite (`fifo`, `bfs`, `dfs`, `random`, `pct`, `coverage_guided`)
- ✅ Schedule consistency fix: replication now uses per-key version ordering to avoid DFS stale-write divergence
- ⬜ Full checker ecosystem pending

### M7 Shrinking Engine
- ✅ Input/step shrinking for run traces
- ✅ Input shrinking for fuzz traces
- ✅ Schedule shrinking for explore traces
- ✅ Cross-dimension explore shrinking (`--minimize all`) now reduces schedule + setup fault steps
- ✅ Shrink status-preservation guard: passing traces stay passing after minimize/replay
- ⬜ Full node/fault/schedule/input joint minimization pending

### M8 TypeScript SDK
- ✅ Contract/spec documented in `SDK-TS.md`
- ✅ Production NPM package scaffolded in `sdk-ts/` with full CLI command parity wrapper
- ✅ Streaming helper (`stream(...)`) and scenario builder helpers (`ScenarioBuilder`, `DistributedScenarioBuilder`)
- ✅ Type-safe SDK pipeline: strict TS config, declaration output (`dist/index.d.ts`), prepack typecheck+build

### M9 CI + Reporting
- ✅ JSON + JUnit + HTML report outputs
- ✅ `fozzy report show` and basic `fozzy report query --jq` support
- ✅ `fozzy report query --jq` now supports array wildcard paths (e.g. `.findings[].title`)
- ✅ `fozzy report query --jq` accepts jq-style path ergonomics (`findings[0].title`, `$.findings[0].title`)
- ✅ `artifacts ls` supports both run-id and `.fozzy` trace paths
- ✅ Timeline artifact output (`timeline.json`) included in artifact listing
- ✅ Global CLI flags (like `--json`) are accepted before or after subcommand
- ✅ `--json` mode now emits JSON error envelopes for CLI parse/usage failures (for example missing required args), not plain-text parse output
- ✅ CI flaky analysis command added (`fozzy report flaky ...`); richer policy semantics still pending
- ✅ Full `jq` parity is still pending (advanced filters/functions not implemented)
- ✅ `report query` now supports `--list-paths` shape introspection
- ✅ Missed `report query` paths now return "did you mean ..." suggestions (for example `identity.runId`)
- ✅ `report flaky` now reports `flakeRatePct` and supports `--flake-budget <pct>` enforcement

### M10 Hardening
- ✅ Determinism audit command added (`fozzy doctor --deep --scenario ... --runs ... --seed ...`)
- ✅ Performance pass: scheduler decision labels compacted to step-kind identifiers
- ✅ Trace-size pass: traces write compact JSON by default (`FOZZY_TRACE_PRETTY=1` for pretty)
- ✅ Trace format compatibility tests added (legacy/new decision schema parsing)
- ✅ UX polish and diagnostics are partial (shrink default path now deterministic and explicit)
- ✅ Deterministic timeout semantics fixed: `--timeout` now applies to virtual elapsed time under `--det`
- ✅ Atomic trace writes prevent concurrent same-path `--record` corruption
- ✅ Explicit `--record-collision=error|overwrite|append` policy on `run/test/fuzz/explore`
- ✅ Deterministic active-writer lock conflict error for same-path `--record`
- ✅ Trace integrity checksum validation on read/replay
- ✅ `fozzy trace verify <path>` integrity + schema warning command
- ✅ Replay now emits explicit stale-schema warnings for older trace versions
- ✅ Artifacts export now fails non-zero when no artifacts are produced or input run/trace is missing
- ✅ Artifacts export ZIP writes are atomic (no empty/corrupt partial output on failure)
- ✅ CI gate added: export artifact ZIP must exist and pass `unzip -t` integrity validation
- ✅ Canonical local gate command added: `fozzy ci <trace>` (trace verify + replay outcome class + artifacts zip integrity + optional flake budget)
- ✅ Deterministic run manifest artifact added: `manifest.json` with fixed schema (`fozzy.run_manifest.v1`) across run modes
- ✅ Reproducer pack export added: `fozzy artifacts pack <run|trace> --out <dir|zip>` including trace/report/events + env/version/commandline metadata
- ✅ `artifacts pack/export --out <dir>` and `corpus import --out <dir>` now preflight all targets so symlink-block failures are atomic (no partial outputs written)
- ✅ `artifacts pack/export` now fail non-zero on incomplete run directories (required bundle files missing) instead of emitting partial bundles
- ✅ Run-id `artifacts pack/export <run>` now honor normal run contracts without requiring `trace.fozzy`/`events.json` (minimum required: `report.json` + valid `manifest.json`)
- ✅ `artifacts pack/export --out <dir>` now reject stale pre-existing unrelated files in output directories (prevents mixed old/new contamination)
- ✅ `artifacts pack/export --out <dir>` now enforce exact-output synchronization by pruning stale pre-existing entries (non-strict and strict modes consistent)
- ✅ `artifacts pack/export` now validate `manifest.json` schema+parse integrity and fail non-zero on corrupted manifest bytes
- ✅ File-output mode (`--out <zip>`) now enforces symlink-safe output path traversal checks (including parent path components)
- ✅ `corpus import` now rejects Windows-style unsafe archive paths (`..\\`, drive-prefixed, UNC-root) on all platforms
- ✅ `corpus import` now rejects unsafe/special archive filenames (control chars, NUL-containing names, Windows-reserved names, trailing-dot/space, cross-platform invalid chars)
- ✅ `corpus import` now rejects NUL-containing raw zip entry names before normalization, preventing ambiguous/truncated filename writes
- ✅ `corpus import` now rejects duplicate archive targets including alias/case-collision forms (for example `dup.bin`, `./dup.bin`, `DUP.BIN`) to prevent silent last-write-wins
- ✅ `corpus import` now preflights raw zip central-directory entries to reject duplicate names and NUL-collision aliases even when archive libraries collapse duplicate headers
- ✅ `corpus import` now refuses overwriting existing output files, preventing duplicate/fallback overwrite behavior during import
- ✅ `corpus export --out <zip>` now rejects symlinked output files and symlinked parent path components (strict and non-strict behavior consistent)
- ✅ `corpus export` now fails non-zero for missing/invalid or empty source corpus directories (no empty zip success artifacts)
- ✅ `corpus export` failure paths are atomic: unreadable source failures do not create output zips and do not clobber pre-existing output files
- ✅ `artifacts pack --out <zip>` is now byte-deterministic for the same run (stable metadata payload and fixed ZIP entry timestamps)
- ✅ CLI contract test matrix expanded across run-like commands, parse failures, and explicit exit-code contract (`0/1/2`)
- ✅ Filesystem chaos/security matrix expanded with host-fs sandbox/path-escape rejection and host-fs execution contract coverage (local test suite)
- ✅ Concurrent stress/repro gates added to local integration suite (`same .fozzy root` multi-run stability checks)
- ✅ `--strict` warning-to-error mode added for run/replay/shrink warning findings, `trace verify`, and `doctor`
- ✅ `trace verify --json --strict` now emits a single final JSON document (error-only on strict failure), preserving machine-parse contract
- ✅ `artifacts pack/export --help` now reflects runtime contract via `RUN_OR_TRACE` argument naming
- ✅ Trace ingest now enforces explicit header compatibility (`format=fozzy-trace`, schema `version` in supported range) for verify/replay/ci, independent of checksum presence
- ✅ Local parity/golden hardening tests added: run-like common flag parsing and end-to-end `record -> replay -> shrink -> replay(min)` for run/fuzz/explore
- ✅ End-to-end golden flows (`record -> replay -> shrink -> replay(min)`) per mode

### M11 Host Capability Execution
- ✅ Added explicit process backend mode (`proc_backend = scripted|host`) with CLI override (`--proc-backend`) and config default
- ✅ Implemented host `proc_spawn` execution path for `fozzy run` / `fozzy test` (non-deterministic mode)
- ✅ Added deterministic safety gate: `--det` + `--proc-backend host` is rejected with explicit error
- ✅ Improved contract/docs/diagnostics for scripted-vs-host proc behavior
- ✅ Added replay/trace semantics for host-proc runs: traces now capture proc result decisions and replay consumes them deterministically; verify warns on legacy host-proc traces without proc decisions
- ✅ Expanded host backend architecture beyond proc:
  - ✅ Host FS backend (`--fs-backend host`) with cwd-root sandboxing and explicit path-escape rejection
  - ✅ Host HTTP backend (`--http-backend host`) now supports both `http://` and `https://` endpoints with deterministic replay decisions
  - ✅ HTTP DSL expanded for production assertions: `http_request.headers` (request headers) and `http_request.expect_headers` / `http_when.headers` (response headers)
  - ✅ Determinism contracts enforced (`--det` rejects host fs/http/proc backends with explicit errors)
  - ✅ Replay contracts enforced for host execution (proc/http decision capture + legacy warning diagnostics)

### M12 Memory Mode (Deterministic Memory Correctness Engine)
- ✅ Memory capability contract finalized (`memory` as first-class runtime capability, deterministic-first, replay-first)
- ⬜ Schema strategy finalized (trace + report + manifest + memory artifacts versioning)
- ⬜ Deterministic memory execution contract implemented:
  - ✅ Allocation order determinism
  - ✅ Leak determinism
  - ✅ OOM determinism
  - ⬜ Shrink determinism

#### M12.1 Deterministic Tracking Foundation
- ✅ Runtime memory state integrated into core execution context (`ExecCtx`) without breaking existing capability patterns
- ✅ Deterministic allocation id generation + callsite hashing
- ✅ Allocation lifetime recording (alloc/free/in-use/peak)
- ✅ Seed-stable allocation ordering preserved under replay
- ✅ Memory counters surfaced in `report.json` and `manifest.json`
- ⬜ `memory.timeline.json` artifact emitted with stable ordering + schema tag

#### M12.2 Deterministic Leak Detection
- ✅ End-of-run leak accounting implemented and replay-stable
- ✅ Leak findings integrated with existing finding taxonomy and strict-mode semantics
- ✅ Leak budget policy implemented (`--leak-budget`)
- ✅ Leak hard-fail policy implemented (`--fail-on-leak`)
- ✅ `memory.leaks.json` artifact emitted and included in artifacts list/export/pack
- ✅ CI/report integration:
  - ✅ `fozzy ci` checks include deterministic leak policy when memory mode is enabled
  - ✅ `fozzy report` surfaces leak counts and budget status

#### M12.3 Deterministic Memory Pressure + OOM Injection
- ✅ Virtual memory ceiling support (`--mem-limit-mb`) implemented and replay-stable
- ✅ Allocation failure scripting (`--mem-fail-after`) implemented and replay-stable
- ✅ Runtime API hooks for memory pressure behavior implemented
- ✅ Replay drift detection includes memory-failure decision mismatches
- ✅ Deterministic/host-mode contracts documented and enforced

#### M12.4 Memory-Aware Shrinking
- ✅ Shrink objective extended to preserve leak/non-leak outcome class
- ⬜ Leak-minimal reproduction strategy implemented
- ✅ Memory delta comparison artifact added (`memory.delta.json`)
- ✅ Shrink output remains replayable and deterministic
- ✅ Existing shrink behavior for non-memory traces remains unchanged

#### M12.5 Memory Forensics Artifacts
- ✅ Allocation graph model implemented
- ✅ `memory.graph.json` artifact emitted with stable deterministic node/edge ordering
- ✅ Artifact diff/export/pack support includes memory graph + memory deltas
- ⬜ Artifact schema docs published for all memory artifact types

#### M12.6 Memory Pressure Fuzzing / Explore Integration
- ✅ Fuzz mode integrates memory pressure controls without replay drift
- ✅ Explore mode supports deterministic memory pressure fault scheduling
- ✅ Fragmentation/pressure-wave controls designed and shipped behind explicit flags
- ⬜ Coverage/checker model extended for memory-pressure outcomes

#### M12.7 CLI / SDK / Docs Parity
- ✅ CLI flags shipped and documented:
  - ✅ `--mem-track`
  - ✅ `--mem-limit-mb`
  - ✅ `--mem-fail-after`
  - ✅ `--fail-on-leak`
  - ✅ `--leak-budget`
  - ✅ `--mem-artifacts`
- ✅ `fozzy usage`, `CLI.md`, `README.md`, and scenario docs updated
- ✅ TS SDK parity shipped (`sdk-ts/` and `SDK-TS.md`) for all new memory controls
- ✅ `fozzy full` policy controls added for production CI roots:
  - ✅ `--allow-expected-failures`
  - ✅ `--scenario-filter`
  - ✅ `--skip-steps`
  - ✅ `--required-steps`
- ✅ Scenario authoring ergonomics expanded:
  - ✅ `fozzy schema` command alias: `fozzy steps`
  - ✅ `fozzy validate <scenario> --json` command for parse/shape diagnostics
- ✅ Run-selector alias parity across report/artifacts/memory with CI guidance to prefer explicit run ids/trace paths in race-sensitive automation

#### M12.8 Verification / Hardening Gate (Production)
- ⬜ Unit tests:
  - ⬜ allocator determinism
  - ⬜ leak accounting correctness
  - ⬜ OOM/fail-after determinism
  - ⬜ trace/report/artifact serde compatibility
- ⬜ Integration tests:
  - ⬜ golden flow coverage for memory run/test/fuzz/explore paths
  - ⬜ CLI parity tests for all memory flags and strict-mode behaviors
  - ⬜ artifacts list/diff/export/pack coverage for memory artifacts
- ✅ Determinism audit command gates added for memory scenarios
- ⬜ End-to-end required gate sequence for memory shipping:
  - ✅ `fozzy doctor --deep --scenario <memory_scenario> --runs 5 --seed <seed> --json`
  - ✅ `fozzy test --det --strict <memory_scenarios...> --json`
  - ✅ `fozzy run <memory_scenario> --det --record <trace.fozzy> --json`
  - ✅ `fozzy trace verify <trace.fozzy> --strict --json`
  - ✅ `fozzy replay <trace.fozzy> --json`
  - ✅ `fozzy ci <trace.fozzy> --json`
- ✅ Host-backed runtime checks executed where feasible for delivery confidence:
  - ✅ `fozzy run ... --proc-backend host --fs-backend host --http-backend host --json`

## Production Backlog (Next Execution Order)
1. ✅ Execute M12.1 deterministic memory tracking foundation.
2. ✅ Execute M12.2 deterministic leak detection + CI/report policy integration.
3. ✅ Execute M12.3 memory pressure limits + deterministic OOM injection.
4. ⬜ Execute M12.4 memory-aware shrinking (`memory.delta.json`) with replay-preservation checks.
5. ✅ Execute M12.5 memory forensic artifacts (`memory.graph.json`) + artifact tooling parity.
6. ✅ Execute M12.6 fuzz/explore memory-pressure integration.
7. ✅ Execute M12.7 CLI/SDK/docs parity.
8. ⬜ Execute M12.8 hardening gate and production release criteria.
9. ✅ Execute M13.2 profiler schema/artifact plumbing and command-surface scaffolding.
10. ✅ Execute M13.3 deterministic event-tracing profiler baseline (`fozzy profile top/timeline`).
11. ✅ Execute M13.4 CPU sampling v1 + flame/top/export (Linux-first).
12. ✅ Execute M13.5/M13.6 heap + latency critical-path analysis and `fozzy profile diff/explain`.
13. ✅ Execute M13.8 metric-preserving perf shrink.
14. ✅ Execute M13.11 hardening gate and profiler production release criteria.

## Definition of Done for 1.0
- Replay does not drift across supported platforms.
- Shrinking consistently yields minimal actionable reproductions.
- CLI contract is stable and documented.
- SDK-TS stable API ships as a thin wrapper.
- Distributed exploration is robust enough for real system regression suites.

## Definition of Done for Memory Mode v1
- Deterministic allocation tracking is replay-stable across supported platforms.
- Leak outcomes are reproducible and enforceable in CI (`--fail-on-leak` / `--leak-budget`).
- Memory artifacts are stable, schema-versioned, diffable, and included in artifact workflows.
- Shrinking preserves memory outcome class and produces smaller actionable leak repro traces.
- Trace/replay/verify/ci flows reject or warn on memory-schema drift using existing strict-mode policy.

### M13 Deterministic Profiler Command Surface (Performance Forensics Engine)
- ✅ Ship `fozzy profile` as a first-class command namespace, matching existing command architecture (`report`, `memory`, `artifacts`) and run selector behavior (`<run-id|trace>` + alias policy).
- ✅ Build profiler features on top of existing deterministic assets first (trace decisions, timeline events, memory artifacts, replay/shrink) before adding new sampling collectors.
- ✅ Keep profiler output regression-first and actionable-first (top regressions, causal chain, minimal repro trace path), not visualization-only.
- ✅ Preserve existing non-negotiables: replay correctness > feature count, strict mode defaults, schema-versioned artifacts, atomic writes, and CI-safe contracts.

#### M13.1 Command Namespace + CLI Contract (Parity-First)
- ✅ Add command family in CLI/docs/usage:
  - ✅ `fozzy profile top <run-id|trace> [--cpu|--heap|--latency|--io|--sched] [--limit <n>]`
  - ✅ `fozzy profile flame <run-id|trace> [--cpu|--heap] [--out <path>] [--format folded|svg|speedscope]`
  - ✅ `fozzy profile timeline <run-id|trace> [--out <path>] [--format json|html]`
  - ✅ `fozzy profile diff <left-run-id|trace> <right-run-id|trace> [--cpu] [--heap] [--latency] [--io] [--sched]`
  - ✅ `fozzy profile explain <run-id|trace> [--diff-with <run-id|trace>]`
  - ✅ `fozzy profile export <run-id|trace> --format speedscope|pprof|otlp --out <path>`
  - ✅ `fozzy profile shrink <trace.fozzy> --metric p99_latency|cpu_time|alloc_bytes --direction increase|decrease [--budget <dur>]`
- ✅ Enforce selector/alias parity with existing `report`/`artifacts`/`memory` commands.
- ✅ Add strict-mode diagnostics for missing profiler artifacts and unsupported profile modes (warning in relaxed mode, error in strict mode where contract requires artifact presence).
- ✅ Keep parse behavior + exit-code behavior consistent with existing `0/1/2` CLI contract matrix.

#### M13.2 Unified Profiler Data Model + Artifact Set (Schema-First)
- ✅ Extend run artifact manifest with profiler capability + artifact pointers:
  - ✅ `manifest.json` entries for profiler domains and schema versions.
- ⬜ Upgrade timeline schema to a canonical event model with stable ordering:
  - ⬜ Required fields: `t_virtual`, `t_mono` (optional), `kind`, `run_id`, `seed`, `thread`, `task`, `span_id`, `parent_span_id`, `tags`, `cost`.
  - ⬜ Event kinds: `span_start`, `span_end`, `event`, `sample`, `alloc`, `free`, `io`, `net`, `sched`.
- ⬜ Add profiler artifacts (versioned, deterministic ordering where applicable):
  - ⬜ `profile.cpu.json` (sampling + folded stacks + symbol refs)
  - ⬜ `profile.heap.json` (allocation hotspots/lifetimes/retention suspects)
  - ⬜ `profile.latency.json` (distributions + critical path + wait reasons)
  - ⬜ `profile.metrics.json` (aggregates used by `top`, `diff`, `explain`)
  - ⬜ `symbols.json` (or build-id reference map)
- ⬜ Integrate all profiler artifacts with `fozzy artifacts ls|diff|export|pack|bundle`.
- ⬜ Publish schema docs for all profiler artifacts with compatibility policy matching trace/manifest conventions.

#### M13.3 Mode A: Deterministic Event Tracing Profiler (Always-On Baseline)
- ✅ Reuse existing trace/timeline capture path as baseline profiler channel (lowest overhead mode).
- ⬜ Add engine-native performance events:
  - ⬜ Scheduler picks/waits/starvation windows
  - ⬜ Capability durations (`fs/http/proc/net`) and payload sizes where available
  - ⬜ Step/span duration boundaries
  - ⬜ Memory counters and allocation/free markers (from M12 path)
- ⬜ Implement `fozzy profile top --io --sched --heap` without CPU sampling dependency.
- ⬜ Ensure replay uses captured event stream deterministically with drift detection unchanged.

#### M13.4 Mode B: CPU Sampling Profiler v1 (Host-Time Domain)
- ⬜ Linux-first collector (`perf_event_open`) with permission/capability diagnostics and in-process fallback sampling.
- ⬜ macOS collector design + implementation path (Mach sampling and symbolization pipeline) with explicit parity checklist.
- ⬜ Capture per-thread stacks, sample count, sample period, and collector metadata.
- ⬜ Symbolization pipeline:
  - ⬜ build-id/module capture
  - ⬜ deferred symbolization for offline export
  - ⬜ stable folded-stack output for flamegraph tooling
- ⬜ Implement `fozzy profile flame` + `fozzy profile top --cpu`.
- ⬜ Document and enforce host-time semantics: CPU samples are not replay-deterministic but are comparable across repeated deterministic replays.

#### M13.5 Mode C: Heap/Allocation Profiler v1 (Memory Mode Leverage)
- ⬜ Build heap profile on top of existing memory tracking in M12 (no duplicate alloc pipeline).
- ⬜ Add callsite-centric analysis:
  - ⬜ in-use bytes by callsite
  - ⬜ alloc-rate by callsite
  - ⬜ lifetime histograms
  - ⬜ retention suspects and graph anchors
- ⬜ Implement `fozzy profile top --heap` and `fozzy profile diff --heap`.
- ⬜ Wire heap profiler findings into `report.json` finding taxonomy and strict budget semantics where configured.

#### M13.6 Mode D: Latency/Critical-Path Profiler v1 (Causal Diagnostics)
- ⬜ Compute per-span latency distributions (p50/p95/p99/max) and variance summaries.
- ⬜ Build dependency graph from span parentage + wait reasons + scheduler/IO edges.
- ⬜ Extract critical path and tail amplification suspects.
- ⬜ Implement `fozzy profile top --latency`, `fozzy profile diff --latency`, and `fozzy profile explain`.
- ⬜ Emit concise automated diagnosis in report surfaces:
  - ⬜ regression statement
  - ⬜ top shifted path
  - ⬜ likely cause domain (`io|sched|cpu|heap|payload`)
  - ⬜ evidence pointers (artifact + span/callsite ids)

#### M13.7 Determinism Contract: Dual Time Domains
- ⬜ Formalize two time domains in docs + schemas:
  - ⬜ `virtual_time` (deterministic, replay-critical)
  - ⬜ `host_monotonic_time` (performance measurement, non-deterministic)
- ⬜ Keep replay correctness bound only to deterministic decisions/events.
- ⬜ Add statistical comparison support for host-time metrics across repeated identical deterministic runs.
- ⬜ Expose confidence metadata for regression diffs when host-time data is used.

#### M13.8 Regression Diff + Shrink-on-Metric (Fozzy Differentiator)
- ⬜ Implement `fozzy profile diff` as first-class regression analyzer (not just raw metric delta dump).
- ⬜ Add metric-preserving shrink objective path:
  - ⬜ `fozzy profile shrink <trace> --metric ... --direction ...`
  - ⬜ Preserve target regression condition while minimizing input/schedule/fault surface.
- ⬜ Reuse existing shrink infrastructure (`input|schedule|faults|all`) and extend objective function for performance predicates.
- ⬜ Ensure shrunk perf traces remain replayable, verifiable, and CI-gateable.

#### M13.9 Integration Across Existing Surface (No Siloed Profiler)
- ⬜ `fozzy run/test/fuzz/explore`: add profiler capture flags with safe defaults and explicit overhead levels.
- ⬜ `fozzy replay`: support profiler-aware replay reports and optional export regeneration.
- ⬜ `fozzy ci`: add optional performance gate checks (for example max p99 delta budget).
- ⬜ `fozzy report`: include profiler diagnosis sections and queryable perf paths.
- ⬜ `fozzy artifacts`: full parity for profiler artifact listing/diff/export/pack/bundle.
- ⬜ `fozzy usage` + `fozzy full` + `fozzy gate`: include profiler-aware recommended flows and targeted execution where feasible.
- ⬜ SDK-TS parity for profiler command wrappers and typed JSON outputs.

#### M13.10 Production Gate Sequence (Profiler Shipping Contract)
- ⬜ Deterministic baseline gate (scenario-level):
  - ⬜ `fozzy doctor --deep --scenario <prof_scenario> --runs 5 --seed <seed> --json`
  - ⬜ `fozzy test --det --strict <prof_scenarios...> --json`
- ⬜ Trace+replay gate for profiler artifacts:
  - ⬜ `fozzy run <prof_scenario> --det --record <trace.fozzy> --json`
  - ⬜ `fozzy trace verify <trace.fozzy> --strict --json`
  - ⬜ `fozzy replay <trace.fozzy> --json`
  - ⬜ `fozzy ci <trace.fozzy> --json`
- ⬜ Host-backed runtime confidence checks for real-system perf behavior where feasible:
  - ⬜ `fozzy run ... --proc-backend host --fs-backend host --http-backend host --json`
- ⬜ Profiler command gate:
  - ⬜ `fozzy profile top <run|trace> --cpu --heap --latency --json`
  - ⬜ `fozzy profile diff <left> <right> --cpu --heap --latency --json`
  - ⬜ `fozzy profile explain <run|trace> --json`

#### M13.11 Test Matrix + Hardening Requirements
- ⬜ Unit tests:
  - ⬜ event schema encode/decode + compatibility
  - ⬜ folded-stack aggregation correctness
  - ⬜ latency critical-path extraction correctness
  - ⬜ heap callsite aggregation + lifetime histogram correctness
  - ⬜ diff heuristics stability and tie-breaking determinism
- ⬜ Integration tests:
  - ⬜ golden `run -> profile top/flame/timeline/export` flows
  - ⬜ golden `record -> replay -> profile diff/explain` parity flows
  - ⬜ `artifacts ls/diff/export/pack/bundle` profiler coverage
  - ⬜ strict/unsafe behavior for missing/legacy profiler artifacts
- ⬜ Performance/overhead tests:
  - ⬜ overhead budgets per profiling mode (`baseline`, `sampled`, `full`)
  - ⬜ bounded memory growth for long-running captures
  - ⬜ artifact size budgets + compression behavior

#### M13.12 Rollout Plan (Execution Order)
1. ✅ M13.2 schema + artifact plumbing and `artifacts`/`report` integration.
2. ✅ M13.3 deterministic event tracing profiler + `fozzy profile top` for IO/scheduler/heap-from-memory.
3. ✅ M13.4 CPU sampling profiler + flame/top/export pipeline (Linux first).
4. ✅ M13.5 heap profiler deep views + diff integration.
5. ✅ M13.6 latency critical path + `fozzy profile explain` narratives.
6. ✅ M13.8 perf regression shrink objective.
7. ✅ M13.11 hardening + production gate promotion.

## Definition of Done for Profiler Mode v1
- `fozzy profile` command family is CLI/SDK/docs parity-complete and follows existing run selector + strict mode contracts.
- Profiler artifacts are schema-versioned, deterministic where required, and fully integrated with `artifacts` + `report` + `ci` workflows.
- Deterministic baseline profiling (`io/sched/heap/latency`) is replay-stable and trace-verify compatible.
- CPU sampling flow is production-usable with clear host-time semantics, symbolization path, and diff-ready outputs.
- `fozzy profile diff` and `fozzy profile explain` provide actionable regression diagnosis with evidence pointers.
- `fozzy profile shrink` can minimize a regression while preserving chosen performance predicate.
