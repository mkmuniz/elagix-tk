use std::collections::{HashMap, HashSet};

/// `bornes/prosa` (specs.md §7) — resumo extrativo por TF-IDF. Não tem
/// mecanismo de interceptação próprio (specs §7.1): é função pura, chamada
/// por `bornes/comandos` (corpo de mensagem de commit em `git log`/`git
/// show`) e disponível como utilitário standalone (`elagix compress`).
/// Decisão explícita de specs §7.1: só a estratégia extrativa — pontua
/// frases por TF-IDF e mantém as de maior pontuação, sem modelo treinado,
/// sem embedding.
///
/// Resume `text` extraindo até `max_sentences` frases de maior pontuação
/// TF-IDF, devolvidas na ORDEM ORIGINAL em que aparecem (legibilidade —
/// um resumo fora de ordem cronológica confundiria mais do que ajudaria).
/// Fail-open (regra de negócio 3): texto vazio ou já dentro do teto de
/// frases volta sem modificação — resumir texto que já é curto não
/// economiza nada e arrisca reformatar à toa.
pub fn summarize(text: &str, max_sentences: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() || max_sentences == 0 {
        return trimmed.to_string();
    }

    let sentences = split_sentences(trimmed);
    if sentences.len() <= max_sentences {
        return trimmed.to_string();
    }

    let tokenized: Vec<Vec<String>> = sentences.iter().map(|s| tokenize(s)).collect();
    let scores = tfidf_scores(&tokenized);

    let mut ranked: Vec<usize> = (0..sentences.len()).collect();
    ranked.sort_by(|&a, &b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut top: Vec<usize> = ranked.into_iter().take(max_sentences).collect();
    top.sort_unstable(); // ordem original, não ordem de pontuação

    let out = top
        .iter()
        .map(|&i| sentences[i].trim())
        .collect::<Vec<_>>()
        .join(" ");

    // Regra de negócio 6: garantidamente menor por construção (subconjunto
    // de frases), mas a checagem explícita custa nada e segue o mesmo
    // padrão defensivo do resto do projeto (camada_b, mcp_proxy).
    if out.len() < trimmed.len() {
        out
    } else {
        trimmed.to_string()
    }
}

/// Teto de frases sugerido quando quem chama não sabe de antemão quanto
/// cortar (usado por `elagix compress`, utilitário standalone) — mantém
/// aproximadamente 1/3 das frases originais, no mínimo 1.
pub fn suggested_sentence_budget(text: &str) -> usize {
    let n = split_sentences(text.trim()).len();
    (n / 3).max(1)
}

/// Divisor de frases simples: corta em `.`/`!`/`?` seguido de espaço ou fim
/// de texto. Não trata abreviações ("Sr.", "v1.2") como caso especial —
/// limitação conhecida e aceitável pro caso de uso real (corpo de commit,
/// prompt, prosa curta — não texto jurídico/acadêmico denso de abreviações).
fn split_sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if matches!(b, b'.' | b'!' | b'?') {
            let boundary = bytes.get(i + 1).map(|c| c.is_ascii_whitespace()).unwrap_or(true);
            if boundary {
                let s = text[start..=i].trim();
                if !s.is_empty() {
                    out.push(s);
                }
                start = i + 1;
            }
        }
    }
    let rest = text[start..].trim();
    if !rest.is_empty() {
        out.push(rest);
    }
    out
}

fn tokenize(sentence: &str) -> Vec<String> {
    sentence
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_lowercase())
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// TF-IDF clássico: `tf` = frequência normalizada na própria frase, `idf` =
/// log(N frases / frases que contêm a palavra) + 1 (suavizado, pra palavra
/// presente em toda frase não zerar o score inteiro). Pontuação da frase =
/// soma do tf-idf de cada palavra única nela.
fn tfidf_scores(tokenized: &[Vec<String>]) -> Vec<f64> {
    let n = tokenized.len() as f64;
    let mut df: HashMap<&str, usize> = HashMap::new();
    for sent in tokenized {
        let unique: HashSet<&str> = sent.iter().map(String::as_str).collect();
        for w in unique {
            *df.entry(w).or_insert(0) += 1;
        }
    }

    tokenized
        .iter()
        .map(|sent| {
            if sent.is_empty() {
                return 0.0;
            }
            let mut tf: HashMap<&str, usize> = HashMap::new();
            for w in sent {
                *tf.entry(w.as_str()).or_insert(0) += 1;
            }
            let len = sent.len() as f64;
            tf.iter()
                .map(|(w, count)| {
                    let tf_score = *count as f64 / len;
                    let idf = (n / *df.get(w).unwrap_or(&1) as f64).ln() + 1.0;
                    tf_score * idf
                })
                .sum::<f64>()
        })
        .collect()
}

/// Lista curta de stopwords inglês+português — corpo de commit neste
/// projeto mistura os dois idiomas (visto nos fixtures reais de M2/M4).
/// Não pretende ser exaustiva, só remover o ruído de maior frequência que
/// distorceria o TF-IDF (artigos/preposições aparecem em toda frase e não
/// carregam sinal nenhum sobre qual frase é mais importante).
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "is", "are", "was", "were",
    "be", "this", "that", "it", "with", "as", "at", "by", "from", "but", "not", "no", "so",
    "we", "our", "has", "have", "had", "will", "would", "can", "could", "if", "than", "then",
    "o", "os", "as", "um", "uma", "de", "da", "do", "das", "dos", "e", "ou", "que", "em", "no",
    "na", "nos", "nas", "para", "por", "com", "como", "se", "ao", "aos", "é", "foi", "ser",
    "não", "mais", "já", "também", "isso", "esse", "essa", "só", "sem", "pra",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_within_budget_passes_through() {
        let text = "Fixed the bug. Added a test.";
        assert_eq!(summarize(text, 3), text);
    }

    #[test]
    fn empty_text_passes_through() {
        assert_eq!(summarize("", 1), "");
        assert_eq!(summarize("   ", 2), "");
    }

    #[test]
    fn picks_highest_scoring_sentence_and_shrinks() {
        let text = "Never ran cargo fmt this session, only build/clippy/test -- the CI \
                     fmt-check gate caught real drift across committee.rs and other files. \
                     Ok. Fixed now.";
        let out = summarize(text, 1);
        assert!(out.len() < text.len());
        assert!(out.contains("cargo fmt")); // frase de maior conteúdo/sinal, não "Ok."
    }

    #[test]
    fn preserves_original_order_for_multiple_sentences() {
        let text = "Alpha bravo charlie delta introduces the new committee router logic. \
                     Filler filler filler filler filler. \
                     Echo foxtrot golf hotel rewrites the persona registry cache path. \
                     More filler text that says very little of substance here today.";
        let out = summarize(text, 2);
        let pos_committee = out.find("committee router");
        let pos_persona = out.find("persona registry");
        assert!(pos_committee.is_some() && pos_persona.is_some());
        assert!(pos_committee < pos_persona); // ordem original preservada, não ordem de score
    }

    #[test]
    fn never_exceeds_original_length() {
        let text = "One. Two. Three. Four. Five. Six. Seven. Eight.";
        let out = summarize(text, 2);
        assert!(out.len() <= text.len());
    }
}
