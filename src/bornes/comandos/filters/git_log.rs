/// Layer A — parser for `git log` (specs.md §5.4b, "structural truncation
/// with a hard cutoff"). Keeps the first commit almost complete (hash,
/// author, date, first line of the message) and drops the rest, replacing
/// it with a count of omitted lines.
///
/// M7 (specs.md §7.2): the first commit's message body, which used to be
/// dropped entirely (like RTK), now goes through `bornes/prosa` — summarized
/// down to 1 sentence instead of erased, only labeled "summary" when it
/// actually shrank (otherwise it was a single sentence already, shown in
/// full and labeled "body").
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
        let label = if summary.len() < body.len() {
            "summary"
        } else {
            "body"
        };
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
        let input =
            "commit abc123\nAuthor: A <a@b.com>\nDate:   today\n\n    fix: bug\n\n    body line\n";
        let out = filter(input);
        assert!(out.contains("commit abc123"));
        assert!(out.contains("fix: bug"));
        // M7: a single-sentence body doesn't shrink (nothing to summarize) —
        // shown in full and labeled "body", not dropped like before M7.
        assert!(out.contains("body: body line"));
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

    /// Real fixture: `git log -5` captured from the bastion-agent repository
    /// during this session (2026-07-26) — 5 real commits, one of them with a
    /// long message body (a whole paragraph). Serves as a regression test
    /// for the shim recursion bug found testing this live (it wasn't a
    /// parser bug — it was in `resolve_real_binary`, already fixed).
    #[test]
    fn real_fixture_five_commits() {
        let input = include_str!("test_fixture_gitlog.txt");
        let commit_count = input.lines().filter(|l| l.starts_with("commit ")).count();
        assert_eq!(commit_count, 5);

        let out = filter(input);
        assert!(out.starts_with("commit cb93a3bd721a85b25c113413c8ed93b099bcc7f8"));
        assert!(out.contains("chore: cargo fmt (fix CI fmt-check failure)"));
        // M7: a 2-sentence body is now summarized down to 1 (bornes/prosa)
        // instead of being dropped entirely — only one of the two original
        // sentences survives.
        assert!(out.contains("summary:"));
        assert!(!(out.contains("Never ran cargo fmt") && out.contains("Purely mechanical")));
        assert!(!out.contains("commit e1fe7741")); // second commit doesn't show up
        assert!(out.contains("[+154 lines omitted]"));
    }
}
