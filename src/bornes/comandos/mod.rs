mod camada_b;
mod filters;
mod shim;

use crate::core::store;
use std::process::ExitCode;

/// Tamanho mínimo pra dedup entrar em jogo (specs §8.3) — abaixo disso, a
/// linha de referência custaria mais do que economiza.
const DEDUP_MIN_BYTES: usize = 200;

/// `bornes/comandos` (specs.md §5) — shim de `$PATH`: intercepta a saída de
/// comando de shell (`git`, `docker`, `cargo`, `pytest`...) invocado como
/// `invoked_name` (o `argv[0]` do processo, resolvido pelo `main.rs`).
pub fn run(invoked_name: &str, rest_args: &[String]) -> ExitCode {
    let Some(real_bin) = shim::resolve_real_binary(invoked_name) else {
        eprintln!("elagix: não achei o binário real de '{invoked_name}' no PATH");
        return ExitCode::FAILURE;
    };

    // Passthrough total pra uso humano interativo — nunca filtra quando é um TTY
    // (specs.md §5.1). No Unix isso substitui o processo atual (exec de verdade).
    if shim::stdout_is_tty() {
        if let Err(e) = shim::exec_passthrough(&real_bin, rest_args) {
            eprintln!("elagix: falha ao executar {invoked_name}: {e}");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS; // inatingível no Unix (exec substitui o processo)
    }

    // Cache (specs.md §8.2, escopo v1 decidido em §13): só o único caso
    // comprovadamente imutável — `git show <sha explícito>`. `HEAD`/branch
    // ficam de fora porque podem apontar pra outro commit amanhã. A chave
    // carrega uma versão do formato ("v1") pra nunca servir saída obsoleta se
    // o filtro de `git_diff` mudar no futuro.
    let git_show_cache_key = if invoked_name == "git"
        && rest_args.len() == 2
        && rest_args[0] == "show"
        && looks_like_git_sha(&rest_args[1])
    {
        Some(format!("git-show:v1:{}", rest_args[1]))
    } else {
        None
    };
    // (chegar até aqui já implica caminho não-interativo — o branch de TTY
    // acima sempre retorna/substitui o processo antes de chegar nesta linha.)
    if let Some(key) = &git_show_cache_key {
        if let Some(cached) = store::get_keyed(key) {
            print!("{cached}");
            if !cached.ends_with('\n') {
                println!();
            }
            return ExitCode::SUCCESS;
        }
    }

    // Caminho não-interativo (pipe) — aqui entra a filtragem.
    let (run_args, subcommand): (Vec<String>, Option<&str>) = match invoked_name {
        "git" if rest_args.first().map(String::as_str) == Some("status")
            && rest_args.len() == 1 =>
        {
            (
                vec!["status".into(), "--porcelain=v1".into(), "--branch".into()],
                Some("git-status"),
            )
        }
        "git" if rest_args.first().map(String::as_str) == Some("log") => {
            (rest_args.to_vec(), Some("git-log"))
        }
        "git"
            if matches!(rest_args.first().map(String::as_str), Some("diff") | Some("show")) =>
        {
            (rest_args.to_vec(), Some("git-diff"))
        }
        "pytest" => (rest_args.to_vec(), Some("pytest")),
        "cargo" if rest_args.first().map(String::as_str) == Some("test") => {
            (rest_args.to_vec(), Some("cargo-test"))
        }
        _ => (rest_args.to_vec(), None),
    };

    let captured = match shim::run_captured(&real_bin, &run_args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("elagix: falha ao executar {invoked_name}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let raw = String::from_utf8_lossy(&captured.stdout);

    // Camada B (specs.md §5.2/§5.3): só entra em jogo quando nenhum parser
    // dedicado da Camada A bateu — é o fallback declarativo pra cauda longa.
    let camada_b_filters = camada_b::load_all();
    let camada_b_match = if subcommand.is_none() {
        camada_b::find_match(&camada_b_filters, invoked_name, rest_args)
    } else {
        None
    };

    // Regra de negócio 2 (specs.md §4): nenhum atalho de sucesso se o processo não
    // terminou com sucesso. Isso é responsabilidade de CADA filtro reportar a
    // verdade (nunca fabricar "sucesso"), não de desligar a filtragem inteira em
    // qualquer saída não-zero — aliás o caso de maior valor do pytest (falha na
    // coleta de teste) só existe justamente quando o exit code NÃO é zero. Achado
    // ao vivo testando nesta sessão (2026-07-26): a versão anterior desligava o
    // filtro exatamente no caso que mais queríamos demonstrar. A Camada B aplica a
    // mesma regra internamente pros seus próprios atalhos (`match_output`/
    // `on_empty`) — ver camada_b/engine.rs.
    let filtered = match subcommand {
        Some("git-status") => Some(filters::git_status::filter(&raw)),
        Some("git-log") => Some(filters::git_log::filter(&raw)),
        Some("git-diff") => Some(filters::git_diff::filter(&raw)),
        Some("pytest") => Some(filters::pytest::filter(&raw)),
        Some("cargo-test") => Some(filters::cargo_test::filter(&raw)),
        _ => camada_b_match.map(|f| camada_b::apply(&f.pipeline, &raw, captured.exit_code)),
    };

    // Disclosure progressivo (specs.md §8.1): quando o filtro sinaliza que
    // conteúdo de verdade foi descartado (não só reformatado), guarda o bruto
    // no armazém e anexa uma dica recuperável. Testado ANTES da regra 6 de
    // propósito: se a dica não couber no orçamento, a regra 6 cai pro bruto
    // completo — que já É a informação total, então nada se perde de qualquer
    // forma.
    let filtered = filtered.map(|f| {
        if f.contains("lines omitted") || f.contains("more changed lines") {
            let hash = store::put(&raw);
            format!("{f}\n(bruto completo: elagix show {hash})")
        } else {
            f
        }
    });

    // Regra de negócio 6: saída filtrada nunca pode ser maior que a original.
    let mut output = match filtered {
        Some(f) if f.len() < raw.len() => f,
        _ => raw.into_owned(),
    };

    // Cache (§8.2): só grava depois de confirmar sucesso — nunca cacheia
    // processo que falhou/foi interrompido (regra de negócio 2/3).
    if let Some(key) = &git_show_cache_key {
        if captured.exit_code == 0 {
            store::put_keyed(key, &output);
        }
    }

    // Deduplicação (§8.3) — a única das duas técnicas de armazém que reduz
    // token de fato. Aproxima "mesma sessão" por janela de tempo (limitação
    // documentada em specs §8.3: não há id de sessão estável disponível aqui).
    if output.len() >= DEDUP_MIN_BYTES {
        let hash = store::put(&output); // garante recuperável via `elagix show`
        let window: u64 = std::env::var("ELAGIX_DEDUP_WINDOW_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1800);
        if let store::Dedup::SeenRecently = store::check_and_record_dedup(&output, window) {
            let msg = format!("(igual à saída anterior — elagix show {hash} pra ver de novo)");
            if msg.len() < output.len() {
                output = msg;
            }
        }
    }

    print!("{output}");
    if !output.ends_with('\n') {
        println!();
    }

    ExitCode::from(captured.exit_code as u8)
}

fn looks_like_git_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}
