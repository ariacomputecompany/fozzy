# New Fixes

This document is now the execution brief for five parallel engineers. The goal is to finish the whole hardening pass in one coordinated sweep, with no backwards-compatibility compromises and no “leave it for later” seams. Each engineer owns one lane completely, should optimize for the architecturally correct end state rather than incremental patching, and must avoid editing files assigned to another lane unless the plan is explicitly revised first. If a change appears to cross a boundary, the owning engineer should move the abstraction instead of reaching across it. Every item from the original fix list is assigned exactly once below so we can get full coverage in one pass.

## Engineer 1: Artifact Trust, Bundle Integrity, and Selector Semantics

You own the artifact truth model. Your job is to make every artifact-facing surface trust exactly one validated bundle abstraction, with no side entrances, no wrapper-vs-trace ambiguity, no duplicate selector logic, and no “parseable means trusted” shortcuts. You should treat this lane as the repository’s trust-boundary rewrite: artifact resolution, manifest validation, trace loading, sidecar trust, selector aliasing, integrity caching, and artifact-diff efficiency all belong here. The rest of the system should consume typed validated results from you, not rebuild trust logic ad hoc. Do not let command-specific behavior survive if it creates divergent truth semantics between `artifacts`, `memory`, `profile`, `report`, and `ci`. You are working close to Engineer 2 on finalization-produced identity and close to Engineer 3 on CLI wording, so your interfaces must be explicit and typed, but you should not edit runtime execution or help-text surfaces yourself. Your completion bar is that a caller can only get either an observational direct-trace view or a validated run-bundle view, and that distinction is shared, intentional, cached where appropriate, and impossible to bypass accidentally.

- Stop wrapper-backed artifact consumers from bypassing trace checksum and header validation.
- Make manifest validation match the manifest schema that is actually emitted.
- Finish the artifact resolver split before shipping another hardening pass.
- Centralize artifact and trust resolution behind one validated bundle abstraction.
- Add a parsed-artifact cache for report/manifest/trace/sidecar loads.
- Stop hashing full traces on every profile load when only staleness is being checked.
- Unify alias resolution semantics across artifacts, memory, profile, and report consumers.
- Keep the current artifact-integrity strictness, but move it behind cheaper, typed interfaces.
- Stop read-oriented profile and report commands from mutating artifacts implicitly, or at minimum make those refreshes locked and atomic.
- Fix the current profile sidecar identity regression before trusting the strict workflow gate again.
- Make direct-trace mode and validated-wrapper mode explicit product surfaces instead of letting each command improvise the distinction.
- Stop accepting arbitrary existing file paths as implicit run selectors through parent-directory fallback.
- Add a real run index for alias resolution before `.fozzy/runs` size turns common commands into full directory scans.
- Replace directory-mtime alias selection with immutable run identity or completion metadata.
- Stop `artifacts diff` from byte-scanning every same-size file pair in the common case.
- Remove the redundant manifest-backed summary reload from artifact integrity validation.
- Stop `fozzy ci` from paying the full artifact export and unzip tax on every invocation when the caller only needs integrity/replay semantics.
- Centralize structural validation of `memory.graph.json` and make every real consumer enforce it, not just the workflow-status layer.
- Stop defaulting profile views to a CPU domain the product does not yet support.

## Engineer 2: Runtime Execution Core, Finalization, and Producer Truth

You own runtime behavior, execution-state architecture, and final persisted run truth. Your job is to make run, replay, fuzz, explore, shrink, test aggregation, manifest/report/trace finalization, and doctor all share one coherent execution model and one authoritative producer pipeline. Favor deeper structure changes over local band-aids: if runtime finalization depends on command-layer producers, move ownership; if run and replay duplicate loops, unify them; if timing or findings are computed in multiple places, make one source canonical and force every artifact to derive from it. You should assume that artifact consumers downstream will become stricter after Engineer 1’s changes, so producer truth needs to be complete, stable, and final before wrappers are written. You are working adjacent to Engineer 3 on surfaced command contracts and adjacent to Engineer 5 on file decomposition, so keep your abstractions clean enough to be moved into smaller modules without changing behavior. Your completion bar is that execution outputs become single-source-of-truth artifacts with deterministic suite behavior, exact shrink semantics, and no late mutations that can create inconsistent persisted state.

- Either implement memory support for `fozzy explore` or remove the CLI contract that claims it exists.
- Remove redundant manifest writes from run and replay flows.
- Make `trace verify` report checksum validity explicitly instead of inferring it from successful parse.
- Deduplicate helper logic that already exists in multiple runtime and command modules.
- Refactor the engine so run and replay share a single execution core.
- Shrink the responsibility footprint of `ExecCtx`.
- Revisit `exec_expect_failure` so nested failure assertions do not silently discard useful evidence.
- Tighten test-run seed strategy for multi-scenario suites.
- Rework aggregate test trace identity so per-test traces are tied cleanly back to the suite run.
- Make parallel `fozzy test --jobs` aggregation deterministic before merging findings and naming recorded traces.
- Expand `doctor` to cover more of the real production surface.
- Make report and manifest writes the final commit point of a run, not an early intermediate state.
- Make wrapper summaries and direct-trace summaries agree on final timing fields.
- Preserve exact failure-class semantics during shrink instead of collapsing everything into “not pass.”
- Move profile artifact production out of command-layer ownership so runtime finalization does not depend on CLI command modules.
- Align library-default memory behavior with CLI and config-default behavior.
- Make trace summary findings match the final persisted report findings, then validate that coherence explicitly.
- Make reporter artifacts consume the same final identity state as report, manifest, and trace outputs in fuzz and explore flows.
- Stop deriving manifest profile capabilities from opportunistic filesystem scans after the fact.

## Engineer 3: CLI Surface, Workflow Contracts, Reporting Semantics, and Public API

You own what the product claims to users and to library consumers. Your job is to make the public API, the Clap model, workflow gates, reporting commands, selector help, schema/help generation, and CLI-level machine-readable behavior line up exactly with what the runtime can actually do. Remove parseable-but-rejected states, misleading names, hidden profile work in cheap report paths, and silent error suppression. If the CLI says a mode exists, it must work; if a flag is named like jq, it must be jq or be renamed; if `fozzy gate` and `fozzy full` are treated as production proof, they must actually validate the real command surface rather than only library entrypoints. You are working close to Engineer 1 on selector/trust semantics and close to Engineer 2 on runtime-owned producer contracts, so consume their stable interfaces rather than recreating them. You are also working close to Engineer 5 on splitting `cli_workflows` into smaller modules, so organize behavior around crisp interfaces that can be moved without semantic drift. Your completion bar is that the CLI and public library surface tell the truth, reject impossible states up front, expose failures explicitly, and no longer encode shadow schemas or normalization rules that diverge from the real product.

- Narrow the public API surface exported from the crate root.
- Remove command-surface drift where Clap accepts states the runtime later rejects.
- Rename or narrow `report query --jq` so the contract matches the implementation.
- Reduce manual coupling between runtime DSL, schema docs, and usage docs.
- Replace manual global-argument normalization with one command model that Clap can own directly.
- Either make `--reporter json` a real execution-mode contract or remove it from run/test/fuzz/explore surfaces.
- Always emit structured CI results even when the check fails.
- Stop `report show` and `report query` from paying the full profile-explain path on every invocation when no profile diagnosis will be surfaced.
- Stop `report` from silently suppressing profile-diagnosis failures.
- Make profile selector help text match the resolution behavior the command actually implements.
- Replace `map suites` scenario classification heuristics with parsed scenario metadata before the topology surface becomes policy-bearing.
- Add a scan cache or incremental mode for `fozzy map suites` before repository size makes the command feel heavy.
- Make `fozzy gate` and `fozzy full` prove the real CLI contract instead of only proving the library contract.
- Stop the strict workflow layer from treating valid “no profile data” states as hard failures.
- Stop re-encoding producer-owned profile contracts inside the workflow gate layer.
- Clean up `fozzy gate` temporary trace artifacts the same way `fozzy full` already does.
- Make `fozzy full` prove that `host_backends_run` actually exercised host-backed behavior instead of only rerunning a scenario under host backends.

## Engineer 4: End-to-End Coverage, SDK Parity, and Repository Hygiene

You own proof, parity, and cleanup. Your job is to make sure the hardening pass is verified from the outside, not just “implemented.” Build or update the regression matrix that proves selectors, aliases, wrappers, direct traces, stale sidecars, report timing fields, and SDK types all match the real binary contract. Treat the TypeScript SDK as public surface that must be derived from or tested against the Rust CLI instead of hand-waved into parity. Also clean the repository itself so tracked noise stops obscuring source truth. You are not responsible for moving production code into smaller files; that belongs to Engineer 5. You are working near Engineers 1 through 3 because your tests should lock their contracts in place, but you should not absorb their implementation responsibilities. Your completion bar is that this pass leaves behind explicit end-to-end proofs, SDK parity coverage, a build-green discipline check, and a cleaner tree with fewer sources of accidental confusion.

- Add end-to-end regression matrices for selector parity and stale-sidecar recovery.
- Restore build-green discipline before relying on the rest of the confidence stack.
- Bring the TypeScript SDK back into strict parity with the Rust CLI before treating it like a trustworthy public surface.
- Add explicit parity tests for full report/trace timing coherence, not just identity-path coherence.
- Clean accidental workspace artifacts out of the tracked tree and keep them out.
- Remove stray non-source workspace artifacts in addition to Finder metadata.

## Engineer 5: File Splitting, Module Decomposition, and Production-Grade Layout

You own the structural cleanup pass. Your job is to break oversized files and mixed-responsibility modules into crisp, single-purpose units with stable names, explicit ownership, and minimal cross-talk. This is not cosmetic formatting; it is architectural decomposition. Treat every big file as a symptom that responsibilities are colliding. Split workflow orchestration from workflow policy and workflow contracts. Split giant test suites by feature area so regressions land where they belong. Move large in-file test modules out of production files so contributors can reason about control flow without paging through fixtures. Because you are intentionally working near logic that Engineers 2 and 3 also touch, you must not redesign behavior yourself beyond what is necessary to perform the split cleanly; instead, preserve semantics while extracting structure, and make the resulting module boundaries match the ownership lines in this document. Your completion bar is that the repository no longer relies on giant “everything” files, production modules are named for one concern each, and future hardening work can land in the right file without reopening this cleanup.

- Split the workflow mega-module into smaller, typed components.
- Break up the very large test files by feature area without losing breadth.
- Stop mixing large production workflow code with a large embedded test suite in the same file.
- Move the large in-file test modules out of already-large production files.

## Full-Coverage Checklist

Before implementation begins, the five engineers should sanity-check these lane boundaries together once and then work independently:

1. Engineer 1 owns artifact trust, selectors, sidecars, aliasing, integrity validation, and artifact-read performance.
2. Engineer 2 owns runtime execution, finalization order, timing truth, suite determinism, shrink semantics, and producer-owned artifact generation.
3. Engineer 3 owns public API shape, Clap/state validity, report/query semantics, workflow contract truth, and CLI-facing output behavior.
4. Engineer 4 owns external proof: regression matrices, SDK parity, compile/build discipline checks, and repository hygiene.
5. Engineer 5 owns file splitting and module layout only, preserving behavior while making responsibilities explicit.

If a task seems to belong to two lanes, do not overlap implementation. Move the abstraction boundary so one engineer owns the new seam and the other consumes it. The goal is not just to finish the list; it is to leave the repository in a state where future hardening passes do not recreate the same ambiguity.
