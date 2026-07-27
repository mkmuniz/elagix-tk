#!/usr/bin/env bash
# Elagix installer — Linux/Mac/WSL (specs.md §5.1, M8).
#
# v1 (2026-07-26): builds from source with `cargo`, doesn't download a
# prebuilt binary — there's no release/CDN pipeline yet (specs §13, decision
# recorded in MILESTONES.md). Does three things, all under $HOME, no sudo:
# (1) `cargo build --release`; (2) creates symlinks in `~/.elagix/shims/`
# for each command in the list below, all pointing to the same binary — what
# decides what to filter is `invoked_name` (argv[0]) inside elagix itself,
# not the installer; (3) makes sure `~/.elagix/shims` is at the front of
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
SHIMS_DIR="${ELAGIX_SHIMS_DIR:-$HOME/.elagix/shims}"

# Commands with a known filter today (Layer A: git/pytest/cargo — specs
# §5.4; Layer B: docker/npm/terraform — filters-toml/*.toml). Adding a new
# command here doesn't need new code if a Layer B rule for it already
# exists; it just needs one more symlink.
DEFAULT_COMMANDS=(git cargo pytest docker npm terraform)

if ! command -v cargo >/dev/null 2>&1; then
    echo "elagix: needs cargo (Rust) installed — https://rustup.rs" >&2
    exit 1
fi

echo "elagix: building (cargo build --release)..."
(cd "$SCRIPT_DIR" && cargo build --release)

BIN_PATH="$SCRIPT_DIR/target/release/elagix"
if [ ! -x "$BIN_PATH" ]; then
    echo "elagix: build finished but couldn't find the binary at $BIN_PATH" >&2
    exit 1
fi

mkdir -p "$SHIMS_DIR"
for cmd in "${DEFAULT_COMMANDS[@]}"; do
    ln -sf "$BIN_PATH" "$SHIMS_DIR/$cmd"
done
echo "elagix: shims created in $SHIMS_DIR for: ${DEFAULT_COMMANDS[*]}"

# Detects the right files from the user's login shell, not from whatever
# shell is running this script right now (which could just be "bash" via
# `sh install.sh`).
case "$(basename "${SHELL:-bash}")" in
    zsh) LOGIN_FILE="$HOME/.zprofile"; INTERACTIVE_FILE="$HOME/.zshrc" ;;
    *) LOGIN_FILE="$HOME/.profile"; INTERACTIVE_FILE="$HOME/.bashrc" ;;
esac

PATH_LINE="export PATH=\"$SHIMS_DIR:\$PATH\""

# Login file (~/.profile): plain append — conventionally has no
# interactivity guard, and covers the real `wsl -e bash -lc` case.
if [ -f "$LOGIN_FILE" ] && grep -Fq "$SHIMS_DIR" "$LOGIN_FILE"; then
    echo "elagix: PATH already configured in $LOGIN_FILE (nothing to do)"
else
    {
        echo ""
        echo "# Elagix — \$PATH shim (specs.md §5.1)"
        echo "$PATH_LINE"
    } >> "$LOGIN_FILE"
    echo "elagix: added to PATH in $LOGIN_FILE"
fi

# Interactive file (~/.bashrc): prepend at the TOP of the file, on purpose —
# covers the non-login+interactive case (`bash -ic`), which never reads
# ~/.profile. Only stays ahead of any interactivity guard the file already
# has if our line is the FIRST thing in the file.
if [ -f "$INTERACTIVE_FILE" ] && grep -Fq "$SHIMS_DIR" "$INTERACTIVE_FILE"; then
    echo "elagix: PATH already configured in $INTERACTIVE_FILE (nothing to do)"
elif [ -f "$INTERACTIVE_FILE" ]; then
    TMP_FILE="$(mktemp)"
    {
        echo "# Elagix — \$PATH shim (specs.md §5.1). Must come BEFORE any"
        echo "# interactivity guard the rest of the file already has (e.g."
        echo "# Ubuntu's default \"if not interactive, exit\") — otherwise it"
        echo "# has no effect in the non-login+interactive case (\`bash -ic\`)."
        echo "$PATH_LINE"
        echo ""
        cat "$INTERACTIVE_FILE"
    } > "$TMP_FILE"
    mv "$TMP_FILE" "$INTERACTIVE_FILE"
    echo "elagix: added to the top of $INTERACTIVE_FILE"
else
    { echo "# Elagix — \$PATH shim (specs.md §5.1)"; echo "$PATH_LINE"; } > "$INTERACTIVE_FILE"
    echo "elagix: created $INTERACTIVE_FILE with elagix's PATH"
fi

echo ""
echo "elagix: installed. Open a new terminal to activate it (covers both"
echo "interactive use and the way Claude Code invokes commands via a login shell)."
echo "elagix: test with 'git status | cat' — if it filters, it worked."
