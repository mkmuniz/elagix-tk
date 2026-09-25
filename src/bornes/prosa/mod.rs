use std::collections::{HashMap, HashSet};

/// `bornes/prosa` (specs.md §7) — TF-IDF extractive summarization. Has no
/// interception mechanism of its own (specs §7.1): it's a pure function,
/// called by `bornes/comandos` (commit message body in `git log`/`git
/// show`) and available as a standalone utility (`elagix compress`).
/// Explicit decision from specs §7.1: extractive strategy only — scores
/// sentences by TF-IDF and keeps the highest-scoring ones, no trained
/// model, no embeddings.
///
/// Summarizes `text` by extracting up to `max_sentences` of the
/// highest-scoring sentences by TF-IDF, returned in their ORIGINAL order of
/// appearance (readability — a summary out of chronological order would
/// confuse more than it would help). Fail-open (business rule 3): empty
/// text or text already within the sentence cap is returned unmodified —
/// summarizing text that's already short saves nothing and risks
/// reformatting for no reason.
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
    top.sort_unstable(); // original order, not score order

    let out = top
        .iter()
        .map(|&i| sentences[i].as_str())
        .collect::<Vec<_>>()
        .join(" ");

    // Business rule 6: guaranteed to be smaller by construction (a subset of
    // sentences), but the explicit check costs nothing and follows the same
    // defensive pattern as the rest of the project (camada_b, mcp proxy).
    if out.len() < trimmed.len() {
        out
    } else {
        trimmed.to_string()
    }
}

/// One-line summary of a commit body for `git log`/`git show` (M7):
/// returns `("summary", <1 sentence>)` when it actually shrank, or
/// `("body", <whole body>)` when it was a single sentence already — shown in
/// full rather than labeled as if it had been compressed (business rule 5).
/// Whitespace is flattened so the result always fits on one line.
pub fn commit_body_line(body_lines: &[&str]) -> (&'static str, String) {
    let body = body_lines.join("\n");
    let flat = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let whole = flat(&body);
    let summary = flat(&summarize(&body, 1));
    if summary.len() < whole.len() {
        ("summary", summary)
    } else {
        ("body", whole)
    }
}

/// Suggested sentence cap for when the caller doesn't know ahead of time how
/// much to cut (used by `elagix compress`, the standalone utility) — keeps
/// roughly 1/3 of the original sentences, at least 1.
pub fn suggested_sentence_budget(text: &str) -> usize {
    let n = split_sentences(text.trim()).len();
    (n / 3).max(1)
}

/// Common abbreviations (EN + PT) whose trailing `.` doesn't end a sentence.
const ABBREVIATIONS: &[&str] = &[
    "e.g", "i.e", "etc", "vs", "approx", "mr", "mrs", "ms", "dr", "prof", "inc", "ltd", "jr", "sr",
    "sra", "st", "no", "fig", "cf", "al", "ex", "p.ex", "obs", "pág", "nº",
];

/// Sentence splitter (revised 2026-09-24). Two passes:
///
/// 1. Structure: a blank line or a line starting a list item (`- `, `* `,
///    `• `, `1. `, `1) `) always starts a new segment — commit bodies are
///    often bullet lists without final periods, which the old splitter
///    glued into one giant "sentence". Other line breaks are soft wraps
///    (commit bodies wrap at ~72 columns) and become spaces.
/// 2. Punctuation: cuts on `.`/`!`/`?` followed by whitespace or end of
///    text — except after a known abbreviation ("e.g.", "Mr.", "etc."), a
///    single-letter initial ("J. Smith"), or when the next word starts in
///    lowercase (the sentence clearly continues). Dots inside a token
///    ("v1.2", "main.rs") never split, since no whitespace follows them.
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for segment in structural_segments(text) {
        split_on_punctuation(&segment, &mut out);
    }
    out
}

fn structural_segments(text: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || starts_list_item(trimmed) {
            if !current.trim().is_empty() {
                segments.push(std::mem::take(&mut current));
            }
            current.clear();
        }
        if !trimmed.is_empty() {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(trimmed);
        }
    }
    if !current.trim().is_empty() {
        segments.push(current);
    }
    segments
}

fn starts_list_item(line: &str) -> bool {
    if ["- ", "* ", "• "].iter().any(|p| line.starts_with(p)) {
        return true;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && (line[digits..].starts_with(". ") || line[digits..].starts_with(") "))
}

fn split_on_punctuation(segment: &str, out: &mut Vec<String>) {
    let chars: Vec<(usize, char)> = segment.char_indices().collect();
    let mut start = 0;
    for (n, &(i, c)) in chars.iter().enumerate() {
        if !matches!(c, '.' | '!' | '?') {
            continue;
        }
        let next = chars.get(n + 1).map(|&(_, c)| c);
        if next.is_some_and(|c| !c.is_whitespace()) {
            continue; // "v1.2", "main.rs", "?!"
        }
        if c == '.' && !ends_sentence(&segment[start..i], &segment[i + 1..]) {
            continue;
        }
        let sentence = segment[start..=i].trim();
        if !sentence.is_empty() {
            out.push(sentence.to_string());
        }
        start = i + c.len_utf8();
    }
    let rest = segment[start..].trim();
    if !rest.is_empty() {
        out.push(rest.to_string());
    }
}

/// Whether a `.` between `before` and `after` really ends a sentence.
fn ends_sentence(before: &str, after: &str) -> bool {
    let word = before
        .rsplit(char::is_whitespace)
        .next()
        .unwrap_or("")
        .trim_start_matches(['(', '"', '\'']);
    let lower = word.to_lowercase();
    if ABBREVIATIONS.contains(&lower.as_str()) {
        return false;
    }
    if word.chars().count() == 1 && word.chars().all(char::is_alphabetic) {
        return false; // initial: "J. Smith"
    }
    match after.trim_start().chars().next() {
        Some(c) => !c.is_lowercase(),
        None => true,
    }
}

fn tokenize(sentence: &str) -> Vec<String> {
    sentence
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_lowercase())
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .collect()
}

/// Classic TF-IDF: `tf` = frequency normalized within the sentence itself,
/// `idf` = log(N sentences / sentences containing the word) + 1 (smoothed,
/// so a word present in every sentence doesn't zero out the whole score).
/// A sentence's score = the sum of tf-idf for each unique word in it.
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

/// Short English+Portuguese stopword list — this data table supports
/// summarizing text in either language (a user's own commit bodies may be
/// in Portuguese, as seen in this project's own real fixtures from M2/M4).
/// Not meant to be exhaustive, just enough to remove the highest-frequency
/// noise that would distort TF-IDF (articles/prepositions show up in every
/// sentence and carry no signal about which sentence matters most).
const STOPWORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "is", "are", "was", "were", "be",
    "this", "that", "it", "with", "as", "at", "by", "from", "but", "not", "no", "so", "we", "our",
    "has", "have", "had", "will", "would", "can", "could", "if", "than", "then", "o", "os", "as",
    "um", "uma", "de", "da", "do", "das", "dos", "e", "ou", "que", "em", "no", "na", "nos", "nas",
    "para", "por", "com", "como", "se", "ao", "aos", "é", "foi", "ser", "não", "mais", "já",
    "também", "isso", "esse", "essa", "só", "sem", "pra",
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
        assert!(out.contains("cargo fmt")); // highest-content/signal sentence, not "Ok."
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
        assert!(pos_committee < pos_persona); // original order preserved, not score order
    }

    #[test]
    fn never_exceeds_original_length() {
        let text = "One. Two. Three. Four. Five. Six. Seven. Eight.";
        let out = summarize(text, 2);
        assert!(out.len() <= text.len());
    }

    #[test]
    fn abbreviations_and_initials_do_not_split() {
        let s = split_sentences(
            "Use a cache, e.g. the CAS store. Talked to J. Smith about v1.2 today. Done.",
        );
        assert_eq!(
            s,
            vec![
                "Use a cache, e.g. the CAS store.",
                "Talked to J. Smith about v1.2 today.",
                "Done."
            ]
        );
    }

    #[test]
    fn lowercase_continuation_does_not_split() {
        let s = split_sentences("Edited main.rs etc. and more. Next one.");
        assert_eq!(s, vec!["Edited main.rs etc. and more.", "Next one."]);
    }

    #[test]
    fn bullets_and_blank_lines_are_boundaries() {
        let body = "Intro paragraph wraps\nacross two lines.\n\n- first bullet without period\n- second bullet\n  continues here\n1. numbered item";
        assert_eq!(
            split_sentences(body),
            vec![
                "Intro paragraph wraps across two lines.",
                "- first bullet without period",
                "- second bullet continues here",
                "1. numbered item"
            ]
        );
    }
}
