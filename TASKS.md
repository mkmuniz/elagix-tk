# Elagix — Task backlog

Every item here comes from a gap already recorded in `KNOWN_ISSUES.md` (which explains the *why* behind each one) — this file is just the actionable version, to pull from when developing. No implied priority/order yet.

## Platform / installation

- [ ] Validate `install.ps1` with full activation on a Windows machine with a genuine native toolchain (`git`/`cargo`/`npm` on the Windows PATH, not just WSL).
- [x] Build and test Elagix on real macOS — native build + 41 tests on Apple Silicon (2026-09-24); `macos-latest` job added to CI.
- [x] Test `install.sh`'s `zsh` path live — validated on macOS (2026-09-24) with `zsh -lic`/`-lc`/`-ic`; the `.zshrc` line now goes at the END (after nvm), not the top.
- [ ] Set up a release pipeline (CI + a published binary) to stop depending on building from source on every install.
- [ ] Repeat the "how does Claude Code actually invoke a shell" check (login/interactive, etc.) on any new platform, before declaring activation ready there — don't assume it generalizes from WSL/Linux.
- [x] Add the `elagix` binary itself to the PATH — `install.sh` now links `~/.elagix/shims/elagix`.

## Command coverage (Layer B / `bornes/comandos`)

- [x] First batch of long-tail filters (2026-09-24): npm/pnpm/yarn/pip install, docker pull/build, dotnet, cargo stderr, go — plus stderr support in Layer B (`stream = "stderr"|"both"`).
- [ ] Write TOML filters for the remaining long-tail commands (RTK audit, specs §10): `go-build`, `tsc`, `rg`, `make`, `jq`, `poetry`, `uv`, `mise`, `jj`, `nx`, `turbo`, `pre-commit`, `grep`, `fd`, `tree`, `wc`, `df`, `stat`, `shellcheck`, `yamllint`, `oxlint`, `ruff-format`, `cargo-clippy`, `ls-la`, `golangci-lint`, among others.
- [x] ~~Make Layer B's pipeline order configurable~~ — it already is: steps run in the order the TOML lists them (the "fixed order" note was wrong).
- [ ] Implement the catalog actions (specs §5.3) still missing: `group_by`, `json_extract`/`json_schema`/`ndjson_stream`, `regex_extract`, `state_machine`, `aggregate`, `format_template`, `compact_path`.
- [ ] Build a Layer A parser for file reading (`read`/`smart`) — unblocks the "summarize a long docstring/comment" use case via `bornes/prosa`.

## Cache, progressive disclosure, and dedup (`core/store`)

- [ ] Extend the cache to working-tree-dependent commands (`git status`/`git diff` with no fixed commit — needs to check `.git/index` mtime).
- [ ] Evaluate a file-read cache (key: path + mtime + size, or a content hash).
- [x] Scope dedup to a real session id — uses `CLAUDE_CODE_SESSION_ID` (or `ELAGIX_SESSION_ID`), time window kept as the upper bound (2026-09-24).
- [ ] Validate the cleanup policy (14 days, ~2% sweep per write) at real usage volume, not just the volume generated during development.

## `bornes/mcp`

- [ ] Test against a real production MCP server (today only validated against a fake server written for testing).
- [ ] Automate, or at least formally document, the step of rewriting an MCP server's config to `elagix mcp -- <real command>` — today it's 100% manual and hasn't been done on any real server.
- [ ] Support OAuth and remote HTTP streaming (stdio only today).
- [ ] Field pruning by semantic relevance (pagination, HATEOAS links, redundant timestamps) — today only the 3 mechanical techniques (null-strip, truncation, array cap).
- [ ] Handle/compress requests initiated by the MCP server itself (e.g. `sampling/createMessage`) — pass straight through today.
- [ ] Test and handle the proxy's behavior if the real server dies in the middle of a pending call.

## `bornes/prosa`

- [ ] Improve the sentence splitter to handle common abbreviations (`Mr.`, `v1.2`, etc.).
- [ ] Evaluate an importance heuristic beyond pure TF-IDF (or formally accept the current limitation — it picks by word rarity, not intuitive "importance").
- [ ] Consider a real tokenizer instead of the `bytes/4` estimate.

## Quality / process

- [ ] Write end-to-end integration tests against the compiled binary (today all validation of real behavior is manual/live, not part of `cargo test`).
- [ ] Formally audit determinism (protects the provider's prompt cache, specs §8.4) — today it's only "by construction", with no dedicated test.
- [x] Set up CI (cross-platform build + test) — Linux, Windows cross-compile, macOS.
