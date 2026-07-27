use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

/// Lazy-loading de schema (specs.md §6.1). `tools/list` normalmente devolve
/// nome + descrição completa (às vezes um parágrafo inteiro) + `inputSchema`
/// completo pra CADA ferramenta — isso é pago pra TODA sessão, mesmo que o
/// modelo só use uma ferramenta ou nenhuma. Aqui devolvemos wrappers mínimos
/// e guardamos o original em `schemas` pra `get_tool_schema` buscar sob
/// demanda (mod.rs intercepta essa chamada localmente, nunca chega no
/// servidor real).
const MAX_DESC_CHARS: usize = 140;

pub fn transform_tools_list(msg: &Value, schemas: &Mutex<HashMap<String, Value>>) -> Value {
    let Some(tools) = msg.pointer("/result/tools").and_then(Value::as_array) else {
        return msg.clone(); // fail-open (regra 3): formato inesperado, repassa como veio
    };

    let mut cache = schemas.lock().unwrap();
    let mut compact: Vec<Value> = Vec::with_capacity(tools.len() + 1);
    for tool in tools {
        if let Some(name) = tool.get("name").and_then(Value::as_str) {
            cache.insert(name.to_string(), tool.clone());
        }
        compact.push(compress_tool_for_listing(tool));
    }
    compact.push(synthetic_get_tool_schema_tool());
    drop(cache);

    let mut out = msg.clone();
    out["result"]["tools"] = Value::Array(compact);

    // Regra de negócio 6, aplicada à mensagem JSON-RPC inteira: com só uma
    // ferramenta minúscula, o wrapper + a ferramenta sintética podem custar
    // mais do que o original — nesse caso devolve sem transformar.
    let orig_len = serde_json::to_string(msg).map(|s| s.len()).unwrap_or(usize::MAX);
    let new_len = serde_json::to_string(&out).map(|s| s.len()).unwrap_or(usize::MAX);
    if new_len < orig_len {
        out
    } else {
        msg.clone()
    }
}

fn compress_tool_for_listing(tool: &Value) -> Value {
    let name = tool
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let short = tool
        .get("description")
        .and_then(Value::as_str)
        .map(first_sentence)
        .unwrap_or_default();
    json!({
        "name": name,
        "description": format!("{short} [schema completo: get_tool_schema(\"{name}\")]"),
        "inputSchema": {"type": "object"},
    })
}

fn first_sentence(desc: &str) -> String {
    let cut = desc.find(". ").map(|i| i + 1).unwrap_or(desc.len());
    let sentence = desc[..cut.min(desc.len())].trim();
    if sentence.chars().count() > MAX_DESC_CHARS {
        format!("{}…", sentence.chars().take(MAX_DESC_CHARS).collect::<String>())
    } else {
        sentence.to_string()
    }
}

fn synthetic_get_tool_schema_tool() -> Value {
    json!({
        "name": "get_tool_schema",
        "description": "Devolve o schema completo (inputSchema + descrição integral) de uma ferramenta pelo nome, antes de chamá-la de verdade.",
        "inputSchema": {
            "type": "object",
            "properties": { "tool_name": {"type": "string"} },
            "required": ["tool_name"],
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ferramenta com descrição/schema verbosos, do tamanho comum em
    /// servidores MCP reais (specs §6.1) — um catálogo com várias dessas é
    /// exatamente o caso que motiva o lazy-loading (uma única ferramenta
    /// minúscula não amortiza o custo fixo da ferramenta sintética
    /// `get_tool_schema`, ver `tiny_single_tool_falls_back_to_original`).
    fn verbose_tool(name: &str) -> Value {
        json!({
            "name": name,
            "description": format!(
                "{name} searches the internal documentation corpus for relevant passages. \
                 Supports pagination, relevance scoring, optional filtering by document \
                 category, and returns highlighted excerpts around the matched terms."
            ),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Free-text search query"},
                    "category": {"type": "string", "enum": ["api", "runbook", "product", "faq"]},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            }
        })
    }

    #[test]
    fn transforms_tool_list_and_caches_full_schema() {
        let schemas = Mutex::new(HashMap::new());
        let tools_in = vec![verbose_tool("search_docs"), verbose_tool("search_runbooks")];
        let msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": { "tools": tools_in }
        });

        let out = transform_tools_list(&msg, &schemas);
        let tools = out["result"]["tools"].as_array().unwrap();

        assert_eq!(tools.len(), 3); // 2 ferramentas reais (comprimidas) + get_tool_schema sintética
        assert_eq!(tools[0]["name"], "search_docs");
        assert_eq!(tools[0]["inputSchema"], json!({"type": "object"}));
        let original_desc_len = tools_in[0]["description"].as_str().unwrap().len();
        assert!(tools[0]["description"].as_str().unwrap().len() < original_desc_len);
        assert_eq!(tools[2]["name"], "get_tool_schema");

        let cached = schemas.lock().unwrap();
        let full = cached.get("search_docs").unwrap();
        assert!(full["description"].as_str().unwrap().contains("highlighted excerpts"));
        assert_eq!(full["inputSchema"]["required"], json!(["query"]));
    }

    #[test]
    fn tiny_single_tool_falls_back_to_original() {
        let schemas = Mutex::new(HashMap::new());
        let msg = json!({
            "jsonrpc": "2.0", "id": 1,
            "result": {"tools": [{"name": "x", "description": "y", "inputSchema": {}}]}
        });
        let out = transform_tools_list(&msg, &schemas);
        // regra 6: wrapper + ferramenta sintética custariam mais que o original minúsculo
        assert_eq!(out, msg);
    }
}
