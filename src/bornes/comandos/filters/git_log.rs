/// Camada A — parser pra `git log` (specs.md §5.4b, "truncamento estrutural com corte duro").
/// Mantém o primeiro commit quase completo (hash, autor, data, primeira linha da
/// mensagem) e descarta o resto, substituindo por uma contagem de linhas omitidas.
///
/// M7 (specs.md §7.2): o corpo da mensagem do primeiro commit, que antes era
/// descartado por completo (igual o RTK), agora passa por `bornes/prosa` — resume
/// em 1 frase em vez de apagar, só rotulado "resumo" quando de fato encolheu
/// (senão seria uma frase só, mostrada por completo e rotulada "corpo").
pub fn filter(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    let mut commit_starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("commit "))
        .map(|(i, _)| i)
        .collect();

    if commit_starts.is_empty() {
        return raw.to_string();
    }
    commit_starts.push(lines.len());

    let first_end = commit_starts[1];
    let first_block = &lines[commit_starts[0]..first_end];

    let mut out = String::new();
    out.push_str(first_block[0]); // "commit <hash>"
    out.push('\n');

    let mut subject_found = false;
    let mut body_lines: Vec<&str> = Vec::new();
    for line in &first_block[1..] {
        let trimmed = line.trim_start();
        if trimmed.starts_with("Author:") || trimmed.starts_with("Date:") {
            out.push_str("  ");
            out.push_str(trimmed);
            out.push('\n');
        } else if !subject_found && !trimmed.is_empty() {
            out.push_str("  ");
            out.push_str(trimmed);
            out.push('\n');
            subject_found = true;
        } else if subject_found && !trimmed.is_empty() {
            body_lines.push(trimmed);
        }
    }

    if !body_lines.is_empty() {
        let body = body_lines.join(" ");
        let summary = crate::bornes::prosa::summarize(&body, 1);
        let label = if summary.len() < body.len() { "resumo" } else { "corpo" };
        out.push_str(&format!("  {label}: {summary}\n"));
    }

    let remaining_lines = lines.len() - first_end;
    if remaining_lines > 0 {
        out.push_str(&format!("  [+{remaining_lines} lines omitted]"));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_commit_passthrough_with_marker() {
        let input = "commit abc123\nAuthor: A <a@b.com>\nDate:   today\n\n    fix: bug\n\n    body line\n";
        let out = filter(input);
        assert!(out.contains("commit abc123"));
        assert!(out.contains("fix: bug"));
        // M7: corpo de 1 frase só não encolhe (nada pra resumir) — mostrado por
        // completo e rotulado "corpo", não descartado como antes do M7.
        assert!(out.contains("corpo: body line"));
    }

    #[test]
    fn multiple_commits_truncated() {
        let input = "commit aaa\nAuthor: A\nDate: d1\n\n    first\n\ncommit bbb\nAuthor: B\nDate: d2\n\n    second\n";
        let out = filter(input);
        assert!(out.contains("commit aaa"));
        assert!(out.contains("first"));
        assert!(!out.contains("second"));
        assert!(out.contains("omitted"));
    }

    /// Fixture real: `git log -5` capturado do repositório bastion-agent nesta sessão
    /// (2026-07-26) — 5 commits reais, um deles com corpo de mensagem longo (parágrafo
    /// inteiro). Serve de regressão pro bug de recursão do shim que achamos testando
    /// isso ao vivo (não era bug do parser — era do `resolve_real_binary`, já corrigido).
    #[test]
    fn real_fixture_five_commits() {
        let input = include_str!("test_fixture_gitlog.txt");
        let commit_count = input.lines().filter(|l| l.starts_with("commit ")).count();
        assert_eq!(commit_count, 5);

        let out = filter(input);
        assert!(out.starts_with("commit cb93a3bd721a85b25c113413c8ed93b099bcc7f8"));
        assert!(out.contains("chore: cargo fmt (fix CI fmt-check failure)"));
        // M7: corpo de 2 frases agora é resumido em 1 (bornes/prosa) em vez de
        // descartado por completo — só uma das duas frases originais sobrevive.
        assert!(out.contains("resumo:"));
        assert!(!(out.contains("Never ran cargo fmt") && out.contains("Purely mechanical")));
        assert!(!out.contains("commit e1fe7741")); // segundo commit não aparece
        assert!(out.contains("[+154 lines omitted]"));
    }
}
