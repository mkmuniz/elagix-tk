const CAP_LIST: usize = 20;

/// Camada A — parser de verdade pra `git status` (specs.md §5.4a).
/// Recebe a saída de `git status --porcelain=v1 --branch` (o shim sempre roda com
/// essas flags internamente, independente do que o usuário digitou, pra ter um
/// formato confiável de parsear — não depende de locale/largura de coluna).
pub fn filter(porcelain_output: &str) -> String {
    let mut lines = porcelain_output.lines();
    let branch_line = lines.next().unwrap_or("## ?");
    let branch_info = parse_branch_line(branch_line);

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    for line in lines {
        if line.len() < 3 {
            continue;
        }
        let (x, y) = (line.as_bytes()[0] as char, line.as_bytes()[1] as char);
        let file = &line[3..];
        if x == '?' && y == '?' {
            untracked.push(file);
        } else {
            if x != ' ' {
                staged.push(file);
            }
            if y != ' ' && y != '?' {
                unstaged.push(file);
            }
        }
    }

    if staged.is_empty() && unstaged.is_empty() && untracked.is_empty() {
        // Regra de negócio 5 corolário: mensagem só dispara quando de fato não há nada —
        // nunca inventa "limpo" se a leitura falhou (isso seria coberto pelo fail-open, não aqui).
        // Nome do branch de propósito FORA da mensagem: branches longos (comuns em fluxo
        // de feature branch) fariam essa mensagem ficar maior que a saída porcelain crua,
        // disparando a regra de negócio 6 (nunca piorar) e devolvendo o original sem filtro —
        // vimos isso acontecer de verdade testando contra um branch real desta sessão.
        return "clean — nothing to commit".to_string();
    }

    let mut out = String::new();
    out.push_str(&branch_info);
    out.push('\n');
    push_section(&mut out, "staged", &staged);
    push_section(&mut out, "unstaged", &unstaged);
    push_section(&mut out, "untracked", &untracked);
    out.trim_end().to_string()
}

fn push_section(out: &mut String, label: &str, files: &[&str]) {
    if files.is_empty() {
        return;
    }
    out.push_str(&format!("{label} ({}):\n", files.len()));
    for f in files.iter().take(CAP_LIST) {
        out.push_str("  ");
        out.push_str(f);
        out.push('\n');
    }
    if files.len() > CAP_LIST {
        out.push_str(&format!("  ... (+{} more)\n", files.len() - CAP_LIST));
    }
}

fn parse_branch_line(line: &str) -> String {
    // formato: "## branch...upstream [ahead N, behind M]" ou "## branch" ou "## HEAD (no branch)"
    line.trim_start_matches("## ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_tree() {
        let input = "## main...origin/main\n";
        assert!(filter(input).starts_with("clean — nothing to commit"));
    }

    #[test]
    fn staged_and_untracked() {
        let input = "## main...origin/main\nM  src/lib.rs\n?? new_file.txt\n";
        let out = filter(input);
        assert!(out.contains("staged (1)"));
        assert!(out.contains("src/lib.rs"));
        assert!(out.contains("untracked (1)"));
        assert!(out.contains("new_file.txt"));
    }
}
