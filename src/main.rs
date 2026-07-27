mod bornes;
mod core;

use std::path::PathBuf;
use std::process::ExitCode;

/// Ponto de entrada único pros três `bornes` (specs.md §3) — decide só duas
/// coisas: se foi invocado como `elagix` de verdade (meta-comando, `core::meta`)
/// ou como um shim de comando (`bornes::comandos`, o caso comum: `argv[0]` é
/// "git"/"cargo"/etc. porque é um symlink/cópia criado pelo instalador).
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
