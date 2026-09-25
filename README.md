<div align="center">

# Elagix

**Cuts token waste in coding-agent sessions (Claude Code) — for real, without depending on Claude Code features we've already proven broken.**

[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](Cargo.toml)
[![CI](https://github.com/mkmuniz/elagix-tk/actions/workflows/ci.yml/badge.svg)](https://github.com/mkmuniz/elagix-tk/actions/workflows/ci.yml)

</div>

---

## Why this project exists

While setting up [RTK](https://github.com/rtk-ai/rtk) — the tool that inspired this project — we discovered, by testing it live, that its automatic command-rewriting mechanism depends on the `updatedInput` field returned by Claude Code's `PreToolUse` hooks. That field is **silently ignored on Windows** ([publicly confirmed issue, `anthropics/claude-code` #79321](https://github.com/anthropics/claude-code/issues/79321)). Without that mechanism, RTK never even gets invoked — there's no fallback.

We investigated two more hook candidates for solving the same kind of problem (`UserPromptSubmit`, `PostToolUse.updatedToolOutput`) — all three turned out broken or missing on this platform. Design conclusion: **no Elagix mechanism can depend on a Claude Code hook to mutate a command, a prompt, or an output.** It has to intercept from the outside, in layers Claude Code doesn't even know exist.

Elagix does this with a **`$PATH` shim** (the same decades-old technique used by `nvm`/`pyenv`/`asdf`) for shell commands, a **JSON-RPC protocol proxy** for MCP tools, and an **extractive summarization** function for prose — three axes of waste, three independent interception mechanisms, none of them dependent on a hook.

## What it optimizes

| Module (`borne`) | What it compresses | Mechanism | Status |
|---|---|---|---|
| `bornes/comandos` | Output of `git`, `cargo`, `pytest`, `docker`, `npm`, `pnpm`, `yarn`, `pip`, `dotnet`, `go`, `terraform` | `$PATH` shim — intercepts, filters, returns | ✅ Active, validated live |
| `bornes/mcp` | MCP tool schema (lazy loading) + call result | JSON-RPC proxy over stdio | ✅ Validated against a real server (`@modelcontextprotocol/server-filesystem`); opt-in per server |
| `bornes/prosa` | Commit message body (`git log`/`git show`) | TF-IDF extractive summarization (no model, no embeddings) | ✅ Active, integrated into `comandos`'s Layer A |

Any other command (`ls`, `curl`, `make`, `jq`, ...) passes straight through, unfiltered — see [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) for full coverage details and what's missing.

## Measured results (not estimates)

Every percentage below is a real measurement, taken by running the binary against real repositories during this project's development — not an estimate, not a marketing number.

| Command | Raw | Elagix | Reduction |
|---|---|---|---|
| `git status` (clean branch) | 174 B | 28 B | 84% |
| `git log -5` | 9,313 B | 907 B | 90.3% (one line per commit — all 5 stay visible) |
| `git show` (6-file diff) | 32,001 B | 5,740 B | 82.1% (RTK: 71.4% on the same diff) |
| `pytest` (collection error) | 3,245 B | 106 B | 96.7% (preserves the real error reason; RTK doesn't) |
| `docker images` (21 images) | 1,782 B | 1,234 B | 30.8% |
| `docker build` (3-step Dockerfile) | 1,719 B | 175 B | 89.8% |
| `npm install` (deprecated deps) | 675 B | 204 B | 69.8% |
| MCP `tools/list` (2 tools) | 1,047 B | 566 B | 45.9% |
| MCP `tools/list` (real filesystem server, 14 tools) | 13,018 B | 2,940 B | 77.4% |
| MCP `tools/call` (JSON result) | 16,658 B | 4,530 B | 72.8% |
| Repeated `git show <sha>` (cache) | — | — | ~23× faster, byte-identical |
| Repeated command (dedup) | 316 B | 75 B | short reference instead of the full text |

Full detail on each measurement, methodology, and the cases where the technique **doesn't** help (documented with the same honesty) in [`MILESTONES.md`](MILESTONES.md) and [`specs.md`](specs.md) §10.

## Installation

Requires [Rust](https://rustup.rs) — the installer builds from source (there's no release/prebuilt-binary pipeline yet).

**Linux / macOS / WSL:**
```bash
git clone https://github.com/mkmuniz/elagix-tk.git
cd elagix-tk
bash install.sh
```

**Windows (native, outside WSL):**
```powershell
git clone https://github.com/mkmuniz/elagix-tk.git
cd elagix-tk
./install.ps1
```
> ⚠️ `install.ps1` has only been tested in safe mode (no full activation) — none of this project's development machines have native `git`/`cargo`/`npm` on Windows to validate it end to end. See [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md).

After installing, open a new terminal (and reload VS Code / start a new Claude Code session). No need to prefix anything — when an AI agent runs `git status`, `git log`, `cargo test`, etc., the output already comes out filtered. Humans and regular scripts get the untouched output.

Opt-in/out per tool: `ELAGIX_FORCE=1` turns filtering on for an agent that doesn't set `CLAUDECODE`/`AI_AGENT`; `ELAGIX_DISABLE=1` turns it off (e.g. `ELAGIX_DISABLE=1 git diff > x.patch`).

## Using the MCP proxy

MCP servers aren't intercepted automatically — wrap each one you want compressed by putting `elagix mcp [--keep-schemas] --` in front of its command. In Claude Code:

```bash
claude mcp add filesystem -- elagix mcp --keep-schemas -- npx -y @modelcontextprotocol/server-filesystem ~/projects
```

- **`--keep-schemas`** (recommended for Claude Code): leaves `tools/list` untouched and only compresses tool results. Claude Code already loads MCP tool schemas on demand through its own tool search, which relies on the full descriptions — shrinking them there costs more than it saves. Drop the flag for clients that load every schema up front.
- Results are compressed only when they're JSON (nulls dropped, long strings/arrays trimmed with an `elagix show <hash>` recovery hint). Tools that read files (`read`, `file`, `cat`, `open`, `download` in the name, or listed in `ELAGIX_MCP_RAW_TOOLS=a,b`) are never touched, so a file's content always arrives intact.
- If the server dies mid-call, pending requests get a JSON-RPC error instead of hanging.

## How it works

```mermaid
sequenceDiagram
    participant Claude as Claude Code
    participant Shell
    participant Shim as ~/.elagix/shims/git (Elagix binary)
    participant RealGit as real git (original PATH)
    Claude->>Shell: runs "git status" (no prefix)
    Shell->>Shim: resolves "git" -> the shim (ahead in PATH)
    Shim->>Shim: is stdout a TTY, or is no AI agent calling?
    alt TTY or no agent (human, VS Code Git panel, git hooks, scripts)
        Shim->>RealGit: exec directly, no filtering
    else AI agent capturing (CLAUDECODE / AI_AGENT / ELAGIX_FORCE set)
        Shim->>RealGit: runs the real git, captures stdout + exit code
        RealGit-->>Shim: raw output
        Shim->>Shim: Layer A (dedicated parser) or Layer B (declarative rule)
        Shim-->>Claude: compressed output, exit code preserved
    end
```

Two filtering layers for `bornes/comandos`: **Layer A** (hand-written parsers for the highest-volume commands — `git status/log/diff/show`, `pytest`, `cargo test`) and **Layer B** (a declarative TOML rule engine for the long tail, `docker`/`npm`/`terraform`/etc., extensible without recompiling).

## Design principles (non-negotiable)

Motivated by a real, documented risk ([arXiv 2607.13071](https://arxiv.org/abs/2607.13071) — compression that turns "process interrupted" into "confirmed success" for an agent's later sessions):

1. Exit code always preserved and signaled unambiguously.
2. No "success"/"no changes" shortcut if the process failed or was interrupted.
3. **Fail-open**: an error in the filter lets the raw output through unmodified.
4. Raw output always recoverable (`elagix show <hash>`).
5. Never falsify or infer a result — only reformats what actually came out.
6. **Filtered output can never be larger than the original** — if it doesn't shrink, it isn't applied.
7. Always inherits the parent process's own `$PATH`/environment — never resolves a binary on its own.

Full detail on each rule and the empirical evidence behind it: [`specs.md`](specs.md) §4.

## Project structure

```
src/
  main.rs        # entry point: meta-command (core::meta) vs shim (bornes::comandos)
  core/
    store.rs      # content-addressed store — cache, progressive disclosure, dedup
    meta.rs        # routes `elagix show/store/compress/mcp`
  bornes/
    comandos/      # $PATH shim + Layer A (parsers) + Layer B (TOML rules)
    mcp/            # JSON-RPC proxy, schema lazy-loading + result compression
    prosa/          # TF-IDF extractive summarization
```

## Documentation

- [`specs.md`](specs.md) — full technical specification: architecture, business rules, the mechanics of each technique with real examples, the empirical RTK audit that drove the decisions.
- [`MILESTONES.md`](MILESTONES.md) — M0-M8 development history, with every live validation and the real bugs found along the way.
- [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md) — known gaps and limitations, with context on why.
- [`TASKS.md`](TASKS.md) — actionable backlog derived from the known issues.

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md).

## License

[Apache License 2.0](LICENSE).
