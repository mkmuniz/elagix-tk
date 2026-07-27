use super::Step;
use regex::Regex;

/// Aplica o pipeline declarativo (specs.md §5.3) em ordem. `exit_code` é
/// passado explicitamente porque `MatchOutput` e `OnEmpty` são atalhos de
/// "sucesso confirmado" — regra de negócio 2/3 (specs.md §4, achado de
/// epistemic failure, seção 11): nenhum dos dois pode disparar se o processo
/// não saiu com 0, senão um erro real vira uma frase de sucesso genérica.
pub fn apply(steps: &[Step], raw: &str, exit_code: i32) -> String {
    let mut working = raw.to_string();

    for step in steps {
        match step {
            Step::StripAnsi => working = strip_ansi(&working),
            Step::Replace { pattern, replacement } => {
                let Ok(re) = Regex::new(pattern) else { continue }; // fail-open (regra 3)
                working = re.replace_all(&working, replacement.as_str()).into_owned();
            }
            Step::MatchOutput { pattern, message } => {
                if exit_code != 0 {
                    continue;
                }
                let Ok(re) = Regex::new(pattern) else { continue };
                if re.is_match(&working) {
                    return message.clone(); // curto-circuito: specs.md §5.4a
                }
            }
            Step::KeepLinesMatching { patterns } => {
                let res: Vec<Regex> = patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();
                working = working
                    .lines()
                    .filter(|l| res.iter().any(|re| re.is_match(l)))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Step::StripLinesMatching { patterns } => {
                let res: Vec<Regex> = patterns.iter().filter_map(|p| Regex::new(p).ok()).collect();
                working = working
                    .lines()
                    .filter(|l| !res.iter().any(|re| re.is_match(l)))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Step::Dedup => {
                let mut out: Vec<&str> = Vec::new();
                for line in working.lines() {
                    if out.last() != Some(&line) {
                        out.push(line);
                    }
                }
                working = out.join("\n");
            }
            Step::TruncateLines { max_chars } => {
                working = working
                    .lines()
                    .map(|l| {
                        if l.chars().count() > *max_chars {
                            let cut: String = l.chars().take(*max_chars).collect();
                            format!("{cut}…")
                        } else {
                            l.to_string()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            Step::MaxLines { limit } => {
                let total = working.lines().count();
                if total > *limit {
                    let kept: Vec<&str> = working.lines().take(*limit).collect();
                    working = format!(
                        "{}\n[+{} lines omitted]",
                        kept.join("\n"),
                        total - limit
                    );
                }
            }
            Step::OnEmpty { message } => {
                if exit_code == 0 && working.trim().is_empty() {
                    working = message.clone();
                }
            }
        }
    }

    working
}

fn strip_ansi(s: &str) -> String {
    // Regex compilada uma vez por chamada (v1) — commands passam por aqui uma
    // única vez por invocação, custo desprezível frente ao processo filho.
    let re = Regex::new("\x1b\\[[0-9;]*[a-zA-Z]").expect("regex ANSI estática válida");
    re.replace_all(s, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bornes::comandos::camada_b::Step;

    #[test]
    fn strip_ansi_removes_color_codes() {
        let input = "\x1b[32mok\x1b[0m failed: \x1b[31mno\x1b[0m";
        let out = apply(&[Step::StripAnsi], input, 0);
        assert_eq!(out, "ok failed: no");
    }

    #[test]
    fn match_output_short_circuits_only_on_success() {
        let steps = vec![Step::MatchOutput {
            pattern: "up to date".into(),
            message: "docker: nada a puxar".into(),
        }];
        assert_eq!(apply(&steps, "image is up to date\n", 0), "docker: nada a puxar");
        // exit_code != 0 -> não pode fabricar sucesso (regra 2/3)
        assert_eq!(apply(&steps, "image is up to date\n", 1), "image is up to date\n");
    }

    #[test]
    fn strip_lines_matching_removes_noise() {
        let steps = vec![Step::StripLinesMatching {
            patterns: vec!["^Pulling".into(), "^Digest:".into()],
        }];
        let input = "Pulling fs layer\nDigest: sha256:abc\nStatus: Downloaded\n";
        assert_eq!(apply(&steps, input, 0), "Status: Downloaded");
    }

    #[test]
    fn max_lines_caps_and_marks_omission() {
        let steps = vec![Step::MaxLines { limit: 2 }];
        let input = "a\nb\nc\nd\n";
        assert_eq!(apply(&steps, input, 0), "a\nb\n[+2 lines omitted]");
    }

    #[test]
    fn on_empty_only_fires_on_success() {
        let steps = vec![
            Step::StripLinesMatching {
                patterns: vec![".*".into()],
            },
            Step::OnEmpty {
                message: "nada mudou".into(),
            },
        ];
        assert_eq!(apply(&steps, "x\ny\n", 0), "nada mudou");
        assert_eq!(apply(&steps, "x\ny\n", 1), "");
    }

    #[test]
    fn dedup_collapses_consecutive_duplicates() {
        let steps = vec![Step::Dedup];
        assert_eq!(apply(&steps, "a\na\nb\nb\nb\na\n", 0), "a\nb\na");
    }
}
