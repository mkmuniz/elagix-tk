mod engine;

use serde::Deserialize;

pub use engine::apply;

/// One filter file = one rule (specs.md §5.2/§5.3). `match_command` matches
/// the invoked name (e.g. "docker"); `match_args_prefix`, if non-empty,
/// requires the first N arguments to match exactly (e.g. ["images"] to only
/// trigger on `docker images`, not `docker ps`).
#[derive(Deserialize, Debug)]
pub struct FilterFile {
    pub match_command: String,
    #[serde(default)]
    pub match_args_prefix: Vec<String>,
    pub pipeline: Vec<Step>,
}

/// Layer B action catalog (specs.md §5.3). Only the highest-value subset
/// makes it into v1 — `group_by`, `json_extract`/`json_schema`,
/// `state_machine`, `aggregate`, `format_template`, and `compact_path` are
/// left for later (no v1 command needs them yet).
#[derive(Deserialize, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Step {
    StripAnsi,
    Replace {
        pattern: String,
        replacement: String,
    },
    /// Short-circuit: specs.md §5.4a. Only fires on a successful process
    /// (business rule 2/3) — enforced in the engine, not here.
    MatchOutput {
        pattern: String,
        message: String,
    },
    KeepLinesMatching {
        patterns: Vec<String>,
    },
    StripLinesMatching {
        patterns: Vec<String>,
    },
    Dedup,
    TruncateLines {
        max_chars: usize,
    },
    MaxLines {
        limit: usize,
    },
    /// Same caveat as `MatchOutput`: only fires on confirmed success.
    OnEmpty {
        message: String,
    },
}

/// Filters embedded in the binary (specs.md §5.2 — long tail without needing
/// a dedicated parser). Adding a new command here still requires a
/// recompile, but the engine itself (engine.rs) doesn't change — the real
/// "no recompile" extension point is `$ELAGIX_FILTERS_DIR` (see `load_all`),
/// where new `.toml` files are read at runtime.
const EMBEDDED: &[(&str, &str)] = &[
    (
        "docker-images.toml",
        include_str!("../filters-toml/docker-images.toml"),
    ),
    (
        "git-branch.toml",
        include_str!("../filters-toml/git-branch.toml"),
    ),
    (
        "terraform-plan.toml",
        include_str!("../filters-toml/terraform-plan.toml"),
    ),
    (
        "npm-install.toml",
        include_str!("../filters-toml/npm-install.toml"),
    ),
];

/// Loads the embedded filters plus any extra `.toml` in
/// `$ELAGIX_FILTERS_DIR` (default `~/.elagix/filters`). Business rule 3
/// (fail-open): a malformed `.toml` file is ignored with a warning on
/// stderr, never brings down the whole process.
pub fn load_all() -> Vec<FilterFile> {
    let mut out = Vec::new();

    for (name, raw) in EMBEDDED {
        match toml::from_str::<FilterFile>(raw) {
            Ok(f) => out.push(f),
            Err(e) => eprintln!("elagix: embedded filter '{name}' is invalid, skipping: {e}"),
        }
    }

    if let Some(dir) = external_filters_dir()
        && let Ok(entries) = std::fs::read_dir(&dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            match std::fs::read_to_string(&path)
                .ok()
                .and_then(|raw| toml::from_str::<FilterFile>(&raw).ok())
            {
                Some(f) => out.push(f),
                None => eprintln!(
                    "elagix: filter '{}' is invalid or unreadable, skipping",
                    path.display()
                ),
            }
        }
    }

    out
}

fn external_filters_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("ELAGIX_FILTERS_DIR") {
        return Some(PathBuf::from(dir));
    }
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".elagix").join("filters"))
}

use std::path::PathBuf;

/// Finds the first filter whose `match_command` matches the invoked binary
/// and whose `match_args_prefix` (if any) is a prefix of the real arguments.
pub fn find_match<'a>(
    filters: &'a [FilterFile],
    invoked_name: &str,
    rest_args: &[String],
) -> Option<&'a FilterFile> {
    filters.iter().find(|f| {
        f.match_command == invoked_name
            && rest_args.len() >= f.match_args_prefix.len()
            && rest_args
                .iter()
                .zip(f.match_args_prefix.iter())
                .all(|(a, b)| a == b)
    })
}
