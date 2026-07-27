const MAX_CHANGED_LINES_PER_FILE: usize = 10;

/// Layer A — unified diff parser (specs.md §5.4c, "true structural
/// parsing"). Keeps the hunks per file, with a cap on changed lines per file
/// (same technique documented for RTK, specs section 9).
///
/// M7 (specs.md §7.2): when there's a commit header (`git show`, not plain
/// `git diff`), that header is no longer dropped entirely — it used to be
/// that the commit's hash/subject/body all disappeared, even the hash (a
/// real loss: there was no way to tell which commit this diff was without
/// running another command). Now keeps "commit <hash> — <subject>" and
/// summarizes the body via `bornes/prosa` instead of dropping it.
pub fn filter(raw: &str) -> String {
    // `git show` has a commit header before the first "diff --git" (which is
    // why we look for "\ndiff --git" in the middle of the text); `git diff`
    // (working tree, no commit) starts DIRECTLY with "diff --git", no
    // newline before it at all — a real bug found while testing
    // (2026-07-26), not just a synthetic test detail.
    let (commit_header, body): (Option<&str>, &str) = if raw.starts_with("diff --git ") {
        (None, raw)
    } else if let Some(pos) = raw.find("\ndiff --git ") {
        (Some(&raw[..pos]), &raw[pos + 1..])
    } else {
        // No diff header at all — fail-open (business rule 3).
        return raw.to_string();
    };

    let mut out = String::new();
    if let Some(header) = commit_header
        && let Some(summary) = summarize_commit_header(header)
    {
        out.push_str(&summary);
        out.push('\n');
    }
    for file_block in split_file_blocks(body) {
        out.push_str(&filter_file_block(file_block));
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Extracts hash/subject/body from a `git show` commit header and builds a
/// compact line — `None` only if the header comes in an unexpected format
/// without even a "commit <hash>" line (fail-open, rule 3).
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
        let label = if summary.len() < body.len() {
            "summary"
        } else {
            "body"
        };
        out.push_str(&format!("\n  {label}: {summary}"));
    }

    Some(out)
}

fn split_file_blocks(body: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut rest = body;
    while let Some(pos) = rest[7..].find("diff --git ") {
        // looks for the NEXT occurrence after the current one (skips the 7
        // chars of "diff --git " already seen)
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
        .unwrap_or("(unknown file)");

    let mut out = format!("{filename}\n");
    let mut changed_count = 0usize;
    let mut truncated = false;

    for line in block.lines().skip(1) {
        if line.starts_with("index ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            continue; // low-level metadata, no informational value
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
            // Real bug found live (2026-07-26): the earlier version only
            // skipped the changed line and CONTINUED the loop, letting
            // later hunks show up partially (an "@@" header and context
            // with no real content) — looked like broken code, not a clear
            // cutoff. Now it stops for good on the first excess: an honest
            // cut instead of a confusing output.
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
        // `git diff` (working tree, no commit) starts directly with "diff
        // --git" — regression test for the real bug found live (2026-07-26).
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
        // M7: the commit's hash+subject no longer disappear entirely (before,
        // even the hash itself was lost — impossible to tell which commit
        // this diff was without running a separate command).
        assert!(out.starts_with("commit abc — msg"));
    }

    #[test]
    fn summarizes_long_commit_body_before_hunks() {
        let input = "commit deadbeef\nAuthor: A\nDate: d\n\n    fix: race condition\n\n    First sentence explaining the actual bug in detail. Second sentence adding filler context that matters less.\n\ndiff --git a/src/lib.rs b/src/lib.rs\nindex 1..2 100644\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";
        let out = filter(input);
        assert!(out.starts_with("commit deadbeef — fix: race condition"));
        assert!(out.contains("summary:")); // a 2-sentence body was summarized down to 1
        assert!(out.contains("src/lib.rs"));
    }

    #[test]
    fn caps_large_file_diff() {
        let mut input = String::from(
            "diff --git a/big.rs b/big.rs\nindex 1..2 100644\n--- a/big.rs\n+++ b/big.rs\n@@ -1,20 +1,20 @@\n",
        );
        for i in 0..20 {
            input.push_str(&format!("-old{i}\n+new{i}\n"));
        }
        let out = filter(&input);
        assert!(out.contains("more changed lines"));
        assert!(out.contains("-old0"));
        assert!(!out.contains("old19")); // truncated before getting here
    }

    #[test]
    fn no_diff_header_passthrough() {
        let input = "nothing to see here\n";
        assert_eq!(filter(input), input);
    }
}
