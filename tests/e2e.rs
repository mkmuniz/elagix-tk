//! End-to-end tests against the compiled binary: the shim is exercised the
//! way a shell would run it (a symlink named after the tool, placed ahead of
//! a fake "real" tool in PATH), so every behavior validated live by hand
//! (agent gating, exit codes, fail-open, recovery, dedup, determinism) is now
//! part of `cargo test`. Unix-only: shims are symlinks and fakes are sh
//! scripts.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_elagix");

/// An isolated sandbox: its own shims dir, fake-tools dir and store, so
/// tests never touch `~/.elagix` and can run in parallel.
struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "elagix-e2e-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&root);
        for sub in ["shims", "real", "store"] {
            fs::create_dir_all(root.join(sub)).unwrap();
        }
        symlink(BIN, root.join("shims").join("elagix")).unwrap();
        Sandbox { root }
    }

    fn shims(&self) -> PathBuf {
        self.root.join("shims")
    }

    /// A fake real tool: a script that prints `stdout`/`stderr` and exits.
    fn fake_tool(&self, name: &str, stdout: &str, stderr: &str, exit: i32) {
        let out = self.root.join(format!("{name}.stdout"));
        let err = self.root.join(format!("{name}.stderr"));
        fs::write(&out, stdout).unwrap();
        fs::write(&err, stderr).unwrap();
        let script = format!(
            "#!/bin/sh\ncat '{}'\ncat '{}' >&2\nexit {exit}\n",
            out.display(),
            err.display()
        );
        let path = self.root.join("real").join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn shim(&self, name: &str) {
        symlink(BIN, self.shims().join(name)).unwrap();
    }

    /// Runs `name args...` through the shims dir, as an AI agent (`agent`)
    /// or as a plain script. stdout is a pipe here (never a TTY).
    fn run(&self, name: &str, args: &[&str], agent: bool, extra: &[(&str, &str)]) -> Output {
        let path = format!(
            "{}:{}:/usr/bin:/bin",
            self.shims().display(),
            self.root.join("real").display()
        );
        let mut cmd = Command::new(self.shims().join(name));
        cmd.args(args)
            .env_clear()
            .env("PATH", path)
            .env("HOME", &self.root)
            .env("ELAGIX_SHIMS_DIR", self.shims())
            .env("ELAGIX_STORE_DIR", self.root.join("store"))
            .env("ELAGIX_FILTERS_DIR", self.root.join("no-extra-filters"));
        if agent {
            cmd.env("CLAUDECODE", "1");
        }
        for (k, v) in extra {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

const GIT_LOG: &str = "commit 1111111111aaaa\nAuthor: Ana <a@x.io>\nDate:   Sat Jul 25 22:42:21 2026 -0300\n\n    feat: first\n\n    Long body sentence one here. Another sentence two.\n\ncommit 2222222222bbbb\nAuthor: Bia <b@x.io>\nDate:   Fri Jul 24 10:00:00 2026 -0300\n\n    fix: second\n\n    Body of the second commit.\n";

fn git_sandbox() -> Sandbox {
    let sb = Sandbox::new();
    sb.fake_tool("git", GIT_LOG, "", 0);
    sb.shim("git");
    sb
}

#[test]
fn without_agent_output_is_untouched() {
    let sb = git_sandbox();
    let out = sb.run("git", &["log"], false, &[]);
    assert_eq!(stdout(&out), GIT_LOG);
    assert!(out.status.success());
}

#[test]
fn agent_gets_filtered_output() {
    let sb = git_sandbox();
    let out = sb.run("git", &["log"], true, &[]);
    let text = stdout(&out);
    assert!(text.starts_with("1111111111 2026-07-25 22:42 Ana — feat: first"));
    assert!(text.contains("2222222222 2026-07-24 10:00 Bia — fix: second"));
    assert!(text.len() < GIT_LOG.len());
}

#[test]
fn elagix_disable_wins_inside_an_agent() {
    let sb = git_sandbox();
    let out = sb.run("git", &["log"], true, &[("ELAGIX_DISABLE", "1")]);
    assert_eq!(stdout(&out), GIT_LOG);
}

#[test]
fn exit_code_and_stderr_are_preserved() {
    let sb = Sandbox::new();
    sb.fake_tool("git", GIT_LOG, "fatal: something broke\n", 3);
    sb.shim("git");
    let out = sb.run("git", &["log"], true, &[]);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(stderr(&out), "fatal: something broke\n");
}

#[test]
fn missing_real_binary_behaves_like_command_not_found() {
    let sb = Sandbox::new();
    sb.shim("terraform"); // no fake "real" terraform
    let out = sb.run("terraform", &["plan"], true, &[]);
    assert_eq!(out.status.code(), Some(127));
    assert!(stderr(&out).contains("command not found"));
}

#[test]
fn shim_never_resolves_itself() {
    // ELAGIX_SHIMS_DIR points elsewhere, so the folder check can't help —
    // only the "is this my own executable" guard stops the recursion.
    // A tool name that exists nowhere else in PATH (macOS ships a real
    // /usr/bin/git, which would be found legitimately).
    let sb = Sandbox::new();
    sb.shim("elagix-e2e-tool");
    let other = sb.root.join("elsewhere");
    fs::create_dir_all(&other).unwrap();
    let out = sb.run(
        "elagix-e2e-tool",
        &["x"],
        true,
        &[("ELAGIX_SHIMS_DIR", other.to_str().unwrap())],
    );
    assert_eq!(out.status.code(), Some(127));
}

#[test]
fn omitted_content_is_recoverable_with_elagix_show() {
    let sb = git_sandbox();
    let out = sb.run("git", &["log"], true, &[]);
    let text = stdout(&out);
    let hash = text
        .split("elagix show ")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .expect("recovery hint present");
    let shown = sb.run("elagix", &["show", hash], false, &[]);
    assert_eq!(stdout(&shown), GIT_LOG);
}

#[test]
fn dedup_is_scoped_to_the_session() {
    // Dedup only kicks in from 200 bytes of output, so use a longer log.
    let sb = Sandbox::new();
    sb.fake_tool("git", &GIT_LOG.repeat(4), "", 0);
    sb.shim("git");
    let a = [("CLAUDE_CODE_SESSION_ID", "a")];
    let b = [("CLAUDE_CODE_SESSION_ID", "b")];
    let first = stdout(&sb.run("git", &["log"], true, &a));
    let repeat = stdout(&sb.run("git", &["log"], true, &a));
    let other_session = stdout(&sb.run("git", &["log"], true, &b));
    assert!(repeat.starts_with("(same as previous output"));
    assert_eq!(other_session, first);
}

/// Determinism (specs §8.4): the same input must always produce
/// byte-identical output, or the provider's prompt cache is invalidated
/// on every repeat. Each run gets a fresh store so dedup can't kick in.
#[test]
fn output_is_deterministic() {
    let runs: Vec<String> = (0..3)
        .map(|_| {
            let sb = git_sandbox();
            stdout(&sb.run("git", &["log"], true, &[]))
        })
        .collect();
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[1], runs[2]);
}

#[test]
fn stderr_filters_apply_only_for_agents() {
    let sb = Sandbox::new();
    let noise = "#0 building with \"default\" instance\n#1 [internal] load build definition\n#1 DONE 0.1s\n#2 [1/2] FROM alpine\n#3 [2/2] RUN make\n#3 0.51 error: boom\n";
    sb.fake_tool("docker", "", noise, 1);
    sb.shim("docker");

    let agent = sb.run("docker", &["build", "."], true, &[]);
    assert_eq!(
        stderr(&agent),
        "#2 [1/2] FROM alpine\n#3 [2/2] RUN make\n#3 0.51 error: boom\n"
    );
    assert_eq!(agent.status.code(), Some(1));

    let script = sb.run("docker", &["build", "."], false, &[]);
    assert_eq!(stderr(&script), noise);
}

#[test]
fn filter_never_inflates_output() {
    // A tiny git log: the one-line form plus hint would not be smaller, so
    // rule 6 must hand back the original bytes.
    let sb = Sandbox::new();
    let tiny = "commit a\n\n    x\n";
    sb.fake_tool("git", tiny, "", 0);
    sb.shim("git");
    let out = sb.run("git", &["log"], true, &[]);
    assert!(stdout(&out).len() <= tiny.len() + 1);
}

#[test]
fn compress_meta_command_summarizes_stdin() {
    use std::io::Write;
    let sb = Sandbox::new();
    let mut child = Command::new(sb.shims().join("elagix"))
        .args(["compress", "--sentences", "1"])
        .env("ELAGIX_STORE_DIR", sb.root.join("store"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            b"The cache is fast. The parser handles diffs. Rare zebra quantum words appear here.",
        )
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = stdout(&out);
    assert!(out.status.success());
    assert_eq!(text.trim().matches('.').count(), 1, "{text}");
}

/// A long-running command with no filter (a dev server) must stream its
/// output live — capturing it would show the agent nothing until exit.
#[test]
fn unfiltered_long_running_command_streams_live() {
    use std::io::{BufRead, BufReader};
    use std::time::{Duration, Instant};

    let sb = Sandbox::new();
    let path = sb.root.join("real").join("pnpm");
    fs::write(&path, "#!/bin/sh\necho ready\nsleep 30\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    sb.shim("pnpm");

    let mut child = Command::new(sb.shims().join("pnpm"))
        .arg("dev")
        .env_clear()
        .env(
            "PATH",
            format!(
                "{}:{}:/usr/bin:/bin",
                sb.shims().display(),
                sb.root.join("real").display()
            ),
        )
        .env("HOME", &sb.root)
        .env("ELAGIX_SHIMS_DIR", sb.shims())
        .env("ELAGIX_STORE_DIR", sb.root.join("store"))
        .env("CLAUDECODE", "1")
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let elapsed = start.elapsed();
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(line, "ready\n");
    assert!(
        elapsed < Duration::from_secs(15),
        "output was buffered ({elapsed:?})"
    );
}
