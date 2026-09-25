mod camada_b;
mod filters;
mod shim;

use crate::core::store;
use std::process::ExitCode;

/// Minimum size for dedup to kick in (specs §8.3) — below this, the
/// reference line would cost more than it saves.
const DEDUP_MIN_BYTES: usize = 200;

/// `bornes/comandos` (specs.md §5) — `$PATH` shim: intercepts the output of
/// a shell command (`git`, `docker`, `cargo`, `pytest`...) invoked as
/// `invoked_name` (the process's `argv[0]`, resolved by `main.rs`).
pub fn run(invoked_name: &str, rest_args: &[String]) -> ExitCode {
    let Some(real_bin) = shim::resolve_real_binary(invoked_name) else {
        // Same message and exit code (127) a shell gives for a missing
        // command, so scripts probing for the tool behave as without Elagix.
        eprintln!("{invoked_name}: command not found (elagix shim: no real binary in PATH)");
        return ExitCode::from(127);
    };

    // Total passthrough for interactive human use — never filters when it's a
    // TTY (specs.md §5.1), nor when no AI agent is calling (see
    // `shim::agent_active`). On Unix this replaces the current process (a real exec).
    if shim::stdout_is_tty() || !shim::agent_active() {
        if let Err(e) = shim::exec_passthrough(&real_bin, rest_args) {
            eprintln!("elagix: failed to run {invoked_name}: {e}");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS; // unreachable on Unix (exec replaces the process)
    }

    // Cache (specs.md §8.2, v1 scope decided in §13): only the one case
    // provably immutable — `git show <explicit sha>`. `HEAD`/branch are
    // excluded because they can point to a different commit tomorrow. The
    // key carries a format version ("v1") so it never serves stale output if
    // the `git_diff` filter changes in the future.
    let git_show_cache_key = if invoked_name == "git"
        && rest_args.len() == 2
        && rest_args[0] == "show"
        && looks_like_git_sha(&rest_args[1])
    {
        Some(format!("git-show:v1:{}", rest_args[1]))
    } else {
        None
    };
    // (getting here already implies the non-interactive path — the TTY
    // branch above always returns/replaces the process before this line.)
    if let Some(key) = &git_show_cache_key
        && let Some(cached) = store::get_keyed(key)
    {
        print!("{cached}");
        if !cached.ends_with('\n') {
            println!();
        }
        return ExitCode::SUCCESS;
    }

    // Non-interactive path (pipe) — this is where filtering kicks in.
    let (run_args, subcommand): (Vec<String>, Option<&str>) = match invoked_name {
        "git"
            if rest_args.first().map(String::as_str) == Some("status") && rest_args.len() == 1 =>
        {
            (
                vec!["status".into(), "--porcelain=v1".into(), "--branch".into()],
                Some("git-status"),
            )
        }
        "git" if rest_args.first().map(String::as_str) == Some("log") => {
            (rest_args.to_vec(), Some("git-log"))
        }
        "git"
            if matches!(
                rest_args.first().map(String::as_str),
                Some("diff") | Some("show")
            ) =>
        {
            (rest_args.to_vec(), Some("git-diff"))
        }
        "pytest" => (rest_args.to_vec(), Some("pytest")),
        "cargo" if rest_args.first().map(String::as_str) == Some("test") => {
            (rest_args.to_vec(), Some("cargo-test"))
        }
        _ => (rest_args.to_vec(), None),
    };

    let captured = match shim::run_captured(&real_bin, &run_args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("elagix: failed to run {invoked_name}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let raw = String::from_utf8_lossy(&captured.stdout);

    // Layer B (specs.md §5.2/§5.3): only kicks in when no dedicated Layer A
    // parser matched — it's the declarative fallback for the long tail.
    let camada_b_filters = camada_b::load_all();
    let camada_b_match = if subcommand.is_none() {
        camada_b::find_match(&camada_b_filters, invoked_name, rest_args)
    } else {
        None
    };

    // Business rule 2 (specs.md §4): no success shortcut if the process did
    // not finish successfully. This is each filter's own responsibility (never
    // fabricate "success"), not something achieved by turning off filtering
    // entirely on any non-zero exit — in fact pytest's highest-value case
    // (collection failure) only exists precisely when the exit code is NOT
    // zero. Found live while testing this session (2026-07-26): the earlier
    // version turned off the filter exactly in the case we most wanted to
    // demonstrate. Layer B enforces the same rule internally for its own
    // shortcuts (`match_output`/`on_empty`) — see camada_b/engine.rs.
    let filtered = match subcommand {
        Some("git-status") => Some(filters::git_status::filter(&raw)),
        Some("git-log") => Some(filters::git_log::filter(&raw)),
        Some("git-diff") => Some(filters::git_diff::filter(&raw)),
        Some("pytest") => Some(filters::pytest::filter(&raw)),
        Some("cargo-test") => Some(filters::cargo_test::filter(&raw)),
        _ => camada_b_match.map(|f| camada_b::apply(&f.pipeline, &raw, captured.exit_code)),
    };

    // Progressive disclosure (specs.md §8.1): when the filter signals that
    // real content was dropped (not just reformatted), store the raw output
    // and append a recoverable hint. Checked BEFORE rule 6 on purpose: if the
    // hint doesn't fit the budget, rule 6 falls back to the full raw output —
    // which already IS the complete information, so nothing is lost either way.
    let filtered = filtered.map(|f| {
        if f.contains("lines omitted") || f.contains("more changed lines") {
            let hash = store::put(&raw);
            format!("{f}\n(full output: elagix show {hash})")
        } else {
            f
        }
    });

    // Business rule 6: filtered output can never be larger than the original.
    let mut output = match filtered {
        Some(f) if f.len() < raw.len() => f,
        _ => raw.into_owned(),
    };

    // Cache (§8.2): only writes after confirming success — never caches a
    // process that failed or was interrupted (business rule 2/3).
    if let Some(key) = &git_show_cache_key
        && captured.exit_code == 0
    {
        store::put_keyed(key, &output);
    }

    // Deduplication (§8.3) — the only one of the two store techniques that
    // actually cuts tokens. Approximates "same session" with a time window
    // (documented limitation in specs §8.3: there's no stable session id
    // available here).
    if output.len() >= DEDUP_MIN_BYTES {
        let hash = store::put(&output); // ensures it's recoverable via `elagix show`
        let window: u64 = std::env::var("ELAGIX_DEDUP_WINDOW_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1800);
        if let store::Dedup::SeenRecently = store::check_and_record_dedup(&output, window) {
            let msg = format!("(same as previous output — elagix show {hash} to view it again)");
            if msg.len() < output.len() {
                output = msg;
            }
        }
    }

    print!("{output}");
    if !output.ends_with('\n') {
        println!();
    }

    ExitCode::from(captured.exit_code as u8)
}

fn looks_like_git_sha(s: &str) -> bool {
    (7..=40).contains(&s.len()) && s.chars().all(|c| c.is_ascii_hexdigit())
}
