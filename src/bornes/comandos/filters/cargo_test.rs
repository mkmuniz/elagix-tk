/// Layer A — parser for `cargo test` (specs.md §5.4a, pattern-recognition
/// short-circuit). Extracts the final summary line ("test result: ...") and,
/// if there are failures, the short list of names — drops the verbose
/// compilation output and the full stdout/panic of each failed test.
pub fn filter(raw: &str) -> String {
    let Some(summary) = find_summary_line(raw) else {
        // Unrecognized format (e.g. a compile error before tests ran) —
        // fail-open (business rule 3), passes the original output through unfiltered.
        return raw.to_string();
    };

    let failed_names: Vec<&str> = extract_failed_names(raw);

    let mut out = String::new();
    out.push_str(summary.trim());
    if !failed_names.is_empty() {
        out.push('\n');
        out.push_str("failures:\n");
        for name in &failed_names {
            out.push_str("  ");
            out.push_str(name);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

fn find_summary_line(raw: &str) -> Option<&str> {
    raw.lines().rev().find(|l| l.starts_with("test result:"))
}

fn extract_failed_names(raw: &str) -> Vec<&str> {
    // cargo prints a "failures:\n    name1\n    name2\n\ntest result: ..."
    // section right before the summary — grab just that short list, not the
    // full panics above it.
    let Some(section_start) = raw.rfind("\nfailures:\n") else {
        return Vec::new();
    };
    let after = &raw[section_start + "\nfailures:\n".len()..];
    after
        .lines()
        .take_while(|l| !l.is_empty())
        .map(str::trim)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_passed() {
        let input = "running 3 tests\ntest a ... ok\ntest b ... ok\ntest c ... ok\n\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\n";
        let out = filter(input);
        assert!(out.contains("3 passed; 0 failed"));
        assert!(!out.contains("failures:"));
    }

    #[test]
    fn with_failures_lists_names_only() {
        let input = "running 2 tests\ntest a ... ok\ntest b ... FAILED\n\nfailures:\n\n---- b stdout ----\nthread 'b' panicked at src/lib.rs:10:5:\nassertion failed\n\nfailures:\n    b\n\ntest result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\n";
        let out = filter(input);
        assert!(out.contains("1 passed; 1 failed"));
        assert!(out.contains("failures:"));
        assert!(out.contains("  b"));
        assert!(!out.contains("panicked")); // panic stack trace dropped
    }

    #[test]
    fn unrecognized_format_passthrough() {
        let input = "error[E0433]: failed to resolve\n";
        assert_eq!(filter(input), input);
    }
}
