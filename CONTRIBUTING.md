# Contributing to Schliffe

Thanks for your interest. This project is young (v0.1.0, active development) — the process below is deliberately simple.

## Before opening a PR

1. Read [`specs.md`](specs.md) — the technical specification and its business rules (§4) aren't negotiable by design (motivated by real, documented risks, not aesthetic preference). Any change needs to keep respecting the 7 rules, especially rule 6 ("never inflate the output") and rule 3 ("fail-open").
2. Check [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) and [`TASKS.md`](TASKS.md) — there's probably already a task describing what you want to do, with context on why it hasn't been done yet.

## Development environment

Requires Rust ([rustup.rs](https://rustup.rs)). No system dependencies beyond that — every crate used is pure Rust (no C/FFI), which is why cross-compiling to Windows only needs `mingw-w64` (see `.github/workflows/ci.yml`).

```bash
cargo build --release
cargo test
```

## Adding a new filter (Layer B)

The easiest way to contribute: write a new TOML rule for a command that doesn't have a filter yet (list in `KNOWN_ISSUES.md`). No need to recompile anything beyond running the tests:

1. Create `src/bornes/comandos/filters-toml/<command>.toml` following the format of the existing examples (`match_command`, `match_args_prefix`, `pipeline`).
2. Register the new file in `src/bornes/comandos/camada_b/mod.rs` (`EMBEDDED`).
3. Write a test with a real fixture (captured by running the actual command) — no invented fixtures, `specs.md` §5.4 explains why.

## Adding a dedicated parser (Layer A)

Reserve this for high-volume commands where Layer B isn't enough (specs §5.2 explains the criteria). Follow the pattern in `src/bornes/comandos/filters/*.rs`: fail-open on an unexpected format, never fabricate success, always tested against a real fixture.

## Tests

Every filter/rule needs a test with real data (captured by running the actual command), not synthetic data — several real bugs in this project (documented in `MILESTONES.md`) only showed up when testing against real output, not an invented fixture.

```bash
cargo test
```

## Reporting issues

Open an issue. If it's a known gap, it's probably already in [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) — comment there instead of duplicating it.
