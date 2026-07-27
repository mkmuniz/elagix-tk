mod bornes;
mod core;

use std::path::PathBuf;
use std::process::ExitCode;

/// Single entry point for the three `bornes` (specs.md §3) — decides only two
/// things: whether it was invoked as `elagix` itself (a meta-command,
/// `core::meta`) or as a command shim (`bornes::comandos`, the common case:
/// `argv[0]` is "git"/"cargo"/etc. because it's a symlink/copy created by
/// the installer).
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let invoked_name = PathBuf::from(&args[0])
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("elagix")
        .to_string();
    let rest_args = &args[1..];

    if invoked_name == "elagix" {
        return core::meta::run(rest_args);
    }

    bornes::comandos::run(&invoked_name, rest_args)
}
