# Changelog

All notable changes to this project are recorded here. Format loosely inspired by [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Changed
- **Renamed from Elagix to Schliffe** (German: *the cuts* — the facets of a cut gem). Command `schliffe`, folder `~/.schliffe`, environment variables `SCHLIFFE_*`. `install.sh` migrates an existing Elagix install: moves the stats history, store and custom filters, removes the old shims and PATH lines (with backups) and re-registers the Claude Code hook. The binary still answers to `elagix` for sessions started before the switch.

### Added
- Claude Code `PostToolUse` hook (`schliffe hook install|uninstall`, registered automatically by `install.sh`): compresses results from remote MCP servers (HTTP/OAuth, e.g. Figma) and shrinks images (MCP screenshots, Read on PNG/JPEG) to a 1280px long edge. macOS, Linux and WSL only — not native Windows.

### Fixed
- `install.sh` deleted existing shims for tools it couldn't see from a trimmed environment (e.g. an agent shell without nvm); it now also checks the user's full shell PATH and never removes a shim.

## [0.2.0] — 2026-09-25

First MVP meant for day-to-day use: validated on macOS (Apple Silicon, zsh) with Claude Code in VS Code.

### Added
- `schliffe stats`: savings report (24h / 7 days / all time, top savers, commands that ran with no filter). Size-only log, never arguments or output.
- `schliffe --version`.
- Layer B: filters can target stderr (`stream = "stderr" | "both"`), `match_any` for rules reached through several invocations, `|` alternatives in `match_args_prefix`, and new actions `compact_path`, `collapse_lines_matching`, `squeeze_spaces`.
- New filters, validated against real output: `npm`/`pnpm`/`yarn`/`pip` install, JS builds via the package manager (Next.js, Vite), linters via the package manager (ESLint), `git pull`/`merge`, `docker pull`/`build`/`compose build|pull`, `dotnet`, cargo's stderr progress, `go`.
- MCP proxy: `--keep-schemas` (recommended for Claude Code), recovery hint for trimmed results, JSON-RPC error when the server dies mid-call.
- End-to-end test suite against the compiled binary (`tests/e2e.rs`), including determinism.
- CI job for macOS; release workflow with prebuilt binaries.

### Changed
- Filtering only happens for AI agents (`CLAUDECODE` / `AI_AGENT` / `SCHLIFFE_FORCE`); humans, editors, git hooks and scripts get untouched output. `SCHLIFFE_DISABLE=1` opts out.
- Commands with no filter stream live instead of being captured (dev servers work normally).
- `git log`: one line per commit instead of only the first commit.
- Dedup is scoped to the agent's real session id (`CLAUDE_CODE_SESSION_ID`).
- Sentence splitter handles abbreviations, initials and bullet lists.
- `install.sh`: zsh PATH line at the end of `.zshrc`, shims only for installed tools, binary copied to `~/.schliffe/bin`, `schliffe` itself on PATH.

### Fixed
- The shim could resolve itself as the real binary and recurse until fork failed.
- MCP: JSON *file contents* read through a file tool were being compacted (corruption risk) — file-reading tools are never touched now.
- MCP: pending requests hung forever when the server died.
- Windows: `npm`/`pnpm`/`yarn` (`.cmd`) were never found by their shims.
- Missing real binary now exits 127 like "command not found".

## [0.1.0] — 2026-07-26

First release. Milestones M0-M8 complete (full history in [`MILESTONES.md`](MILESTONES.md)).

### Added
- `bornes/comandos`: `$PATH` shim, Layer A (dedicated parsers for `git status`/`log`/`diff`/`show`, `pytest`, `cargo test`), and Layer B (declarative TOML rule engine, with example filters for `docker images`, `git branch`, `terraform plan`, `npm install`).
- `bornes/mcp`: JSON-RPC proxy over stdio with tool schema lazy-loading and call-result compression.
- `bornes/prosa`: TF-IDF extractive summarization, integrated into commit message bodies (`git log`/`git show`) and available as a standalone utility (`schliffe compress`).
- `core/store`: content-addressed store — `git show <sha>` cache, progressive disclosure (`schliffe show <hash>`), time-window deduplication, automatic expiration-based cleanup.
- `install.sh` (Linux/macOS/WSL) and `install.ps1` (Windows) installers.
- Cross-compilation validated for Windows (`x86_64-pc-windows-gnu`).

### Known to be incomplete
See [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) — notably: Layer B command coverage is still small, no macOS build, `install.ps1` has no fully validated activation, `bornes/mcp` has never been wired up to a real server.
