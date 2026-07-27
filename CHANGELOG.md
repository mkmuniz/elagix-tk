# Changelog

All notable changes to this project are recorded here. Format loosely inspired by [Keep a Changelog](https://keepachangelog.com/).

## [0.1.0] — 2026-07-26

First release. Milestones M0-M8 complete (full history in [`MILESTONES.md`](MILESTONES.md)).

### Added
- `bornes/comandos`: `$PATH` shim, Layer A (dedicated parsers for `git status`/`log`/`diff`/`show`, `pytest`, `cargo test`), and Layer B (declarative TOML rule engine, with example filters for `docker images`, `git branch`, `terraform plan`, `npm install`).
- `bornes/mcp`: JSON-RPC proxy over stdio with tool schema lazy-loading and call-result compression.
- `bornes/prosa`: TF-IDF extractive summarization, integrated into commit message bodies (`git log`/`git show`) and available as a standalone utility (`elagix compress`).
- `core/store`: content-addressed store — `git show <sha>` cache, progressive disclosure (`elagix show <hash>`), time-window deduplication, automatic expiration-based cleanup.
- `install.sh` (Linux/macOS/WSL) and `install.ps1` (Windows) installers.
- Cross-compilation validated for Windows (`x86_64-pc-windows-gnu`).

### Known to be incomplete
See [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) — notably: Layer B command coverage is still small, no macOS build, `install.ps1` has no fully validated activation, `bornes/mcp` has never been wired up to a real server.
