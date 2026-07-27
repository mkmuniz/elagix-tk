use std::env;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Resolves the real binary for `name` in $PATH, skipping Elagix's own shims folder.
///
/// Real bug found and fixed this session (2026-07-26): the first version
/// tried to *discover* its own folder from `argv[0]`, assuming the shell
/// always passes the fully resolved path. Not true — bash can pass just the
/// bare name ("git"), no directory at all. That made the "skip my own
/// folder" check fail silently, resolve itself as the "real binary", and
/// reprocess its own already-filtered output a second time (filter applied
/// twice).
///
/// Fix: don't *discover* the shims folder, **know** it ahead of time — it's
/// Elagix itself that creates the symlinks there during installation, so
/// there's no need to infer anything at runtime.
///
/// Business rule 7 (specs.md §4): always inherits the same $PATH as the
/// parent process, never resolves a binary on its own outside of that.
pub fn resolve_real_binary(name: &str) -> Option<PathBuf> {
    let own_dir = shims_dir();
    let path_var = env::var_os("PATH")?;

    for dir in env::split_paths(&path_var) {
        let dir_canon = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if Some(&dir_canon) == own_dir.as_ref() {
            continue;
        }
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let with_exe = dir.join(format!("{name}.exe"));
            if with_exe.is_file() {
                return Some(with_exe);
            }
        }
    }
    None
}

/// Folder where Elagix's shims live — configurable via `ELAGIX_SHIMS_DIR` to
/// make testing easier (several installs side by side), defaulting to
/// `~/.elagix/shims`. Canonicalized to compare reliably against `$PATH`
/// entries (which can have different forms of the same path).
fn shims_dir() -> Option<PathBuf> {
    let raw = match env::var_os("ELAGIX_SHIMS_DIR") {
        Some(v) => PathBuf::from(v),
        None => {
            let home = env::var_os("HOME")?;
            PathBuf::from(home).join(".elagix").join("shims")
        }
    };
    Some(raw.canonicalize().unwrap_or(raw))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

pub struct CapturedRun {
    pub stdout: Vec<u8>,
    pub exit_code: i32,
}

/// Runs the real binary capturing stdout (stderr passes straight through,
/// same as the original command would) — used on the non-interactive
/// (pipe) path, where the output will be filtered before it reaches the agent.
pub fn run_captured(real_bin: &Path, args: &[String]) -> std::io::Result<CapturedRun> {
    let output = Command::new(real_bin)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()?;
    Ok(CapturedRun {
        stdout: output.stdout,
        exit_code: output.status.code().unwrap_or(1),
    })
}

/// Interactive (TTY) path: replaces the current process with the real
/// binary, without filtering anything — total passthrough. On Unix this is
/// a real exec (same PID, no extra process). On Windows, spawns and waits
/// (there's no exec-replace in std).
#[cfg(unix)]
pub fn exec_passthrough(real_bin: &Path, args: &[String]) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt;
    let err = Command::new(real_bin).args(args).exec();
    Err(err)
}

#[cfg(windows)]
pub fn exec_passthrough(real_bin: &Path, args: &[String]) -> std::io::Result<()> {
    let status = Command::new(real_bin).args(args).status()?;
    std::process::exit(status.code().unwrap_or(1));
}
