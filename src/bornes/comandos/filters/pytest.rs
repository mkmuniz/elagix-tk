/// Layer A — state-machine parser for `pytest` (specs.md §5.4a).
///
/// Deliberate difference from RTK (specs.md §4, business rule 5 corollary,
/// direct finding from our audit): when no test was collected due to an
/// import/config error, preserves the LAST real error line (`E   ...`)
/// instead of just saying "no tests collected" with no reason — keeps almost
/// all the savings without sacrificing information needed to debug.
pub fn filter(raw: &str) -> String {
    if raw.contains("collected 0 items") {
        return filter_collection_failure(raw);
    }
    if let Some(summary) = find_summary_line(raw) {
        return filter_normal_run(raw, summary);
    }
    // Unrecognized format — fail-open (business rule 3).
    raw.to_string()
}

fn filter_collection_failure(raw: &str) -> String {
    let error_count = raw
        .lines()
        .find(|l| l.contains("collected 0 items"))
        .and_then(|l| l.split('/').nth(1))
        .map(|s| s.trim())
        .unwrap_or("unknown errors");

    let last_error_line = raw
        .lines()
        .rfind(|l| l.trim_start().starts_with("E   "))
        .map(str::trim)
        .unwrap_or("(reason not identified)");

    format!("Pytest: 0 tests collected ({error_count}) — {last_error_line}")
}

fn find_summary_line(raw: &str) -> Option<&str> {
    raw.lines()
        .rev()
        .find(|l| l.contains(" passed") || l.contains(" failed") || l.contains(" error"))
}

fn filter_normal_run<'a>(raw: &'a str, summary: &'a str) -> String {
    let failed_names: Vec<&str> = raw.lines().filter(|l| l.starts_with("FAILED ")).collect();

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
