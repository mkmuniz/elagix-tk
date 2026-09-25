# Elagix — Known issues and improvements

A consolidation of everything marked "left for later" across M0-M8 (previously scattered across `specs.md` §13 and each milestone's notes in `MILESTONES.md`). Nothing here blocks current use — these are known gaps, not hidden bugs.

## Platform / installation

- **`install.ps1` not validated with full activation.** Only tested in safe mode (an isolated copy, without touching the real PATH) — this development machine has no native `git`/`cargo`/`npm` on Windows (they only exist via WSL), so there was no way to validate intercepting a real native binary. Needs to run on a Windows machine with a native toolchain installed.
- **macOS: native build only, no cross-compile from Linux.** Builds and passes all tests natively on Apple Silicon (2026-09-24), and CI now has a `macos-latest` job. Cross-compiling *from* Linux is still blocked by the missing SDK (MILESTONES.md M8) — not needed while CI builds natively.
- **Shims only activate inside AI agents.** Since 2026-09-24 the shim filters only when an agent marker is set (`CLAUDECODE`, `AI_AGENT`, or `ELAGIX_FORCE=1`; `ELAGIX_DISABLE=1` turns it off). Other agents that set none of these pass through unfiltered until they're added or the user sets `ELAGIX_FORCE=1` in that tool's environment.
- **Redirects inside an agent are still filtered.** Claude Code captures command output into a regular file, so `git diff > x.patch` run *by the agent* is indistinguishable from normal capture and gets filtered (an invalid patch). Pipes into other programs (`git log | grep`) are filtered too. Workaround: `ELAGIX_DISABLE=1 git diff > x.patch`.
- **Python venvs / version managers can bypass shims.** Activating a venv (or similar) prepends its own `bin/` ahead of `~/.elagix/shims`, so e.g. `pytest` from the venv isn't intercepted.
- **`install.sh`/`install.ps1` always build from source** — there's no prebuilt-binary download yet. A release pipeline (CI + published binaries) is a phase-2 idea (specs §3, original plan).
- **Critical finding already fixed, but worth remembering for any new platform**: shim activation depends entirely on HOW the tool you want to intercept actually invokes a shell (login vs. interactive, etc. — see `specs.md` §5.1 and `MILESTONES.md`, "Critical post-M8 fix" section). Before declaring something "ready" on a new platform, you need to confirm the invocation pattern there experimentally, not assume it generalizes from WSL/Linux.

## Command coverage (Layer B / `bornes/comandos`)

- **Layer B covers 15 filters, still not the whole long tail.** Since 2026-09-24: `npm`/`pnpm`/`yarn` install, `pip`/`pip3` install, `docker` images/pull/build, `dotnet` build/test/run, cargo's stderr progress, `go test` + go's download chatter, `git branch`, `terraform plan`. The `go` and `terraform` filters are unit-tested only (no Go/Terraform on the dev machine). Still no rule for: `tsc`, `rg`, `make`, `jq`, `poetry`, `uv`, `nx`, `turbo`, `pre-commit`, `eslint`, `jest`/`vitest`, `npm run`, among others. `yarn` with no arguments (= install) isn't matched.
- **Filters that target stderr reorder the output**: when a filter asks for stderr (e.g. `docker build`, `npm install`), stderr is captured and printed before stdout instead of interleaved as the tool wrote it.
- **Catalog actions (specs §5.3) not implemented yet**: `group_by`, `json_extract`/`json_schema`/`ndjson_stream`, `regex_extract`, `state_machine`, `aggregate`, `format_template`, `compact_path`. None of the 4 v1 filters have needed them yet.
- **No `read`/`smart` parser** (file reading) in Layer A — blocks the "summarize a long docstring/comment" use case that `bornes/prosa` could cover (specs §13, already resolved as "out of v1" for lack of somewhere to plug it in).

## Cache, progressive disclosure, and dedup (`core/store`)

- **Cache (specs §8.2) only covers `git show <explicit sha>`.** Working-tree cache (`git status`/`git diff` with no fixed commit, would need to check `.git/index` mtime) and file-read cache were left out — neither has a Layer A parser to lean on yet.
- **Dedup is session-scoped only when the agent exposes a session id.** Claude Code sets `CLAUDE_CODE_SESSION_ID` (used since 2026-09-24; `ELAGIX_SESSION_ID` works for other agents). Without one it falls back to the time window (`ELAGIX_DEDUP_WINDOW_SECS`, default 1,800s). Not yet verified: whether Claude Code subagents get their own session id — if they inherit the parent's, a subagent could get a "same as previous output" reference for something only the parent saw (still recoverable via `elagix show`).
- **Cleanup policy (14 days, ~2% probabilistic sweep) never tested at real scale** — only with the small volume generated during this development session.

## `bornes/mcp`

- **Only tested against a fake MCP server** (our own fixture, `fake_mcp_server.py`), never against a real production server. The mechanism is validated, real-world compatibility isn't.
- **Stdio only.** OAuth and remote HTTP streaming aren't supported (specs §13, an explicit v1 scope decision).
- **No field pruning by semantic relevance** (pagination, HATEOAS links, redundant timestamps) — only the 3 purely mechanical techniques (null-strip, string truncation, array cap). Pruning by relevance would require knowing the specific API, which would go against business rule 5.
- **Requests the MCP server itself initiates** (e.g. `sampling/createMessage`) pass straight through with no interception or compression — not the token-waste axis that motivated this borne, but also not addressed.
- **No explicit handling of the child process crashing** mid-session (what happens to the proxy if the real server dies in the middle of a pending call hasn't been exercised).

## `bornes/prosa`

- **The sentence splitter is naive**: cuts on `.`/`!`/`?` followed by a space, with no special-casing for abbreviations (`Mr.`, `v1.2`). Acceptable for the real use case (commit body, short prose), bad for text dense with abbreviations.
- **TF-IDF scores by statistical word rarity, not intuitive "importance"** — validated live that a summary sometimes picks a sentence a human wouldn't have picked first (`elagix compress`, MILESTONES.md M7). Expected algorithm behavior, not a bug, but worth keeping in mind when interpreting a summary.
- **The user's `/compress` doesn't use `bornes/prosa`** — a deliberate decision (specs §7.2/§7.3, they're different tasks), not a gap to close.

## Quality / process

- **`bytes/4` as the token estimate**, never a real tokenizer — the same approximation RTK/snip use, followed for comparison consistency (specs §5.4.1), not precision. A real tokenizer is a phase-2 idea (specs §11).
- **Determinism (specs §8.4) never formally audited** — assumed "by construction" (no layer intentionally uses a timestamp/non-deterministic ordering), but there's no dedicated test proving this to protect the provider's prompt cache.
- **No end-to-end integration test in `cargo test`** — every validation of the compiled binary's actual behavior (live shim, `git show` cache, PATH activation) was manual/live this session, not part of the automated suite.
- **A known and accepted architectural ceiling, not a bug**: static per-command rules are measurably worse than pruning conditioned on the agent's task/intent (arXiv 2604.04979/2604.19572, specs §11) — would require a trained model or intent context passed to the filter, against the project's deterministic philosophy. Recorded, not pursued.
- **Dilution effect** (specs §11): a token reduction in one command's output doesn't equal a reduction in the session's total cost (prompt, history, system prompt also count) — be careful reporting whole-session savings based only on per-command savings.

## Still-open decisions

- None remaining that block current use — the last one ("final name") was resolved: **the project's name is Elagix**, formally confirmed (see specs.md §13).
