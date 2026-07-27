use std::env;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Resolve o binário real de `name` no $PATH, ignorando a pasta de shims do Elagix.
///
/// Bug real encontrado e corrigido nesta sessão (2026-07-26): a primeira versão
/// tentava *descobrir* a própria pasta a partir de `argv[0]`, assumindo que o shell
/// sempre passa o caminho completo resolvido. Não é verdade — bash pode passar só
/// o nome nu ("git"), sem diretório nenhum. Isso fazia a checagem de "pular minha
/// própria pasta" falhar silenciosamente, resolver a si mesmo como "binário real",
/// e reprocessar a própria saída já filtrada uma segunda vez (filtro aplicado 2x).
///
/// Correção: não *descobrir* a pasta de shims, **saber** ela de antemão — é o
/// próprio Elagix quem cria os links simbólicos lá durante a instalação, então não
/// precisa inferir nada em tempo de execução.
///
/// Regra de negócio 7 (specs.md §4): herda o mesmo $PATH do processo pai, nunca
/// resolve binário por conta própria fora disso.
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

/// Pasta onde os shims do Elagix moram — configurável via `ELAGIX_SHIMS_DIR` pra
/// facilitar teste (várias instalações lado a lado), com fallback padrão em
/// `~/.elagix/shims`. Canonicalizada pra comparar de forma confiável contra as
/// entradas de `$PATH` (que podem ter formas diferentes do mesmo caminho).
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

/// Roda o binário real capturando stdout (stderr passa direto, igual o comando original faria) —
/// usado no caminho não-interativo (pipe), onde a saída vai ser filtrada antes de chegar no agente.
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

/// Caminho interativo (TTY): substitui o processo atual pelo binário real, sem
/// filtrar nada — passthrough total. No Unix isso é um exec de verdade (mesmo PID,
/// sem processo extra). No Windows, spawna e espera (não existe exec-replace no std).
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
