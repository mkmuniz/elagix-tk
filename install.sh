#!/usr/bin/env bash
# Schliffe installer — Linux/Mac/WSL (specs.md §5.1, M8).
#
# v1 (2026-07-26): builds from source with `cargo`, doesn't download a
# prebuilt binary — there's no release/CDN pipeline yet (specs §13, decision
# recorded in MILESTONES.md). Does three things, all under $HOME, no sudo:
# (1) `cargo build --release`; (2) creates symlinks in `~/.schliffe/shims/`
# for each command in the list below, all pointing to the same binary — what
# decides what to filter is `invoked_name` (argv[0]) inside schliffe itself,
# not the installer; (3) makes sure `~/.schliffe/shims` is at the front of
# $PATH in TWO files, not just one — real finding from live testing
# (2026-07-26): Claude Code actually invokes commands as
# `wsl -e bash -lc "..."` (login, NON-interactive). `~/.bashrc` alone (where
# v0 of this installer put the line) NEVER runs in that case — the standard
# "if not interactive, exit" guard at the top of Ubuntu's `.bashrc` returns
# before reaching any line appended at the end of the file. bash uses
# different files depending on the login/interactive combination, and no
# single file covers the two combinations that matter here:
#   - login (interactive or not, includes `-lc`)      -> ~/.profile (or ~/.zprofile)
#   - non-login but interactive (e.g. `bash -ic`)      -> ~/.bashrc (or ~/.zshrc)
# That's why the line goes into BOTH, with `.bashrc`/`.zshrc` getting the
# line BEFORE the rest of the content (not at the end) — if it were placed
# after an interactivity guard the file already has, it would have no effect
# in the login+non-interactive case even while being present.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SHIMS_DIR="${SCHLIFFE_SHIMS_DIR:-$HOME/.schliffe/shims}"

# Commands with a known filter today (Layer A: git/pytest/cargo — specs
# §5.4; Layer B: docker/npm/terraform — filters-toml/*.toml). Adding a new
# command here doesn't need new code if a Layer B rule for it already
# exists; it just needs one more symlink.
DEFAULT_COMMANDS=(git cargo pytest docker npm pnpm yarn pip pip3 dotnet go terraform)

if ! command -v cargo >/dev/null 2>&1; then
    echo "schliffe: needs cargo (Rust) installed — https://rustup.rs" >&2
    exit 1
fi

echo "schliffe: building (cargo build --release)..."
(cd "$SCRIPT_DIR" && cargo build --release)

BIN_PATH="$SCRIPT_DIR/target/release/schliffe"
if [ ! -x "$BIN_PATH" ]; then
    echo "schliffe: build finished but couldn't find the binary at $BIN_PATH" >&2
    exit 1
fi

# Installs a COPY of the binary outside the repo: shims pointing straight at
# target/release would all break (git/npm/... "not found" in every shell)
# the moment someone runs `cargo clean` while developing Schliffe itself.
INSTALL_BIN_DIR="${SCHLIFFE_BIN_DIR:-$HOME/.schliffe/bin}"
mkdir -p "$INSTALL_BIN_DIR"
# Copy to a temp name then rename: replacing the file in place would break
# shims that are running right now.
cp "$BIN_PATH" "$INSTALL_BIN_DIR/schliffe.new"
mv -f "$INSTALL_BIN_DIR/schliffe.new" "$INSTALL_BIN_DIR/schliffe"
BIN_PATH="$INSTALL_BIN_DIR/schliffe"

# Migration from Elagix, the project's former name (renamed 2026-09-25).
# Moves the savings history, the store and custom filters to the new
# folder, drops the old shims and their PATH lines (backing up each shell
# file it touches), and leaves ~/.elagix/bin/elagix as a link to the new
# binary: Claude Code sessions opened before the rename still call the hook
# through that path (the binary answers to both names). The Claude Code
# hook itself is re-registered under the new path at the end.
OLD_ROOT="$HOME/.elagix"
NEW_ROOT="$HOME/.schliffe"
if [ -d "$OLD_ROOT" ] && [ -z "${SCHLIFFE_SHIMS_DIR:-}" ]; then
    echo "schliffe: migrating from Elagix (the project's former name)..."
    mkdir -p "$NEW_ROOT"
    for item in stats.log store filters; do
        if [ -e "$OLD_ROOT/$item" ] && [ ! -e "$NEW_ROOT/$item" ]; then
            mv "$OLD_ROOT/$item" "$NEW_ROOT/$item"
        fi
    done
    rm -rf "${OLD_ROOT:?}/shims"
    mkdir -p "$OLD_ROOT/bin"
    ln -sf "$BIN_PATH" "$OLD_ROOT/bin/elagix"
    for rc in "$HOME/.zprofile" "$HOME/.zshrc" "$HOME/.profile" "$HOME/.bashrc"; do
        if [ -f "$rc" ] && grep -q '/\.elagix/shims' "$rc"; then
            cp "$rc" "$rc.elagix-backup"
            grep -v \
                -e '/\.elagix/shims' \
                -e '^# Elagix — ' \
                -e '^# file, after nvm/pyenv/etc., so the shims stay first in PATH\.$' \
                -e '^# interactivity guard the rest of the file already has' \
                -e "^# Ubuntu's default \"if not interactive, exit\"" \
                -e '^# has no effect in the non-login+interactive case' \
                "$rc.elagix-backup" > "$rc" || true
            echo "schliffe: removed Elagix's PATH line from $rc (backup: $rc.elagix-backup)"
        fi
    done
    echo "schliffe: migration done — ~/.elagix now only holds a compatibility link;"
    echo "          delete it once every Claude Code session has been restarted."
fi

mkdir -p "$SHIMS_DIR"
# Only shims commands that actually exist on this machine (looked up with the
# shims folder removed from PATH, so a re-run doesn't find its own shims): a
# shim for a missing tool would make `command -v terraform` succeed and
# mislead scripts that probe for it. Installed a new tool later? Re-run this.
#
# The lookup uses the union of this process's PATH and the PATH of the
# user's full login+interactive shell (found 2026-09-25): run from a trimmed
# environment — e.g. an agent's shell that never sourced ~/.zshrc, where nvm
# and dotnet live — `npm`/`dotnet` looked "not installed". And an existing
# shim is never removed: a tool that can't be seen from here isn't proof
# it's gone.
USER_SHELL_PATH="$("${SHELL:-/bin/bash}" -lic 'printf %s "$PATH"' </dev/null 2>/dev/null | tail -n 1 || true)"
REAL_PATH="$(printf '%s:%s' "$PATH" "$USER_SHELL_PATH" | tr ':' '\n' | grep -v '^$' | grep -vxF "$SHIMS_DIR" | awk '!seen[$0]++' | paste -sd: -)"
SHIMMED=()
SKIPPED=()
for cmd in "${DEFAULT_COMMANDS[@]}"; do
    if PATH="$REAL_PATH" command -v "$cmd" >/dev/null 2>&1 || [ -L "$SHIMS_DIR/$cmd" ]; then
        ln -sf "$BIN_PATH" "$SHIMS_DIR/$cmd"
        SHIMMED+=("$cmd")
    else
        SKIPPED+=("$cmd")
    fi
done
# `schliffe` itself, so `schliffe show <hash>` (the recovery hint printed in
# filtered output) works as a bare command, not only via the full path.
ln -sf "$BIN_PATH" "$SHIMS_DIR/schliffe"
echo "schliffe: shims created in $SHIMS_DIR for: ${SHIMMED[*]:-none} (+ schliffe)"
if [ ${#SKIPPED[@]} -gt 0 ]; then
    echo "schliffe: not installed here, skipped: ${SKIPPED[*]} (re-run after installing them)"
fi

# Detects the right files from the user's login shell, not from whatever
# shell is running this script right now (which could just be "bash" via
# `sh install.sh`).
case "$(basename "${SHELL:-bash}")" in
    zsh) LOGIN_FILE="$HOME/.zprofile"; INTERACTIVE_FILE="$HOME/.zshrc"; SHELL_KIND=zsh ;;
    *) LOGIN_FILE="$HOME/.profile"; INTERACTIVE_FILE="$HOME/.bashrc"; SHELL_KIND=bash ;;
esac

PATH_LINE="export PATH=\"$SHIMS_DIR:\$PATH\""

# Login file (~/.profile): plain append — conventionally has no
# interactivity guard, and covers the real `wsl -e bash -lc` case.
if [ -f "$LOGIN_FILE" ] && grep -Fq "$SHIMS_DIR" "$LOGIN_FILE"; then
    echo "schliffe: PATH already configured in $LOGIN_FILE (nothing to do)"
else
    {
        echo ""
        echo "# Schliffe — \$PATH shim (specs.md §5.1)"
        echo "$PATH_LINE"
    } >> "$LOGIN_FILE"
    echo "schliffe: added to PATH in $LOGIN_FILE"
fi

# Interactive file (~/.bashrc): prepend at the TOP of the file, on purpose —
# covers the non-login+interactive case (`bash -ic`), which never reads
# ~/.profile. Only stays ahead of any interactivity guard the file already
# has if our line is the FIRST thing in the file.
#
# zsh is the exception (validated live on macOS, 2026-09-24): `.zshrc` has
# no interactivity guard, and what it usually DOES have is nvm/pyenv/etc.
# prepending their own bin folders — a line at the top would end up behind
# them (`npm` from nvm would win over the shim). So for zsh the line goes at
# the END, after everything else has touched PATH.
if [ -f "$INTERACTIVE_FILE" ] && grep -Fq "$SHIMS_DIR" "$INTERACTIVE_FILE"; then
    echo "schliffe: PATH already configured in $INTERACTIVE_FILE (nothing to do)"
elif [ "$SHELL_KIND" = zsh ]; then
    {
        echo ""
        echo "# Schliffe — \$PATH shim (specs.md §5.1). Keep this at the END of the"
        echo "# file, after nvm/pyenv/etc., so the shims stay first in PATH."
        echo "$PATH_LINE"
    } >> "$INTERACTIVE_FILE"
    echo "schliffe: added to the end of $INTERACTIVE_FILE"
elif [ -f "$INTERACTIVE_FILE" ]; then
    TMP_FILE="$(mktemp)"
    {
        echo "# Schliffe — \$PATH shim (specs.md §5.1). Must come BEFORE any"
        echo "# interactivity guard the rest of the file already has (e.g."
        echo "# Ubuntu's default \"if not interactive, exit\") — otherwise it"
        echo "# has no effect in the non-login+interactive case (\`bash -ic\`)."
        echo "$PATH_LINE"
        echo ""
        cat "$INTERACTIVE_FILE"
    } > "$TMP_FILE"
    mv "$TMP_FILE" "$INTERACTIVE_FILE"
    echo "schliffe: added to the top of $INTERACTIVE_FILE"
else
    { echo "# Schliffe — \$PATH shim (specs.md §5.1)"; echo "$PATH_LINE"; } > "$INTERACTIVE_FILE"
    echo "schliffe: created $INTERACTIVE_FILE with schliffe's PATH"
fi

# Claude Code hook (bornes/hook): covers what the PATH shim can't reach —
# remote MCP servers (HTTP/OAuth, e.g. Figma) and images. Registered only if
# Claude Code is present (~/.claude exists); idempotent, keeps a backup of
# settings.json. Skip with SCHLIFFE_NO_HOOK=1; undo with `schliffe hook uninstall`.
if [ -n "${SCHLIFFE_NO_HOOK:-}" ]; then
    echo "schliffe: skipping the Claude Code hook (SCHLIFFE_NO_HOOK is set)"
elif [ -d "$HOME/.claude" ]; then
    "$BIN_PATH" hook install
else
    echo "schliffe: Claude Code not found (~/.claude missing) — skipped its hook."
    echo "        Install it later with: schliffe hook install"
fi

echo ""
echo "schliffe: installed. Open a new terminal to activate it (covers both"
echo "interactive use and the way Claude Code invokes commands via a login shell)."
echo "schliffe: test with 'git status | cat' — if it filters, it worked."
