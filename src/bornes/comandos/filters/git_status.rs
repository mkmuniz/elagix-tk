const CAP_LIST: usize = 20;

/// Layer A — a real parser for `git status` (specs.md §5.4a).
/// Receives the output of `git status --porcelain=v1 --branch` (the shim
/// always runs with these flags internally, regardless of what the user
/// typed, to get a reliable format to parse — no dependency on locale or
/// column width).
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
        // Business rule 5 corollary: this message only fires when there's
        // genuinely nothing — never invents "clean" if reading failed (that's
        // covered by fail-open, not here). Branch name deliberately left OUT
        // of the message: long branch names (common in feature-branch
        // workflows) would make this message bigger than the raw porcelain
        // output, triggering business rule 6 (never make it worse) and
        // falling back to the unfiltered original — we saw this happen for
        // real testing against an actual branch this session.
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
    // format: "## branch...upstream [ahead N, behind M]" or "## branch" or "## HEAD (no branch)"
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
