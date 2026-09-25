/// Layer A — parser for `git log` (specs.md §5.4b). One line per commit
/// (`<short hash> <date> <author> — <subject>`), so the agent sees every
/// commit it asked for instead of only the first one.
///
/// History: until 2026-09-24 this kept only the first commit and dropped the
/// rest (the RTK technique) — maximum savings, but `git log -5` answered with
/// one commit, forcing the agent to ask again. Now every commit keeps its
/// identity; what gets dropped is the per-commit noise (email, weekday,
/// timezone, message body).
///
/// M7 (specs.md §7.2): the FIRST commit's body still goes through
/// `bornes/prosa` — summarized down to 1 sentence, labeled "summary" when it
/// actually shrank (otherwise "body", shown in full). Other commits' bodies
/// and anything that isn't part of the message (`--stat`, `-p` patches) are
/// counted as omitted, which also triggers the `elagix show` recovery hint.
pub fn filter(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    let mut commit_starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("commit "))
        .map(|(i, _)| i)
        .collect();

    // Custom formats (`--oneline`, `--format=...`) have no "commit " header
    // and are already compact — fail-open, pass them through.
    if commit_starts.is_empty() {
        return raw.to_string();
    }
    commit_starts.push(lines.len());

    let mut out = String::new();
    let mut omitted = commit_starts[0]; // anything before the first commit
    for (n, window) in commit_starts.windows(2).enumerate() {
        let block = &lines[window[0]..window[1]];
        let commit = parse_commit(block);
        out.push_str(&commit.one_line());
        out.push('\n');

        if n == 0 && !commit.body.is_empty() {
            let (label, text) = crate::bornes::prosa::commit_body_line(&commit.body);
            out.push_str(&format!("  {label}: {text}\n"));
        } else {
            omitted += commit.body.len();
        }
        omitted += commit.other_lines;
    }

    if omitted > 0 {
        out.push_str(&format!("[+{omitted} lines omitted]"));
    }
    out.trim_end().to_string()
}

struct Commit<'a> {
    hash: &'a str,
    refs: Option<&'a str>,
    author: Option<&'a str>,
    date: Option<String>,
    subject: Option<&'a str>,
    body: Vec<&'a str>,
    /// Lines that aren't header or message: `--stat`, `-p` patches, etc.
    other_lines: usize,
}

impl Commit<'_> {
    fn one_line(&self) -> String {
        let short = &self.hash[..self.hash.len().min(10)];
        let mut line = short.to_string();
        for part in [self.date.as_deref(), self.author].into_iter().flatten() {
            line.push(' ');
            line.push_str(part);
        }
        if let Some(subject) = self.subject {
            line.push_str(" — ");
            line.push_str(subject);
        }
        if let Some(refs) = self.refs {
            line.push(' ');
            line.push_str(refs);
        }
        line
    }
}

fn parse_commit<'a>(block: &[&'a str]) -> Commit<'a> {
    // "commit <sha>" or "commit <sha> (HEAD -> main, origin/main)"
    let header = block[0]["commit ".len()..].trim();
    let (hash, refs) = match header.split_once(' ') {
        Some((h, r)) => (h, Some(r.trim())),
        None => (header, None),
    };
    let mut commit = Commit {
        hash,
        refs,
        author: None,
        date: None,
        subject: None,
        body: Vec::new(),
        other_lines: 0,
    };
    for line in &block[1..] {
        if let Some(a) = line.strip_prefix("Author:") {
            // Name only — the email rarely matters to the agent.
            let a = a.trim();
            commit.author = Some(a.split(" <").next().unwrap_or(a).trim());
        } else if let Some(d) = line.strip_prefix("Date:") {
            commit.date = Some(compact_date(d.trim()));
        } else if line.starts_with("Merge:") || line.trim().is_empty() {
            continue;
        } else if let Some(msg) = line.strip_prefix("    ") {
            // git indents every message line by 4 spaces.
            if commit.subject.is_none() {
                commit.subject = Some(msg.trim());
            } else {
                commit.body.push(msg.trim());
            }
        } else {
            commit.other_lines += 1;
        }
    }
    commit
}

/// `Sat Jul 25 22:42:21 2026 -0300` -> `2026-07-25 22:42`. Anything in
/// another format (`--date=iso`, `--date=relative`...) is kept verbatim —
/// reformat only what's recognized, never guess (business rule 5).
fn compact_date(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let parts: Vec<&str> = date.split_whitespace().collect();
    if let [_weekday, month, day, time, year, ..] = parts[..]
        && let Some(m) = MONTHS.iter().position(|&x| x == month)
        && let Ok(d) = day.parse::<u8>()
        && time.len() == 8
        && year.len() == 4
    {
        return format!("{year}-{:02}-{d:02} {}", m + 1, &time[..5]);
    }
    date.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_commit_passthrough_with_marker() {
        let input =
            "commit abc123\nAuthor: A <a@b.com>\nDate:   today\n\n    fix: bug\n\n    body line\n";
        let out = filter(input);
        assert!(out.starts_with("abc123 today A — fix: bug"));
        // M7: a single-sentence body doesn't shrink (nothing to summarize) —
        // shown in full and labeled "body", not dropped like before M7.
        assert!(out.contains("body: body line"));
    }

    #[test]
    fn every_commit_gets_one_line() {
        let input = "commit aaa\nAuthor: A\nDate: d1\n\n    first\n\ncommit bbb\nAuthor: B\nDate: d2\n\n    second\n\n    second body\n";
        let out = filter(input);
        assert_eq!(
            out,
            "aaa d1 A — first\nbbb d2 B — second\n[+1 lines omitted]"
        );
    }

    #[test]
    fn refs_date_and_patch_lines() {
        let input = "commit 0123456789abcdef (HEAD -> main)\nAuthor: Ana <a@b.c>\nDate:   Sat Jul 25 22:42:21 2026 -0300\n\n    fix: x\n\ndiff --git a/f b/f\n+new\n";
        let out = filter(input);
        assert_eq!(
            out,
            "0123456789 2026-07-25 22:42 Ana — fix: x (HEAD -> main)\n[+2 lines omitted]"
        );
    }

    #[test]
    fn oneline_format_passes_through() {
        let input = "abc123 fix: x\ndef456 feat: y\n";
        assert_eq!(filter(input), input);
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
        assert!(out.starts_with(
            "cb93a3bd72 2026-07-25 22:42 Mkmuniz — chore: cargo fmt (fix CI fmt-check failure)"
        ));
        // M7: a 2-sentence body is now summarized down to 1 (bornes/prosa)
        // instead of being dropped entirely — only one of the two original
        // sentences survives.
        assert!(out.contains("summary:"));
        assert!(!(out.contains("Never ran cargo fmt") && out.contains("Purely mechanical")));
        // Every one of the 5 commits keeps its own line now.
        assert!(out.contains("e1fe7741ff 2026-07-25 21:27 Mkmuniz — feat(committee):"));
        assert_eq!(out.lines().filter(|l| l.contains(" Mkmuniz — ")).count(), 5);
        assert!(out.contains("lines omitted]"));
        assert!(out.len() * 5 < input.len()); // still well over 80% smaller
    }
}
