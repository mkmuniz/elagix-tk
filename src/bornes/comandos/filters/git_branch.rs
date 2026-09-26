/// Layer A — `git branch -a` / `git branch -r`. Remote-tracking branches
/// repeat the `remotes/<remote>/` prefix on every line, and most of them
/// mirror a local branch that's already listed. Output: local branches as
/// git prints them, then per remote only the branches that exist *just*
/// there, prefix stripped and comma-separated — plus how many mirror a
/// local one. Long remote lists are capped with a recoverable
/// `[+N lines omitted: ...]` marker (same spirit as RTK's `remote-only (N)`
/// grouping, without dropping the count of what was hidden).
///
/// Anything that isn't a plain branch list (e.g. `-v`/`-vv` columns) is
/// passed through untouched (fail-open, business rule 3).
/// Same order of magnitude as RTK (10): enough to see naming patterns, the
/// rest counted and recoverable.
const MAX_REMOTE_NAMES: usize = 12;

pub fn filter(raw: &str) -> String {
    let mut locals: Vec<&str> = Vec::new();
    // remote -> branch names (without the prefix), in git's order
    let mut remotes: Vec<(&str, Vec<&str>)> = Vec::new();
    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let name = line.get(2..).unwrap_or("").trim();
        if name.is_empty() {
            return raw.to_string();
        }
        if let Some(rest) = name.strip_prefix("remotes/") {
            if rest.contains(" -> ") {
                continue; // remotes/origin/HEAD -> origin/main
            }
            if rest.contains(' ') {
                return raw.to_string();
            }
            let Some((remote, branch)) = rest.split_once('/') else {
                return raw.to_string();
            };
            match remotes.iter_mut().find(|(r, _)| *r == remote) {
                Some((_, list)) => list.push(branch),
                None => remotes.push((remote, vec![branch])),
            }
        } else {
            // Local branch lines are "* name" / "  name" / "+ name" (worktree).
            if name.contains(' ') && !name.starts_with('(') {
                return raw.to_string(); // -v / -vv columns
            }
            locals.push(line);
        }
    }
    if remotes.is_empty() {
        return raw.to_string(); // plain `git branch`: already compact
    }

    let local_names: Vec<&str> = locals
        .iter()
        .map(|l| l.get(2..).unwrap_or("").trim())
        .collect();
    let mut out: Vec<String> = locals.iter().map(|l| l.to_string()).collect();
    for (remote, branches) in &remotes {
        let only: Vec<&str> = branches
            .iter()
            .copied()
            .filter(|b| !local_names.contains(b))
            .collect();
        let mirrored = branches.len() - only.len();
        out.push(format!(
            "{remote}: {} remote-only{}",
            only.len(),
            if mirrored > 0 {
                format!(" (+{mirrored} also local)")
            } else {
                String::new()
            }
        ));
        for chunk in only.chunks(6).take(MAX_REMOTE_NAMES / 6) {
            out.push(format!("  {}", chunk.join(", ")));
        }
        if only.len() > MAX_REMOTE_NAMES {
            out.push(format!(
                "[+{} lines omitted: more {remote} branches]",
                only.len() - MAX_REMOTE_NAMES
            ));
        }
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_remotes_and_skips_mirrors() {
        let raw = "* main\n  feat/a\n  remotes/origin/HEAD -> origin/main\n  remotes/origin/main\n  remotes/origin/feat/a\n  remotes/origin/feat/b\n  remotes/upstream/main\n";
        assert_eq!(
            filter(raw),
            "* main\n  feat/a\norigin: 1 remote-only (+2 also local)\n  feat/b\nupstream: 0 remote-only (+1 also local)"
        );
    }

    #[test]
    fn long_remote_lists_are_capped_with_a_marker() {
        let mut raw = String::from("* main\n");
        for i in 0..100 {
            raw.push_str(&format!("  remotes/origin/feature-{i}\n"));
        }
        let out = filter(&raw);
        assert!(out.contains("origin: 100 remote-only"));
        assert!(out.contains("feature-11") && !out.contains("feature-12"));
        assert!(out.ends_with("[+88 lines omitted: more origin branches]"));
    }

    #[test]
    fn verbose_and_plain_lists_pass_through() {
        let plain = "* main\n  dev\n";
        assert_eq!(filter(plain), plain);
        let verbose = "* main abc1234 [origin/main] msg\n  remotes/origin/main abc1234 msg\n";
        assert_eq!(filter(verbose), verbose);
    }
}
