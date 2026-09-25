use crate::bornes;
use crate::core::store;
use std::process::ExitCode;

/// Elagix's own meta-commands — invoked as `elagix <something>` for real,
/// not as a shim (`argv[0]` literally "elagix", no symlink/copy involved).
/// Routes to the `borne` that owns each feature: `elagix mcp` goes to
/// `bornes::mcp` (specs §6), `elagix compress` uses `bornes::prosa`
/// (specs §7.2), `elagix show`/`elagix store` use the shared store
/// (`core::store`, specs §8).
pub fn run(args: &[String]) -> ExitCode {
    // `elagix mcp -- <real command> [args...]` — separated from the other
    // meta-commands because it has variable arity (everything after "--"
    // belongs to the real server, not to Elagix).
    if args.first().map(String::as_str) == Some("mcp") {
        let after_sep = args
            .iter()
            .skip(1)
            .skip_while(|a| a.as_str() != "--")
            .skip(1);
        let server_args: Vec<String> = after_sep.cloned().collect();
        // Options live between "mcp" and "--".
        let lazy_schemas = !args
            .iter()
            .skip(1)
            .take_while(|a| a.as_str() != "--")
            .any(|a| a == "--keep-schemas");
        return match server_args.split_first() {
            Some((cmd, rest)) => bornes::mcp::run(cmd, rest, lazy_schemas),
            None => {
                eprintln!(
                    "usage: elagix mcp [--keep-schemas] -- <real MCP server command> [args...]"
                );
                ExitCode::FAILURE
            }
        };
    }

    match args {
        [cmd, hash] if cmd == "show" => match store::get(hash) {
            Some(content) => {
                print!("{content}");
                if !content.ends_with('\n') {
                    println!();
                }
                ExitCode::SUCCESS
            }
            None => {
                eprintln!("elagix: hash '{hash}' not found in the store");
                ExitCode::FAILURE
            }
        },
        [cmd, sub] if cmd == "store" && sub == "clear" => match store::clear_all() {
            Ok(()) => {
                println!("elagix: store cleared");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("elagix: failed to clear the store: {e}");
                ExitCode::FAILURE
            }
        },
        [cmd] if cmd == "--version" || cmd == "version" => {
            println!("elagix {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        [cmd] if cmd == "stats" => {
            print!("{}", crate::core::stats::report());
            ExitCode::SUCCESS
        }
        [cmd, sub] if cmd == "store" && sub == "gc" => {
            store::force_gc();
            println!("elagix: cleanup sweep completed");
            ExitCode::SUCCESS
        }
        // `elagix compress [--sentences N]` (specs.md §7.2) — standalone
        // utility for `bornes/prosa`: reads all of stdin, summarizes, prints.
        // Only meant for prose that can tolerate losing a whole sentence
        // (commit body, narrative text) — NOT used by the user's
        // `/compress` (specs §7.3 revised: a prompt draft needs sentence-by-
        // sentence semantic judgment, not whole-sentence selection).
        [cmd] if cmd == "compress" => run_compress(None),
        [cmd, flag, n] if cmd == "compress" && flag == "--sentences" => match n.parse::<usize>() {
            Ok(n) => run_compress(Some(n)),
            Err(_) => {
                eprintln!("elagix: '--sentences' needs a number, got '{n}'");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!(
                "usage: elagix --version | elagix stats | elagix show <hash> | elagix store clear | elagix store gc | elagix compress [--sentences N]"
            );
            ExitCode::FAILURE
        }
    }
}

fn run_compress(max_sentences: Option<usize>) -> ExitCode {
    use std::io::Read;
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() {
        eprintln!("elagix: failed to read stdin");
        return ExitCode::FAILURE;
    }
    // Found while testing the cross-compiled .exe on native PowerShell (M8,
    // 2026-07-26): `"text" | elagix.exe compress` arrives with a UTF-8 BOM
    // (U+FEFF) at the front — a known behavior of how native PowerShell
    // encodes a string literal when piping it to a process's stdin, not an
    // elagix bug. A BOM carries no meaning in plain text, so stripping it
    // doesn't risk business rule 5 (nothing substantive is lost).
    if let Some(rest) = text.strip_prefix('\u{feff}') {
        text = rest.to_string();
    }
    let budget = max_sentences.unwrap_or_else(|| bornes::prosa::suggested_sentence_budget(&text));
    let out = bornes::prosa::summarize(&text, budget);
    println!("{out}");
    ExitCode::SUCCESS
}
