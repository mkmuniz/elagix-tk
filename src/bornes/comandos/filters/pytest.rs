/// Camada A — parser de máquina de estados pra `pytest` (specs.md §5.4a).
///
/// Diferença deliberada em relação ao RTK (specs.md §4, regra de negócio 5 corolário,
/// achado direto da nossa auditoria): quando não há teste coletado por erro de
/// import/config, preserva a ÚLTIMA linha de erro real (`E   ...`) em vez de só
/// dizer "no tests collected" sem motivo — mantém quase toda a economia sem
/// sacrificar informação acionável pra debugar.
pub fn filter(raw: &str) -> String {
    if raw.contains("collected 0 items") {
        return filter_collection_failure(raw);
    }
    if let Some(summary) = find_summary_line(raw) {
        return filter_normal_run(raw, summary);
    }
    // Formato não reconhecido — fail-open (regra de negócio 3).
    raw.to_string()
}

fn filter_collection_failure(raw: &str) -> String {
    let error_count = raw
        .lines()
        .find(|l| l.contains("collected 0 items"))
        .and_then(|l| l.split('/').nth(1))
        .map(|s| s.trim())
        .unwrap_or("erros desconhecidos");

    let last_error_line = raw
        .lines()
        .filter(|l| l.trim_start().starts_with("E   "))
        .next_back()
        .map(str::trim)
        .unwrap_or("(motivo não identificado)");

    format!("Pytest: 0 tests collected ({error_count}) — {last_error_line}")
}

fn find_summary_line(raw: &str) -> Option<&str> {
    raw.lines()
        .rev()
        .find(|l| l.contains(" passed") || l.contains(" failed") || l.contains(" error"))
}

fn filter_normal_run<'a>(raw: &'a str, summary: &'a str) -> String {
    let failed_names: Vec<&str> = raw
        .lines()
        .filter(|l| l.starts_with("FAILED "))
        .collect();

    let mut out = String::new();
    out.push_str(summary.trim_matches(|c: char| c == '=' || c.is_whitespace()));
    if !failed_names.is_empty() {
        out.push('\n');
        for name in &failed_names {
            out.push_str(name);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_failure_preserves_reason() {
        let input = "collected 0 items / 4 errors\n\nERRORS\nE   ModuleNotFoundError: No module named 'x'\n";
        let out = filter(input);
        assert!(out.contains("ModuleNotFoundError"));
        assert!(out.contains("4 errors"));
    }

    #[test]
    fn normal_run_with_failure() {
        let input = "FAILED tests/test_a.py::test_one - AssertionError\n===== 1 passed, 1 failed in 0.5s =====\n";
        let out = filter(input);
        assert!(out.contains("1 passed, 1 failed"));
        assert!(out.contains("FAILED tests/test_a.py::test_one"));
    }
}
