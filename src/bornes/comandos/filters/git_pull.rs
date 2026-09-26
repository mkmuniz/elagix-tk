/// Layer A — `git pull` / `git merge`. The diffstat lists one line per
/// changed file, plus one "create mode" line per new file; what the agent
/// needs is the outcome. A clean fast-forward or merge becomes one line
/// (the idea behind RTK's `ok 26 files +325 -0`, but keeping the commit
/// range and the kind of merge):
///
/// `Fast-forward 2e42548..137537e: 26 files changed, 325 insertions(+)`
///
/// followed by a `[+N lines omitted: ...]` marker, so the full per-file list
/// stays recoverable via `schliffe show`. Anything else — conflicts,
/// errors, "Already up to date.", unknown lines — is kept verbatim; only
/// the per-file rows are ever collapsed.
pub fn filter(raw: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut omitted = 0;
    for line in raw.lines() {
        if is_per_file_row(line) {
            omitted += 1;
        } else if !line.trim().is_empty() {
            kept.push(line);
        }
    }
    if omitted == 0 {
        return raw.to_string();
    }

    let marker = format!("[+{omitted} lines omitted: per-file changes]");
    // The clean cases, collapsed into a single line.
    match kept.as_slice() {
        [updating, "Fast-forward", summary] if is_summary(summary) => {
            let range = updating.strip_prefix("Updating ").unwrap_or(updating);
            format!("Fast-forward {range}:{}\n{marker}", summary_tail(summary))
        }
        [merge, summary] if merge.starts_with("Merge made by") && is_summary(summary) => {
            format!(
                "{}:{}\n{marker}",
                merge.trim_end_matches('.'),
                summary_tail(summary)
            )
        }
        _ => format!("{}\n{marker}", kept.join("\n")),
    }
}

/// ` src/a.ts | 12 ++--`, ` logo.png | Bin 0 -> 2 bytes`,
/// ` create mode 100644 src/a.ts`, ` rename a => b (90%)`, ` mode change ...`.
fn is_per_file_row(line: &str) -> bool {
    let Some(rest) = line.strip_prefix(' ') else {
        return false;
    };
    if rest.starts_with("create mode ")
        || rest.starts_with("delete mode ")
        || rest.starts_with("mode change ")
        || (rest.starts_with("rename ") && rest.ends_with("%)"))
    {
        return true;
    }
    match rest.rsplit_once(" | ") {
        Some((_, stat)) => {
            let stat = stat.trim();
            stat.starts_with("Bin ")
                || stat.split_once(' ').map_or(
                    stat.chars().all(|c| c.is_ascii_digit()),
                    |(n, bar)| {
                        n.chars().all(|c| c.is_ascii_digit())
                            && bar.chars().all(|c| c == '+' || c == '-')
                    },
                )
        }
        None => false,
    }
}

/// ` 26 files changed, 325 insertions(+), 3 deletions(-)`
fn is_summary(line: &str) -> bool {
    let l = line.trim();
    l.contains(" changed") && (l.contains("file changed") || l.contains("files changed"))
}

fn summary_tail(summary: &str) -> String {
    format!(" {}", summary.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_fast_forward_becomes_one_line() {
        let raw = include_str!("../filters-toml/fixtures/git-pull.stdout.txt");
        let out = filter(raw);
        assert_eq!(
            out,
            "Fast-forward 2e42548..137537e: 26 files changed, 325 insertions(+)\n[+52 lines omitted: per-file changes]"
        );
    }

    #[test]
    fn merge_commit_becomes_one_line() {
        let raw = "Merge made by the 'ort' strategy.\n src/a.rs | 3 ++-\n 1 file changed, 2 insertions(+), 1 deletion(-)\n";
        assert_eq!(
            filter(raw),
            "Merge made by the 'ort' strategy: 1 file changed, 2 insertions(+), 1 deletion(-)\n[+1 lines omitted: per-file changes]"
        );
    }

    #[test]
    fn conflicts_and_up_to_date_are_kept() {
        assert_eq!(filter("Already up to date.\n"), "Already up to date.\n");
        let conflict = "Auto-merging src/a.rs\nCONFLICT (content): Merge conflict in src/a.rs\nAutomatic merge failed; fix conflicts and then commit the result.\n";
        assert_eq!(filter(conflict), conflict);
    }
}
