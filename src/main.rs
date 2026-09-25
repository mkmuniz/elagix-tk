mod bornes;
mod core;

use std::path::PathBuf;
use std::process::ExitCode;

/// Single entry point for the three `bornes` (specs.md §3) — decides only two
/// things: whether it was invoked as `schliffe` itself (a meta-command,
/// `core::meta`) or as a command shim (`bornes::comandos`, the common case:
/// `argv[0]` is "git"/"cargo"/etc. because it's a symlink/copy created by
/// the installer).
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let invoked_name = PathBuf::from(&args[0])
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("schliffe")
        .to_string();
    let rest_args = &args[1..];

    // "elagix" is the project's former name (renamed 2026-09-25): an old
    // `~/.elagix/bin/elagix` link or a Claude Code hook still pointing at it
    // keeps working as the meta-command instead of being taken for a shim.
    if invoked_name == "schliffe" || invoked_name == "elagix" {
        return core::meta::run(rest_args);
    }

    bornes::comandos::run(&invoked_name, rest_args)
}
