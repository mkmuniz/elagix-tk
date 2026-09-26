/// Layer A — parser for `cargo test` (specs.md §5.4a, pattern-recognition
/// short-circuit). Drops the per-test "test x ... ok" lines and keeps what
/// matters:
///
/// - **every suite passed** → one line with the totals across all suites
///   (`cargo test: 89 passed; 0 failed (2 suites, 17.71s)`), the same idea
///   as RTK's summary;
/// - **something failed** → each suite's own result line, then every failing
///   test with where it panicked and the message, so the agent can fix it
///   without re-running.
///
/// Real bug fixed 2026-09-26: the first version only read the LAST
/// "test result:" line. A crate has one per suite (unit tests, each
/// integration test file, doc-tests), so a failure in an early suite
/// followed by a passing last suite read as `test result: ok` — a success
/// shortcut on a failed run (business rule 2), with only the exit code
/// telling the truth.
pub fn filter(raw: &str) -> String {
    let suites: Vec<Suite> = raw.lines().filter_map(Suite::parse).collect();
    if suites.is_empty() {
        // Unrecognized format (e.g. a compile error before tests ran) —
        // fail-open (business rule 3), passes the original output through unfiltered.
        return raw.to_string();
    }

    let all_ok = suites.iter().all(|s| s.ok && s.failed == 0);
    if all_ok {
        return totals_line(&suites);
    }

    let mut out = String::new();
    for s in &suites {
        out.push_str(s.line.trim());
        out.push('\n');
    }
    let failures = failure_details(raw);
    if !failures.is_empty() {
        out.push_str("failures:\n");
        for (name, detail) in failures {
            out.push_str("  ");
            out.push_str(name);
            out.push('\n');
            for line in detail {
                out.push_str("    ");
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out.push_str(&totals_line(&suites));
    out
}

struct Suite<'a> {
    line: &'a str,
    ok: bool,
    passed: u64,
    failed: u64,
    ignored: u64,
    filtered: u64,
    secs: f64,
}

impl<'a> Suite<'a> {
    /// `test result: ok. 73 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.90s`
    fn parse(line: &'a str) -> Option<Self> {
        let rest = line.trim().strip_prefix("test result: ")?;
        let (status, counts) = rest.split_once(". ")?;
        let num = |label: &str| -> u64 {
            counts
                .split(';')
                .find_map(|part| {
                    let part = part.trim();
                    part.strip_suffix(label).and_then(|n| n.trim().parse().ok())
                })
                .unwrap_or(0)
        };
        let secs = counts
            .rsplit("finished in ")
            .next()
            .and_then(|s| s.trim().trim_end_matches('s').parse().ok())
            .unwrap_or(0.0);
        Some(Suite {
            line,
            ok: status == "ok",
            passed: num("passed"),
            failed: num("failed"),
            ignored: num("ignored"),
            filtered: num("filtered out"),
            secs,
        })
    }
}

fn totals_line(suites: &[Suite]) -> String {
    let sum = |f: fn(&Suite) -> u64| suites.iter().map(f).sum::<u64>();
    let mut line = format!(
        "cargo test: {} passed; {} failed",
        sum(|s| s.passed),
        sum(|s| s.failed)
    );
    for (n, label) in [
        (sum(|s| s.ignored), "ignored"),
        (sum(|s| s.filtered), "filtered out"),
    ] {
        if n > 0 {
            line.push_str(&format!("; {n} {label}"));
        }
    }
    let secs: f64 = suites.iter().map(|s| s.secs).sum();
    let plural = if suites.len() == 1 { "" } else { "s" };
    line.push_str(&format!(" ({} suite{plural}, {secs:.2}s)", suites.len()));
    line
}

/// For each failing test: its name plus up to two lines of its panic
/// (location and message), taken from the `---- name stdout ----` blocks.
fn failure_details(raw: &str) -> Vec<(&str, Vec<&str>)> {
    let mut out: Vec<(&str, Vec<&str>)> = Vec::new();
    let lines: Vec<&str> = raw.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let Some(name) = line
            .strip_prefix("---- ")
            .and_then(|l| l.strip_suffix(" stdout ----"))
        else {
            continue;
        };
        let detail: Vec<&str> = lines[i + 1..]
            .iter()
            .skip_while(|l| l.trim().is_empty())
            .take_while(|l| !l.starts_with("---- ") && !l.trim().is_empty())
            .filter(|l| !l.starts_with("note: run with `RUST_BACKTRACE"))
            .take(2)
            .map(|l| l.trim())
            .collect();
        out.push((name, detail));
    }
    if out.is_empty() {
        // No stdout blocks (e.g. `--nocapture`): fall back to the plain name lists.
        let mut in_list = false;
        for line in &lines {
            if *line == "failures:" {
                in_list = true;
                continue;
            }
            if in_list {
                match line.strip_prefix("    ") {
                    Some(name) if !name.starts_with("----") => out.push((name.trim(), Vec::new())),
                    _ => in_list = false,
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_passed_single_suite() {
        let input = "running 3 tests\ntest a ... ok\ntest b ... ok\ntest c ... ok\n\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\n";
        assert_eq!(
            filter(input),
            "cargo test: 3 passed; 0 failed (1 suite, 0.01s)"
        );
    }

    #[test]
    fn all_suites_are_summed() {
        let input = "running 73 tests\n\ntest result: ok. 73 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 2.90s\n\nrunning 16 tests\n\ntest result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 14.81s\n";
        assert_eq!(
            filter(input),
            "cargo test: 89 passed; 0 failed; 1 ignored (2 suites, 17.71s)"
        );
    }

    /// The bug: an early suite failed, the last one passed.
    #[test]
    fn failure_in_an_early_suite_is_never_reported_as_ok() {
        let input = "running 2 tests\ntest a ... ok\ntest b ... FAILED\n\nfailures:\n\n---- b stdout ----\n\nthread 'b' panicked at src/lib.rs:10:5:\nassertion `left == right` failed\nnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n\nfailures:\n    b\n\ntest result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\nrunning 16 tests\n\ntest result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s\n";
        let out = filter(input);
        assert!(
            out.contains("test result: FAILED. 1 passed; 1 failed"),
            "{out}"
        );
        assert!(out.contains("  b\n    thread 'b' panicked at src/lib.rs:10:5:\n    assertion `left == right` failed"), "{out}");
        assert!(
            out.ends_with("cargo test: 17 passed; 1 failed (2 suites, 1.01s)"),
            "{out}"
        );
        assert!(!out.contains("RUST_BACKTRACE"));
    }

    #[test]
    fn unrecognized_format_passthrough() {
        let input = "error[E0433]: failed to resolve\n";
        assert_eq!(filter(input), input);
    }
}
