use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Armazém local endereçado por hash (specs.md §8) — infraestrutura
/// compartilhada por disclosure progressivo (8.1), cache (8.2) e
/// deduplicação (8.3). Layout arquivo-por-hash, sem índice em RAM (decisão
/// registrada em specs §13, 2026-07-26): cada chamada do shim lê só o
/// arquivo do hash que precisa.
const MAX_AGE_DAYS: u64 = 14;
const SWEEP_SAMPLE_RATE: u64 = 50; // ~2% de chance de varrer a cada escrita

fn store_root() -> PathBuf {
    if let Ok(dir) = std::env::var("ELAGIX_STORE_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".elagix").join("store")
}

fn hash_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    // 16 hex chars (64 bits) — colisão desprezível pro volume estimado em
    // specs §8.5 (~100 entradas/dia), curto o bastante pra citar em texto
    // (ex: "elagix show a3f9c2d1e8b04f77").
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

fn sharded_path(subdir: &str, key_hash: &str) -> PathBuf {
    let shard = &key_hash[..2.min(key_hash.len())];
    store_root().join(subdir).join(shard).join(key_hash)
}

fn write_file(path: &Path, content: &str) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(path, content);
}

/// Armazém endereçado por conteúdo (CAS) — mesmo conteúdo sempre produz o
/// mesmo hash, escrita é idempotente (regra de negócio 5: determinístico).
/// Usado por disclosure progressivo (8.1) e como backing store de dedup (8.3).
pub fn put(content: &str) -> String {
    let hash = hash_hex(content.as_bytes());
    let path = sharded_path("cas", &hash);
    if !path.exists() {
        write_file(&path, content);
    }
    maybe_sweep();
    hash
}

pub fn get(hash: &str) -> Option<String> {
    let path = sharded_path("cas", hash);
    fs::read_to_string(path).ok()
}

/// Cache por chave arbitrária (8.2) — diferente do CAS: a chave é o
/// identificador do *comando* (ex.: `"git-show:v1:<sha>"`), não o hash do
/// conteúdo, porque precisa ser consultável ANTES de saber o resultado.
pub fn get_keyed(key: &str) -> Option<String> {
    let path = sharded_path("keyed", &hash_hex(key.as_bytes()));
    fs::read_to_string(path).ok()
}

pub fn put_keyed(key: &str, content: &str) {
    let path = sharded_path("keyed", &hash_hex(key.as_bytes()));
    write_file(&path, content);
    maybe_sweep();
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Resultado da checagem de deduplicação (8.3, "esta é a única técnica que
/// reduz token de fato" — cache sozinho só ganha performance).
pub enum Dedup {
    /// Já apareceu dentro da janela — quem chama já tem o hash (calculou
    /// antes de chamar, pra poder gravar no CAS independente do resultado).
    SeenRecently,
    Fresh,
}

/// Checa se `content` já foi mostrado dentro da janela deslizante (limitação
/// conhecida documentada em specs §8.3: aproxima "sessão" por tempo, não por
/// id real — o shim não tem acesso a um identificador de sessão estável).
/// Sempre registra a aparição atual, mesmo quando `Fresh`.
pub fn check_and_record_dedup(content: &str, window_secs: u64) -> Dedup {
    let hash = hash_hex(content.as_bytes());
    let log_path = store_root().join("seen.log");
    let now = now_secs();

    let mut kept: Vec<(u64, String)> = fs::read_to_string(&log_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let (ts, h) = line.split_once(' ')?;
            let ts: u64 = ts.parse().ok()?;
            (now.saturating_sub(ts) <= window_secs).then(|| (ts, h.to_string()))
        })
        .collect();

    let result = if kept.iter().any(|(_, h)| h == &hash) {
        Dedup::SeenRecently
    } else {
        Dedup::Fresh
    };

    kept.push((now, hash));
    let serialized: String = kept
        .iter()
        .map(|(ts, h)| format!("{ts} {h}\n"))
        .collect();
    write_file(&log_path, &serialized);

    result
}

/// Varredura preguiçosa (specs §13, política de limpeza decidida 2026-07-26):
/// sem daemon, então cada escrita tem uma chance pequena de disparar a
/// varredura em vez de rodar toda vez (custo de I/O desnecessário pro volume
/// estimado em specs §8.5).
fn maybe_sweep() {
    if now_secs() % SWEEP_SAMPLE_RATE == 0 {
        force_gc();
    }
}

/// Remove entradas de `cas/` e `keyed/` mais velhas que `MAX_AGE_DAYS`.
/// Fail-open (regra de negócio 3): erro de I/O numa entrada não interrompe a
/// varredura das demais.
pub fn force_gc() {
    let cutoff = now_secs().saturating_sub(MAX_AGE_DAYS * 24 * 60 * 60);
    for subdir in ["cas", "keyed"] {
        sweep_dir(&store_root().join(subdir), cutoff);
    }
}

fn sweep_dir(dir: &Path, cutoff: u64) {
    let Ok(shards) = fs::read_dir(dir) else { return };
    for shard in shards.flatten() {
        let Ok(entries) = fs::read_dir(shard.path()) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() < cutoff)
                .unwrap_or(false);
            if is_old {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

pub fn clear_all() -> std::io::Result<()> {
    let root = store_root();
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // $HOME é global ao processo — testes de store precisam rodar
    // serializados com um diretório isolado, senão correm em paralelo e
    // pisam no mesmo `~/.elagix/store` de verdade.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_isolated_store<T>(f: impl FnOnce() -> T) -> T {
        let _guard = TEST_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("elagix-store-test-{}", now_secs()));
        unsafe {
            std::env::set_var("ELAGIX_STORE_DIR", &dir);
        }
        let result = f();
        let _ = fs::remove_dir_all(&dir);
        unsafe {
            std::env::remove_var("ELAGIX_STORE_DIR");
        }
        result
    }

    #[test]
    fn put_get_roundtrip() {
        with_isolated_store(|| {
            let hash = put("conteudo de teste");
            assert_eq!(get(&hash), Some("conteudo de teste".to_string()));
        });
    }

    #[test]
    fn put_is_idempotent_same_hash() {
        with_isolated_store(|| {
            let h1 = put("igual");
            let h2 = put("igual");
            assert_eq!(h1, h2);
        });
    }

    #[test]
    fn keyed_cache_roundtrip() {
        with_isolated_store(|| {
            assert!(get_keyed("git-show:v1:abc123").is_none());
            put_keyed("git-show:v1:abc123", "saida cacheada");
            assert_eq!(
                get_keyed("git-show:v1:abc123"),
                Some("saida cacheada".to_string())
            );
        });
    }

    #[test]
    fn dedup_detects_repeat_within_window() {
        with_isolated_store(|| {
            assert!(matches!(
                check_and_record_dedup("saida X", 1800),
                Dedup::Fresh
            ));
            assert!(matches!(
                check_and_record_dedup("saida X", 1800),
                Dedup::SeenRecently
            ));
        });
    }

    #[test]
    fn dedup_ignores_entries_outside_window() {
        with_isolated_store(|| {
            check_and_record_dedup("saida Y", 0); // janela zero: expira na hora
            std::thread::sleep(std::time::Duration::from_secs(1));
            assert!(matches!(
                check_and_record_dedup("saida Y", 0),
                Dedup::Fresh
            ));
        });
    }
}
