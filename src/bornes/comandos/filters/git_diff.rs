const MAX_CHANGED_LINES_PER_FILE: usize = 10;

/// Camada A — parser de diff unificado (specs.md §5.4c, "parsing estrutural de
/// verdade"). Mantém os hunks por arquivo, com um teto de linhas alteradas por
/// arquivo (mesma técnica documentada do RTK, seção 9 do specs).
///
/// M7 (specs.md §7.2): quando há cabeçalho de commit (`git show`, não `git
/// diff` puro), esse cabeçalho não é mais descartado por completo — antes o
/// hash/assunto/corpo do commit sumiam inteiros, mesmo o hash (perda real:
/// não dava pra saber qual commit era esse diff sem rodar outro comando). Agora
/// mantém "commit <hash> — <assunto>" e resume o corpo via `bornes/prosa` em
/// vez de descartar.
pub fn filter(raw: &str) -> String {
    // `git show` tem cabeçalho de commit antes do primeiro "diff --git" (por isso
    // procuramos "\ndiff --git" no meio do texto); `git diff` (working tree, sem
    // commit) começa DIRETO em "diff --git", sem newline nenhum antes — bug real
    // encontrado testando (2026-07-26), não só um detalhe de teste sintético.
    let (commit_header, body): (Option<&str>, &str) = if raw.starts_with("diff --git ") {
        (None, raw)
    } else if let Some(pos) = raw.find("\ndiff --git ") {
        (Some(&raw[..pos]), &raw[pos + 1..])
    } else {
        // Sem cabeçalho de diff nenhum — fail-open (regra de negócio 3).
        return raw.to_string();
    };

    let mut out = String::new();
    if let Some(header) = commit_header {
        if let Some(summary) = summarize_commit_header(header) {
            out.push_str(&summary);
            out.push('\n');
        }
    }
    for file_block in split_file_blocks(body) {
        out.push_str(&filter_file_block(file_block));
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Extrai hash/assunto/corpo do cabeçalho de commit de um `git show` e monta
/// uma linha compacta — `None` só se o cabeçalho vier num formato inesperado
/// sem nem a linha "commit <hash>" (fail-open, regra 3).
fn summarize_commit_header(header: &str) -> Option<String> {
    let mut lines = header.lines();
    let commit_line = lines.next()?.trim();
    if !commit_line.starts_with("commit ") {
        return None;
    }

    let mut subject: Option<&str> = None;
    let mut body_lines: Vec<&str> = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Author:") || trimmed.starts_with("Date:") {
            continue;
        }
        if subject.is_none() {
            subject = Some(trimmed);
        } else {
            body_lines.push(trimmed);
        }
    }

    let mut out = match subject {
        Some(s) => format!("{commit_line} — {s}"),
        None => commit_line.to_string(),
    };

    if !body_lines.is_empty() {
        let body = body_lines.join(" ");
        let summary = crate::bornes::prosa::summarize(&body, 1);
        let label = if summary.len() < body.len() { "resumo" } else { "corpo" };
        out.push_str(&format!("\n  {label}: {summary}"));
    }

    Some(out)
}

fn split_file_blocks(body: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut rest = body;
    while let Some(pos) = rest[7..].find("diff --git ") {
        // procura a PRÓXIMA ocorrência depois da atual (pula os 7 chars de "diff --git " já visto)
        let split_at = pos + 7;
        blocks.push(&rest[..split_at]);
        rest = &rest[split_at..];
    }
    blocks.push(rest);
    blocks
}

fn filter_file_block(block: &str) -> String {
    let filename = block
        .lines()
        .next()
        .and_then(|l| l.rsplit(" b/").next())
        .unwrap_or("(arquivo desconhecido)");

    let mut out = format!("{filename}\n");
    let mut changed_count = 0usize;
    let mut truncated = false;

    for line in block.lines().skip(1) {
        if line.starts_with("index ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            continue; // metadata de baixo nível, sem valor informativo
        }
        if line.starts_with("@@") {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if changed_count >= MAX_CHANGED_LINES_PER_FILE
            && (line.starts_with('+') || line.starts_with('-'))
        {
            // Bug real achado ao vivo (2026-07-26): a versão anterior só pulava a
            // linha alterada e CONTINUAVA o loop, deixando hunks seguintes
            // aparecerem parcialmente (cabeçalho "@@" e contexto sem o conteúdo
            // real) — parecia código quebrado, não um corte claro. Agora para de
            // vez no primeiro excesso: corte honesto em vez de saída confusa.
            truncated = true;
            break;
        }
        if line.starts_with('+') || line.starts_with('-') {
            changed_count += 1;
        }
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }

    if truncated {
        out.push_str(&format!(
            "  ... (+{} more changed lines)\n",
            block
                .lines()
                .filter(|l| l.starts_with('+') || l.starts_with('-'))
                .count()
                .saturating_sub(changed_count)
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_diff_no_commit_header() {
        // `git diff` (working tree, sem commit) começa direto em "diff --git" —
        // regressão do bug real achado ao vivo (2026-07-26).
        let input = "diff --git a/src/lib.rs b/src/lib.rs\nindex 111..222 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,2 @@\n-old line\n+new line\n";
        let out = filter(input);
        assert!(out.contains("src/lib.rs"));
        assert!(out.contains("-old line"));
        assert!(out.contains("+new line"));
    }

    #[test]
    fn strips_commit_metadata_keeps_hunk() {
        let input = "commit abc\nAuthor: A\nDate: d\n\n    msg\n\ndiff --git a/src/lib.rs b/src/lib.rs\nindex 111..222 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,2 @@\n-old line\n+new line\n";
        let out = filter(input);
        assert!(!out.contains("Author:"));
        assert!(out.contains("src/lib.rs"));
        assert!(out.contains("@@ -1,2 +1,2 @@"));
        assert!(out.contains("-old line"));
        assert!(out.contains("+new line"));
        // M7: hash+assunto do commit não somem mais por completo (antes o
        // hash em si já era perdido — impossível saber qual commit era esse
        // diff sem rodar outro comando à parte).
        assert!(out.starts_with("commit abc — msg"));
    }

    #[test]
    fn summarizes_long_commit_body_before_hunks() {
        let input = "commit deadbeef\nAuthor: A\nDate: d\n\n    fix: race condition\n\n    First sentence explaining the actual bug in detail. Second sentence adding filler context that matters less.\n\ndiff --git a/src/lib.rs b/src/lib.rs\nindex 1..2 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let out = filter(input);
        assert!(out.starts_with("commit deadbeef — fix: race condition"));
        assert!(out.contains("resumo:")); // corpo com 2 frases foi resumido pra 1
        assert!(out.contains("src/lib.rs"));
    }

    #[test]
    fn caps_large_file_diff() {
        let mut input = String::from("diff --git a/big.rs b/big.rs\nindex 1..2 100644\n--- a/big.rs\n+++ b/big.rs\n@@ -1,20 +1,20 @@\n");
        for i in 0..20 {
            input.push_str(&format!("-old{i}\n+new{i}\n"));
        }
        let out = filter(&input);
        assert!(out.contains("more changed lines"));
        assert!(out.contains("-old0"));
        assert!(!out.contains("old19")); // truncado antes de chegar aqui
    }

    #[test]
    fn no_diff_header_passthrough() {
        let input = "nada pra ver aqui\n";
        assert_eq!(filter(input), input);
    }
}
