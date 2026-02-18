# Fozzy

Deterministic full-stack testing: test, replay, shrink, fuzz, and distributed exploration via one engine and one CLI.

Status: pre-1.0. This repository is being implemented from [PLAN.md](PLAN.md).

## Install (dev)

```bash
cargo install --path .
fozzy version --json
```

## Quickstart

Initialize a project and run the example scenario:

```bash
fozzy init --force
fozzy run tests/example.fozzy.json --det --json
```

Record a trace on failure, then replay and shrink it:

```bash
fozzy run tests/fail.fozzy.json --det --json
fozzy replay .fozzy/runs/<runId>/trace.fozzy --json
fozzy shrink .fozzy/runs/<runId>/trace.fozzy --minimize all --json
```

## CLI

The canonical CLI surface is documented in [CLI.md](CLI.md).

## Repo Docs

- [PLAN.md](PLAN.md): execution plan / milestones
- [RUST-STYLE-GUIDE.md](RUST-STYLE-GUIDE.md): Rust conventions for this codebase
- [SDK-TS.md](SDK-TS.md): TypeScript SDK contract (thin wrapper over the binary)
- [sdk-ts/](sdk-ts/): production TypeScript SDK package scaffold (`fozzy-sdk`)

## License

MIT (see `LICENSE`).
