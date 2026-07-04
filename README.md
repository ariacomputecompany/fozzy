# Fozzy

Fozzy is a deterministic testing, fuzzing, replay, and exploration runtime.
It provides one native execution surface for scenario runs, suite runs, distributed fault exploration, trace replay, shrinking, artifact inspection, memory diagnostics, profile analysis, and local CI-style gate checks.

## Why use Fozzy

Fozzy is designed to catch and debug high-cost failures that traditional test runners miss:

- Nondeterministic failures: order/race behavior that is hard to reproduce.
- Distributed consistency bugs: partition/heal/crash/restart edge cases.
- Timeout and hang regressions: deterministic virtual-time validation.
- Flakiness drift: run-set variance and flake-budget policy gates.
- Input robustness bugs: malformed inputs and mutation-discovered crashes.
- Replay drift: when a recorded failure no longer reproduces exactly.
- Artifact integrity problems: corrupted traces, invalid checksums, broken exports.

Result: every failure can be recorded, replayed, minimized, and shared as a reproducible artifact.

## Current Guarantees

- Deterministic execution in `--det` mode with seeded RNG, virtual time, and recorded decision logs.
- Recorded traces use the `.fozzy` format with versioned structure and checksum validation.
- `fozzy trace verify` reports checksum presence and checksum validity explicitly.
- `fozzy replay` reconstructs recorded executions from trace decisions instead of rerunning live effects.
- Strict mode is on by default and promotes warning-class integrity problems into hard failures unless `--unsafe` is used explicitly.
- Artifact writes are atomic and trace recording supports explicit collision policies.
- Host proc, fs, and http backends can be used in deterministic mode; live observations are captured into the trace so replay remains deterministic.
- Memory tracking is opt-in and supports leak budgets, memory graphs, memory diffs, and replay-safe memory summaries.
- Run selectors support explicit run ids and trace paths, plus documented aliases like `latest`, `last-pass`, and `last-fail`.
- Machine-readable JSON is available across the operational verification surface, including `test`, `run`, `replay`, `trace verify`, `doctor`, `ci`, `env`, `version`, `schema`, and `validate`.

## End-to-End Flow

The core Fozzy workflow is:

1. execute a scenario or suite
2. optionally record a trace
3. verify the trace
4. replay the trace
5. run CI-style integrity and parity checks against that trace

That gives one concrete artifact chain for debugging and handoff instead of a best-effort rerun model.

## Runtime Backends

Fozzy uses deterministic-first capability backends, with host execution available explicitly when needed.

- Process:
`scripted` (`proc_when` + `proc_spawn`) by default, optional host mode via `--proc-backend host`.
- Filesystem:
`virtual` overlay by default, optional host mode via `--fs-backend host` (cwd-root sandboxed).
- HTTP:
`scripted` (`http_when` + `http_request`) by default, optional host mode via `--http-backend host`.
`http_request` supports request headers and response-header assertions (`expect_headers`) in both scripted and host modes.
`http_when` can also be used with host backend as a response assertion rule (match by absolute URL or request path like `/v1/me`).
Host HTTP backend supports both `http://` and `https://` endpoints.

Host backends are allowed with `--det`.
In `--det` mode, RNG, scheduling, and virtual time remain deterministic while live host proc/fs/http observations are captured into the trace as replay decisions.
That means `fozzy replay` remains deterministic, while repeated live `--det` runs against a changing host environment can still observe different host-side results.
`fozzy env --json` continues to report host capability backends as non-deterministic substrates.

Inspect active runtime capabilities with:

```bash
fozzy env --json
```

## CLI Surface

- `fozzy test`: execute scenario suites.
- `fozzy run`: execute a single scenario.
- `fozzy fuzz`: mutation/property fuzzing.
- `fozzy explore`: deterministic distributed schedule/fault exploration.
- `fozzy replay`: reproduce a recorded trace.
- `fozzy shrink`: minimize failing traces.
- `fozzy ci`: local gate bundle (verify + replay + artifact integrity + optional flake budget).
- `fozzy gate`: lightweight strict targeted gate for change-scoped validation.
- `fozzy report`: render/query reports.
- `fozzy profile`: deterministic performance forensics (`top/flame/timeline/diff/explain/export/shrink`).
- `fozzy memory`: inspect memory graphs, leak tops, and memory diffs.
- `fozzy artifacts`: list/export/pack/bundle run artifacts.
- `fozzy map`: language-agnostic code topology mapping for hotspot-driven suite planning.
  `fozzy map suites` defaults to a `pedantic` coverage profile.
  For closure work, use `fozzy map suites --only-uncovered --only-required --all --json` to enumerate the remaining required gaps directly.

Full command contract: [CLI.md](CLI.md)

## Quickstart

```bash
fozzy init --force
fozzy run tests/example.fozzy.json --det --json
```

Recorded trace lifecycle:

```bash
fozzy run tests/example.fozzy.json --det --record /tmp/run.fozzy --json
fozzy trace verify /tmp/run.fozzy --strict --json
fozzy replay /tmp/run.fozzy --json
fozzy ci /tmp/run.fozzy --json
```

Representative strict suite flow:

```bash
fozzy doctor --deep --scenario tests/example.fozzy.json --runs 5 --seed 12345 --json
fozzy test --det --strict tests/example.fozzy.json tests/memory.pass.fozzy.json --json
```

Host-backed deterministic run:

```bash
fozzy run tests/host.pass.fozzy.json --det \
  --proc-backend host --fs-backend host --http-backend host --json
```

## Install (dev)

```bash
cargo install --path .
fozzy version --json
```

## Repository Docs

- [CLI.md](CLI.md): complete command contract

## License

MIT (see `LICENSE`).
