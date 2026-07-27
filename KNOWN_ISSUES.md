# Elagix — Known issues and improvements

A consolidation of everything marked "left for later" across M0-M8 (previously scattered across `specs.md` §13 and each milestone's notes in `MILESTONES.md`). Nothing here blocks current use — these are known gaps, not hidden bugs.

## Platform / installation

- **`install.ps1` not validated with full activation.** Only tested in safe mode (an isolated copy, without touching the real PATH) — this development machine has no native `git`/`cargo`/`npm` on Windows (they only exist via WSL), so there was no way to validate intercepting a real native binary. Needs to run on a Windows machine with a native toolchain installed.
- **No macOS cross-compile.** Blocked by a missing SDK/Xcode (`cc: unrecognized -arch/-mmacosx-version-min`, evidence in MILESTONES.md M8). Needs a real Mac or a macOS CI runner (e.g. GitHub Actions `macos-latest`) — neither configured yet.
- **`install.sh`'s `zsh` path never tested live** — only `bash`, this machine's real shell. The logic mirrors bash's (`~/.zprofile` + top of `~/.zshrc`), but it hasn't actually been exercised.
- **`install.sh`/`install.ps1` always build from source** — there's no prebuilt-binary download yet. A release pipeline (CI + published binaries) is a phase-2 idea (specs §3, original plan).
- **Critical finding already fixed, but worth remembering for any new platform**: shim activation depends entirely on HOW the tool you want to intercept actually invokes a shell (login vs. interactive, etc. — see `specs.md` §5.1 and `MILESTONES.md`, "Critical post-M8 fix" section). Before declaring something "ready" on a new platform, you need to confirm the invocation pattern there experimentally, not assume it generalizes from WSL/Linux.

## Command coverage (Layer B / `bornes/comandos`)

- **Only 4 example TOML filters exist** (`docker-images`, `git-branch`, `terraform-plan`, `npm-install`). RTK's audit (specs §10) mapped ~60 long-tail commands that never got a rule: `go-build`, `tsc`, `rg`, `make`, `jq`, `poetry`, `uv`, `mise`, `jj`, `nx`, `turbo`, `pre-commit`, `grep`, `fd`, `tree`, `wc`, `df`, `stat`, `shellcheck`, `yamllint`, `oxlint`, `ruff-format`, `cargo-clippy`, `ls-la`, `golangci-lint`, among others.
- **Layer B's pipeline order is fixed**, not configurable per filter (always `strip_ansi → replace → match_output → keep/strip_lines → dedup → truncate_lines → max_lines → on_empty`, see `bornes/comandos/camada_b/engine.rs`). Never needed to change for the 4 existing filters, but may not fit every future case.
- **Catalog actions (specs §5.3) not implemented yet**: `group_by`, `json_extract`/`json_schema`/`ndjson_stream`, `regex_extract`, `state_machine`, `aggregate`, `format_template`, `compact_path`. None of the 4 v1 filters have needed them yet.
- **No `read`/`smart` parser** (file reading) in Layer A — blocks the "summarize a long docstring/comment" use case that `bornes/prosa` could cover (specs §13, already resolved as "out of v1" for lack of somewhere to plug it in).

## Cache, progressive disclosure, and dedup (`core/store`)

- **Cache (specs §8.2) only covers `git show <explicit sha>`.** Working-tree cache (`git status`/`git diff` with no fixed commit, would need to check `.git/index` mtime) and file-read cache were left out — neither has a Layer A parser to lean on yet.
- **Dedup (specs §8.3) approximates "session" with a time window** (`ELAGIX_DEDUP_WINDOW_SECS`, default 1,800s), not a real session id — the shim has no access to any stable Claude Code identifier. May deduplicate across two sessions close in time, or fail to deduplicate within one very long session with big gaps.
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
- **The `elagix` binary itself isn't on `$PATH`** — only the shim names (`git`, `cargo`, `pytest`, `docker`, `npm`, `terraform`) are symlinks/copies in `~/.elagix/shims`. `elagix show`/`elagix store`/`elagix compress`/`elagix mcp` are only reachable today via the binary's full path (`~/projects/elagix/target/release/elagix show ...`), not a bare `elagix`. Found live while testing after the restructuring (2026-07-26) — not a regression, it's been like this since M4/M7, it just hadn't been noticed.

## Still-open decisions

- None remaining that block current use — the last one ("final name") was resolved: **the project's name is Elagix**, formally confirmed (see specs.md §13).
