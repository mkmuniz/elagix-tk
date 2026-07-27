use crate::bornes;
use crate::core::store;
use std::process::ExitCode;

/// Meta-comandos do próprio Elagix — invocados como `elagix <algo>` de verdade,
/// não como um shim (`argv[0]` literalmente "elagix", sem symlink/cópia
/// nenhuma). Roteia pro `borne` dono de cada funcionalidade: `elagix mcp`
/// entra em `bornes::mcp` (specs §6), `elagix compress` usa `bornes::prosa`
/// (specs §7.2), `elagix show`/`elagix store` usam o armazém compartilhado
/// (`core::store`, specs §8).
pub fn run(args: &[String]) -> ExitCode {
    // `elagix mcp -- <comando real> [args...]` — separado dos outros
    // meta-comandos porque tem aridade variável (o resto dos args depois de
    // "--" pertence ao servidor real, não ao Elagix).
    if args.first().map(String::as_str) == Some("mcp") {
        let after_sep = args.iter().skip(1).skip_while(|a| a.as_str() != "--").skip(1);
        let server_args: Vec<String> = after_sep.cloned().collect();
        return match server_args.split_first() {
            Some((cmd, rest)) => bornes::mcp::run(cmd, rest),
            None => {
                eprintln!("uso: elagix mcp -- <comando do servidor MCP real> [args...]");
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
                eprintln!("elagix: hash '{hash}' não encontrado no armazém");
                ExitCode::FAILURE
            }
        },
        [cmd, sub] if cmd == "store" && sub == "clear" => match store::clear_all() {
            Ok(()) => {
                println!("elagix: armazém limpo");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("elagix: falha limpando o armazém: {e}");
                ExitCode::FAILURE
            }
        },
        [cmd, sub] if cmd == "store" && sub == "gc" => {
            store::force_gc();
            println!("elagix: varredura de limpeza concluída");
            ExitCode::SUCCESS
        }
        // `elagix compress [--sentences N]` (specs.md §7.2) — utilitário
        // standalone de `bornes/prosa`: lê stdin inteiro, resume, imprime.
        // Só serve pra prosa que tolera perder frase inteira (corpo de
        // commit, trecho narrativo) — NÃO é usado pelo `/compress` do
        // usuário (specs §7.3 revisado: rascunho de prompt precisa de
        // julgamento semântico frase-a-frase, não seleção de frase inteira).
        [cmd] if cmd == "compress" => run_compress(None),
        [cmd, flag, n] if cmd == "compress" && flag == "--sentences" => {
            match n.parse::<usize>() {
                Ok(n) => run_compress(Some(n)),
                Err(_) => {
                    eprintln!("elagix: '--sentences' precisa de um número, recebi '{n}'");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!(
                "uso: elagix show <hash> | elagix store clear | elagix store gc | elagix compress [--sentences N]"
            );
            ExitCode::FAILURE
        }
    }
}

fn run_compress(max_sentences: Option<usize>) -> ExitCode {
    use std::io::Read;
    let mut text = String::new();
    if std::io::stdin().read_to_string(&mut text).is_err() {
        eprintln!("elagix: falha lendo stdin");
        return ExitCode::FAILURE;
    }
    // Achado testando o .exe cross-compilado no PowerShell nativo (M8,
    // 2026-07-26): `"texto" | elagix.exe compress` chega com um BOM UTF-8
    // (U+FEFF) na frente — comportamento conhecido do PowerShell ao mandar
    // string literal pro stdin de um processo nativo via pipe, não é bug do
    // elagix. BOM não carrega significado nenhum em texto puro, então tirar
    // não arrisca a regra de negócio 5 (nada de substantivo é perdido).
    if let Some(rest) = text.strip_prefix('\u{feff}') {
        text = rest.to_string();
    }
    let budget = max_sentences.unwrap_or_else(|| bornes::prosa::suggested_sentence_budget(&text));
    let out = bornes::prosa::summarize(&text, budget);
    println!("{out}");
    ExitCode::SUCCESS
}
