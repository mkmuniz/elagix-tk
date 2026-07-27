mod engine;

use serde::Deserialize;

pub use engine::apply;

/// Um arquivo de filtro = uma regra (specs.md §5.2/§5.3). `match_command` casa
/// com o nome invocado (ex.: "docker"); `match_args_prefix`, se não vazio,
/// exige que os primeiros N argumentos batam exatamente (ex.: ["images"]
/// pra só ativar em `docker images`, não em `docker ps`).
#[derive(Deserialize, Debug)]
pub struct FilterFile {
    pub match_command: String,
    #[serde(default)]
    pub match_args_prefix: Vec<String>,
    pub pipeline: Vec<Step>,
}

/// Catálogo de ações da Camada B (specs.md §5.3). Só o subconjunto de maior
/// retorno entra no v1 — `group_by`, `json_extract`/`json_schema`,
/// `state_machine`, `aggregate`, `format_template` e `compact_path` ficam
/// pra depois (nenhum comando do v1 precisa deles ainda).
#[derive(Deserialize, Debug)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Step {
    StripAnsi,
    Replace {
        pattern: String,
        replacement: String,
    },
    /// Curto-circuito: specs.md §5.4a. Só dispara em processo bem-sucedido
    /// (regra de negócio 2/3) — reforçado no engine, não aqui.
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
    /// Mesma ressalva de `MatchOutput`: só dispara em sucesso confirmado.
    OnEmpty {
        message: String,
    },
}

/// Filtros embutidos no binário (specs.md §5.2 — cauda longa sem precisar de
/// parser dedicado). Adicionar um comando novo aqui ainda pede recompilar,
/// mas o motor em si (engine.rs) não muda — a extensão real "sem recompilar"
/// vem de `$ELAGIX_FILTERS_DIR` (ver `load_all`), onde arquivos `.toml` novos
/// são lidos em tempo de execução.
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

/// Carrega os filtros embutidos + qualquer `.toml` extra em
/// `$ELAGIX_FILTERS_DIR` (default `~/.elagix/filters`). Regra de negócio 3
/// (fail-open): um arquivo `.toml` malformado é ignorado com aviso em
/// stderr, nunca derruba o processo inteiro.
pub fn load_all() -> Vec<FilterFile> {
    let mut out = Vec::new();

    for (name, raw) in EMBEDDED {
        match toml::from_str::<FilterFile>(raw) {
            Ok(f) => out.push(f),
            Err(e) => eprintln!("elagix: filtro embutido '{name}' inválido, ignorando: {e}"),
        }
    }

    if let Some(dir) = external_filters_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
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
                        "elagix: filtro '{}' inválido ou ilegível, ignorando",
                        path.display()
                    ),
                }
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

/// Acha o primeiro filtro cujo `match_command` bate com o binário invocado e
/// cujo `match_args_prefix` (se houver) é prefixo dos argumentos reais.
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
