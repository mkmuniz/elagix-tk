use serde_json::Value;

/// Compressão de resultado de chamada MCP (specs.md §6.2, reusa as técnicas
/// de §5.5). Só as três seguras e puramente mecânicas entram no v1 — nenhuma
/// delas "adivinha" nada, só remove valor explicitamente ausente (`null`) ou
/// corta o que é grande demais pra caber inteiro:
/// - remove campos `null` recursivamente (não é inferência: o valor já era
///   "ausente", só custava token de moldura);
/// - corta string longa mantendo prefixo + contagem do que sobrou;
/// - limita array grande aos primeiros N itens + marcador de omissão.
/// Poda por relevância semântica de campo (paginação, HATEOAS) fica de fora
/// do v1 — dependeria de conhecer a API específica, contrariaria a regra 5.
const MAX_STRING_CHARS: usize = 300;
const MAX_ARRAY_ITEMS: usize = 10;

pub fn compress_tools_call_result(msg: &Value) -> Value {
    let Some(content) = msg.pointer("/result/content").and_then(Value::as_array) else {
        return msg.clone();
    };

    let mut new_content = Vec::with_capacity(content.len());
    let mut changed = false;
    for block in content {
        if let Some(compacted) = try_compact_text_block(block) {
            new_content.push(compacted);
            changed = true;
        } else {
            new_content.push(block.clone());
        }
    }

    if !changed {
        return msg.clone();
    }

    let mut out = msg.clone();
    out["result"]["content"] = Value::Array(new_content);

    // Regra de negócio 6 na mensagem inteira, não só no bloco de texto.
    let orig_len = serde_json::to_string(msg).map(|s| s.len()).unwrap_or(usize::MAX);
    let new_len = serde_json::to_string(&out).map(|s| s.len()).unwrap_or(usize::MAX);
    if new_len < orig_len {
        out
    } else {
        msg.clone()
    }
}

/// Só comprime blocos `{"type": "text", "text": "<json>"}` cujo texto é JSON
/// de verdade — resultado de ferramenta MCP costuma serializar o payload
/// real como string dentro do content block (specs §6.2). Texto livre (não
/// JSON) fica intocado — fora do escopo desta técnica.
fn try_compact_text_block(block: &Value) -> Option<Value> {
    if block.get("type").and_then(Value::as_str) != Some("text") {
        return None;
    }
    let text = block.get("text").and_then(Value::as_str)?;
    let parsed: Value = serde_json::from_str(text).ok()?;
    let compact = compact_json(&parsed);
    let compact_text = serde_json::to_string(&compact).ok()?;
    if compact_text.len() < text.len() {
        let mut b = block.clone();
        b["text"] = Value::String(compact_text);
        Some(b)
    } else {
        None
    }
}

fn compact_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                if v.is_null() {
                    continue;
                }
                out.insert(k.clone(), compact_json(v));
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            let mut out: Vec<Value> = items.iter().take(MAX_ARRAY_ITEMS).map(compact_json).collect();
            if items.len() > MAX_ARRAY_ITEMS {
                out.push(serde_json::json!({
                    "_elagix_omitted_items": items.len() - MAX_ARRAY_ITEMS
                }));
            }
            Value::Array(out)
        }
        Value::String(s) if s.chars().count() > MAX_STRING_CHARS => {
            let truncated: String = s.chars().take(MAX_STRING_CHARS).collect();
            Value::String(format!(
                "{truncated}…(+{} chars omitidos)",
                s.chars().count() - MAX_STRING_CHARS
            ))
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn wrap(text_payload: &Value) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{"type": "text", "text": text_payload.to_string()}],
                "isError": false
            }
        })
    }

    #[test]
    fn strips_null_fields() {
        let msg = wrap(&json!({"id": 1, "author": null, "title": "ok"}));
        let out = compress_tools_call_result(&msg);
        let text = out["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        assert!(parsed.get("author").is_none());
        assert_eq!(parsed["title"], "ok");
    }

    #[test]
    fn caps_large_array_with_marker() {
        let items: Vec<Value> = (0..30).map(|i| json!({"id": i})).collect();
        let msg = wrap(&json!({"items": items}));
        let out = compress_tools_call_result(&msg);
        let text = out["result"]["content"][0]["text"].as_str().unwrap();
        let parsed: Value = serde_json::from_str(text).unwrap();
        let arr = parsed["items"].as_array().unwrap();
        assert_eq!(arr.len(), MAX_ARRAY_ITEMS + 1); // 10 itens + marcador
        assert_eq!(arr[MAX_ARRAY_ITEMS]["_elagix_omitted_items"], 20);
    }

    #[test]
    fn truncates_long_string() {
        let long = "x".repeat(500);
        let msg = wrap(&json!({"excerpt": long}));
        let out = compress_tools_call_result(&msg);
        let text = out["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.len() < 500);
        assert!(text.contains("chars omitidos"));
    }

    #[test]
    fn non_json_text_passthrough() {
        let msg = json!({
            "jsonrpc": "2.0", "id": 1,
            "result": {"content": [{"type": "text", "text": "plain text, not json"}]}
        });
        let out = compress_tools_call_result(&msg);
        assert_eq!(out, msg);
    }

    #[test]
    fn small_result_falls_back_to_original() {
        let msg = wrap(&json!({"ok": true}));
        let out = compress_tools_call_result(&msg);
        assert_eq!(out, msg); // nada pra cortar, regra 6 evita "melhorar" pra pior
    }
}
