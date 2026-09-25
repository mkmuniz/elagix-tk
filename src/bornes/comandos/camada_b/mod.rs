mod engine;

use serde::Deserialize;

pub use engine::{apply, apply_stderr};

/// One filter file = one rule (specs.md §5.2/§5.3). `match_command` matches
/// the invoked name (e.g. "docker"); `match_args_prefix`, if non-empty,
/// requires the first N arguments to match exactly (e.g. ["images"] to only
/// trigger on `docker images`, not `docker ps`). An element can list
/// alternatives separated by `|` (e.g. ["install|i|ci"]).
#[derive(Deserialize, Debug)]
pub struct FilterFile {
    pub match_command: String,
    #[serde(default)]
    pub match_args_prefix: Vec<String>,
    /// Which output stream the pipeline applies to (default: stdout).
    #[serde(default)]
    pub stream: StreamSpec,
    pub pipeline: Vec<Step>,
}

/// `stream = "stdout" | "stderr" | "both"` in a filter file. "both" runs the
/// same pipeline on each stream separately (never merges them).
#[derive(Deserialize, Debug, Default, PartialEq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum StreamSpec {
    #[default]
    Stdout,
    Stderr,
    Both,
}

/// The stream being filtered right now.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Stream {
    Stdout,
    Stderr,
}

impl StreamSpec {
    fn covers(self, stream: Stream) -> bool {
        matches!(
            (self, stream),
            (StreamSpec::Both, _)
                | (StreamSpec::Stdout, Stream::Stdout)
                | (StreamSpec::Stderr, Stream::Stderr)
        )
    }
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
macro_rules! embedded {
    ($($name:literal),* $(,)?) => {
        &[$(
            (concat!($name, ".toml"), include_str!(concat!("../filters-toml/", $name, ".toml"))),
        )*]
    };
}

const EMBEDDED: &[(&str, &str)] = embedded![
    "docker-images",
    "docker-pull",
    "docker-build",
    "git-branch",
    "terraform-plan",
    "npm-install",
    "pnpm-install",
    "yarn-install-stdout",
    "yarn-install-stderr",
    "pip-install",
    "pip3-install",
    "dotnet-build",
    "cargo-stderr",
    "go-test",
    "go-stderr",
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

/// Finds the first filter for `stream` whose `match_command` matches the
/// invoked binary and whose `match_args_prefix` (if any) is a prefix of the
/// real arguments.
pub fn find_match<'a>(
    filters: &'a [FilterFile],
    invoked_name: &str,
    rest_args: &[String],
    stream: Stream,
) -> Option<&'a FilterFile> {
    filters.iter().find(|f| {
        f.stream.covers(stream)
            && f.match_command == invoked_name
            && rest_args.len() >= f.match_args_prefix.len()
            && rest_args
                .iter()
                .zip(f.match_args_prefix.iter())
                .all(|(a, b)| b.split('|').any(|alt| alt == a))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedded(name: &str) -> FilterFile {
        let (_, raw) = EMBEDDED
            .iter()
            .find(|(n, _)| *n == format!("{name}.toml"))
            .expect("filter is embedded");
        toml::from_str(raw).expect("embedded filter parses")
    }

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn every_embedded_filter_parses() {
        for (name, raw) in EMBEDDED {
            assert!(
                toml::from_str::<FilterFile>(raw).is_ok(),
                "{name} is invalid"
            );
        }
    }

    #[test]
    fn prefix_alternatives_and_stream_selection() {
        let filters: Vec<FilterFile> = EMBEDDED
            .iter()
            .map(|(_, raw)| toml::from_str(raw).unwrap())
            .collect();
        for sub in ["install", "i", "ci", "add"] {
            let m = find_match(&filters, "npm", &args(&[sub, "x"]), Stream::Stderr);
            assert_eq!(m.map(|f| f.match_command.as_str()), Some("npm"), "{sub}");
        }
        assert!(find_match(&filters, "npm", &args(&["run", "build"]), Stream::Stdout).is_none());
        // docker build only targets stderr; docker images only stdout.
        assert!(find_match(&filters, "docker", &args(&["build", "."]), Stream::Stdout).is_none());
        assert!(find_match(&filters, "docker", &args(&["build", "."]), Stream::Stderr).is_some());
        assert!(find_match(&filters, "docker", &args(&["images"]), Stream::Stderr).is_none());
        // cargo test: stdout belongs to Layer A, stderr to cargo-stderr.
        assert!(find_match(&filters, "cargo", &args(&["test"]), Stream::Stderr).is_some());
    }

    /// Real output captured on the dev machine (2026-09-24) — each case
    /// checks what must survive, what must go, and that it actually shrank.
    #[test]
    fn real_fixtures() {
        let cases: &[(&str, &str, &[&str], &[&str])] = &[
            (
                "npm-install",
                include_str!("../filters-toml/fixtures/npm-install.stderr.txt"),
                &[],
                &["deprecated"],
            ),
            (
                "npm-install",
                include_str!("../filters-toml/fixtures/npm-install.stdout.txt"),
                &[
                    "added 47 packages",
                    "5 vulnerabilities (3 moderate, 2 critical)",
                ],
                &["npm fund", "looking for funding"],
            ),
            (
                "pnpm-install",
                include_str!("../filters-toml/fixtures/pnpm-install.stdout.txt"),
                &[
                    "Packages: +112",
                    "+ express 4.21.2",
                    "deprecated subdependencies",
                    "Done in 9s",
                ],
                &["Progress: resolved", "Update available", "+++"],
            ),
            (
                "yarn-install-stdout",
                include_str!("../filters-toml/fixtures/yarn-install.stdout.txt"),
                &["success Saved lockfile.", "Done in"],
                &["[1/4]"],
            ),
            (
                "yarn-install-stderr",
                include_str!("../filters-toml/fixtures/yarn-install.stderr.txt"),
                &[],
                &["deprecated", "No license field"],
            ),
            (
                "pip-install",
                include_str!("../filters-toml/fixtures/pip-install.stdout.txt"),
                &["Successfully installed certifi-2026.7.22"],
                &["Collecting", "Downloading"],
            ),
            (
                "docker-pull",
                include_str!("../filters-toml/fixtures/docker-pull.stdout.txt"),
                &[
                    "Status: Downloaded newer image for busybox:1.36",
                    "docker.io/library/busybox:1.36",
                ],
                &["Pull complete", "Digest:"],
            ),
            (
                "docker-build",
                include_str!("../filters-toml/fixtures/docker-build.stderr.txt"),
                &[
                    "#5 [1/3] FROM",
                    "#6 [2/3] RUN echo hi > /x",
                    "#7 [3/3] COPY . /app",
                ],
                &["[internal]", "exporting", "DONE 0.1s", "transferring"],
            ),
            (
                "dotnet-build",
                include_str!("../filters-toml/fixtures/dotnet-build.stdout.txt"),
                &["Build succeeded.", "0 Warning(s)", "0 Error(s)", "dn.dll"],
                &["Determining projects", "Time Elapsed"],
            ),
        ];
        for (name, raw, keep, drop) in cases {
            let out = engine::apply(&embedded(name).pipeline, raw, 0);
            for k in *keep {
                assert!(out.contains(k), "{name}: lost {k:?}\n{out}");
            }
            for d in *drop {
                assert!(!out.contains(d), "{name}: kept {d:?}\n{out}");
            }
            assert!(out.len() < raw.len(), "{name}: didn't shrink");
        }
    }

    #[test]
    fn go_filters() {
        let raw = "=== RUN   TestA\n--- PASS: TestA (0.00s)\n=== RUN   TestB\n    b_test.go:9: boom\n--- FAIL: TestB (0.00s)\nFAIL\nFAIL\tex.com/m\t0.01s\n";
        let out = engine::apply(&embedded("go-test").pipeline, raw, 1);
        assert_eq!(
            out,
            "    b_test.go:9: boom\n--- FAIL: TestB (0.00s)\nFAIL\nFAIL\tex.com/m\t0.01s"
        );
        let err = "go: downloading github.com/x/y v1.2.3\n./main.go:3:2: undefined: z\n";
        let out = engine::apply(&embedded("go-stderr").pipeline, err, 1);
        assert_eq!(out, "./main.go:3:2: undefined: z");
    }

    #[test]
    fn cargo_stderr_keeps_diagnostics() {
        let raw = "   Compiling regex v1.13.1\n   Compiling elagix v0.1.0 (/x)\nwarning: unused variable: `a`\n --> src/main.rs:2:9\n    Finished `dev` profile [unoptimized] target(s) in 3.1s\n";
        let out = engine::apply(&embedded("cargo-stderr").pipeline, raw, 0);
        assert!(!out.contains("Compiling"));
        assert!(out.contains("warning: unused variable"));
        assert!(out.contains("Finished `dev`"));
    }
}
