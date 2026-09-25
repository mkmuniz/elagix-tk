# Elagix — Task backlog

Every item here comes from a gap already recorded in `KNOWN_ISSUES.md` (which explains the *why* behind each one) — this file is just the actionable version, to pull from when developing. No implied priority/order yet.

## Platform / installation

- [ ] Validate `install.ps1` with full activation on a Windows machine with a genuine native toolchain (`git`/`cargo`/`npm` on the Windows PATH, not just WSL).
- [x] Build and test Elagix on real macOS — native build + 41 tests on Apple Silicon (2026-09-24); `macos-latest` job added to CI.
- [x] Test `install.sh`'s `zsh` path live — validated on macOS (2026-09-24) with `zsh -lic`/`-lc`/`-ic`; the `.zshrc` line now goes at the END (after nvm), not the top.
- [x] Release pipeline — `.github/workflows/release.yml` builds Linux x86_64, macOS arm64/x86_64 and Windows x86_64 binaries on a `v*` tag and attaches them (+ SHA256SUMS) to a GitHub Release. Not run yet (needs a tag push).
- [ ] Make `install.sh`/`install.ps1` download the prebuilt binary from the latest release instead of building from source (once a release exists).
- [ ] Repeat the "how does Claude Code actually invoke a shell" check (login/interactive, etc.) on any new platform, before declaring activation ready there — don't assume it generalizes from WSL/Linux.
- [x] Add the `elagix` binary itself to the PATH — `install.sh` now links `~/.elagix/shims/elagix`.

## Command coverage (Layer B / `bornes/comandos`)

- [x] First batch of long-tail filters (2026-09-24): npm/pnpm/yarn/pip install, docker pull/build, dotnet, cargo stderr, go — plus stderr support in Layer B (`stream = "stderr"|"both"`).
- [ ] Write TOML filters for the remaining long-tail commands (RTK audit, specs §10): `go-build`, `tsc`, `rg`, `make`, `jq`, `poetry`, `uv`, `mise`, `jj`, `nx`, `turbo`, `pre-commit`, `grep`, `fd`, `tree`, `wc`, `df`, `stat`, `shellcheck`, `yamllint`, `oxlint`, `ruff-format`, `cargo-clippy`, `ls-la`, `golangci-lint`, among others.
- [x] ~~Make Layer B's pipeline order configurable~~ — it already is: steps run in the order the TOML lists them (the "fixed order" note was wrong).
- [x] Catalog actions `compact_path`, `collapse_lines_matching` (aggregate-lite, with recovery hint) and `squeeze_spaces`; `match_any` for rules reached through several invocations (2026-09-24).
- [ ] Remaining catalog actions: `group_by`, `json_extract`/`json_schema`/`ndjson_stream`, `regex_extract`, `state_machine`, `format_template`.
- [ ] Build a Layer A parser for file reading (`read`/`smart`) — unblocks the "summarize a long docstring/comment" use case via `bornes/prosa`.

## Cache, progressive disclosure, and dedup (`core/store`)

- [ ] Extend the cache to working-tree-dependent commands (`git status`/`git diff` with no fixed commit — needs to check `.git/index` mtime).
- [ ] Evaluate a file-read cache (key: path + mtime + size, or a content hash).
- [x] Scope dedup to a real session id — uses `CLAUDE_CODE_SESSION_ID` (or `ELAGIX_SESSION_ID`), time window kept as the upper bound (2026-09-24).
- [ ] Validate the cleanup policy (14 days, ~2% sweep per write) at real usage volume, not just the volume generated during development.

## `bornes/mcp`

- [x] Test against a real production MCP server — `@modelcontextprotocol/server-filesystem` (2026-09-24): `tools/list` −77%, `get_tool_schema` round trip OK. Found and fixed: JSON *file contents* were being compressed (corruption risk) — file-reading tools are now never touched.
- [x] Document wrapping an MCP server (README, "Using the MCP proxy"). Automatic rewriting of client configs left out on purpose — editing `~/.claude.json` behind the user's back is riskier than one `claude mcp add` line.
- [ ] Support OAuth and remote HTTP streaming (stdio only today).
- [ ] Field pruning by semantic relevance (pagination, HATEOAS links, redundant timestamps) — today only the 3 mechanical techniques (null-strip, truncation, array cap).
- [ ] Handle/compress requests initiated by the MCP server itself (e.g. `sampling/createMessage`) — pass straight through today.
- [x] Server dying mid-call — pending requests now get a JSON-RPC error (validated live, 0.02s) instead of hanging forever.

## `bornes/prosa`

- [x] Sentence splitter handles abbreviations (EN+PT), initials, lowercase continuations, and treats bullets/blank lines as boundaries (2026-09-24).
- [ ] Evaluate an importance heuristic beyond pure TF-IDF (or formally accept the current limitation — it picks by word rarity, not intuitive "importance").
- [ ] Consider a real tokenizer instead of the `bytes/4` estimate.

## Quality / process

- [x] `elagix stats` — savings report (24h/7d/all time, top savers, unfiltered commands), fed by a size-only log from the shim and the MCP proxy (2026-09-25).

- [x] End-to-end tests against the compiled binary — `tests/e2e.rs` (12 tests: agent gating, exit codes, 127, self-recursion, `elagix show`, session dedup, stderr filters, rule 6, `compress`).
- [x] Determinism test — `tests/e2e.rs::output_is_deterministic` (same input, fresh store, byte-identical output).
- [x] Set up CI (cross-platform build + test) — Linux, Windows cross-compile, macOS.
