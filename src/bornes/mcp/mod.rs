mod compress;
mod schema;

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, ExitCode, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

/// `bornes/mcp` (specs.md §6) — proxy de protocolo JSON-RPC sobre stdio.
/// Diferente do shim de `bornes/comandos` (processo curto, uma chamada só),
/// este processo fica vivo pela duração inteira da sessão MCP: senta entre o
/// cliente (Claude Code, neste processo, stdin/stdout reais) e o servidor MCP
/// de verdade (processo filho spawnado aqui).
///
/// Duas threads: uma lê o stdin do cliente e repassa (com intercepção) pro
/// stdin do servidor; a principal lê o stdout do servidor e repassa (com
/// intercepção) pro stdout real. Estado compartilhado (`ProxyState`) guarda
/// que método cada `id` de requisição pendente representa — resposta
/// JSON-RPC não repete o método, só o `id` (specs §6.1: só dá pra decidir o
/// que transformar numa resposta de `tools/list` sabendo que a requisição
/// correspondente foi um `tools/list`).
enum PendingKind {
    ToolsList,
    ToolsCall,
}

#[derive(Default)]
struct ProxyState {
    pending: Mutex<HashMap<String, PendingKind>>,
    schemas: Mutex<HashMap<String, Value>>,
}

pub fn run(server_cmd: &str, server_args: &[String]) -> ExitCode {
    let mut child = match Command::new(server_cmd)
        .args(server_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("elagix: falha ao iniciar servidor MCP '{server_cmd}': {e}");
            return ExitCode::FAILURE;
        }
    };

    let child_stdin = Arc::new(Mutex::new(child.stdin.take().expect("stdin piped no spawn")));
    let child_stdout = child.stdout.take().expect("stdout piped no spawn");
    let state = Arc::new(ProxyState::default());

    let state_up = state.clone();
    let child_stdin_up = child_stdin.clone();
    let upstream = thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            handle_client_message(&line, &state_up, &child_stdin_up);
        }
        // `child_stdin_up` cai aqui, no fim do closure.
    });

    // Bug real encontrado testando ao vivo (2026-07-26) com o servidor MCP
    // falso: `child_stdin` (este escopo) e `child_stdin_up` (a thread acima)
    // são duas cópias do mesmo `Arc` — o pipe de stdin do processo filho só
    // fecha de verdade quando a ÚLTIMA cópia cai. Sem este `drop` explícito,
    // esta cópia sobrevive até `run()` retornar, o que só aconteceria DEPOIS
    // de `child.wait()` — mas o servidor real (lendo stdin até EOF) nunca
    // recebe esse EOF enquanto o pipe não fecha, então nunca sai sozinho, e
    // `child.wait()` trava pra sempre. Precisa soltar aqui, antes do loop de
    // leitura abaixo — só a cópia da thread importa a partir daqui, e ela cai
    // quando o stdin do CLIENTE fechar.
    drop(child_stdin);

    let reader = BufReader::new(child_stdout);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        handle_server_message(&line, &state);
    }

    let _ = upstream.join();
    match child.wait() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(0) as u8),
        Err(_) => ExitCode::FAILURE,
    }
}

fn id_key(id: &Value) -> String {
    id.to_string()
}

fn write_raw_line<W: Write>(dest: &Arc<Mutex<W>>, line: &str) {
    let mut w = dest.lock().unwrap();
    let _ = writeln!(w, "{line}");
    let _ = w.flush();
}

fn write_value_to_client(value: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}

/// Cliente -> servidor. Intercepta `tools/call get_tool_schema` localmente
/// (nunca chega no servidor real — ele não conhece essa ferramenta
/// sintética) e registra `tools/list`/`tools/call` pendentes pra saber como
/// tratar a resposta correspondente.
fn handle_client_message(
    line: &str,
    state: &Arc<ProxyState>,
    child_stdin: &Arc<Mutex<std::process::ChildStdin>>,
) {
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        write_raw_line(child_stdin, line); // fail-open (regra 3): não parseou, repassa cru
        return;
    };

    let method = msg.get("method").and_then(Value::as_str);
    let id = msg.get("id").cloned();

    if method == Some("tools/call") {
        let tool_name = msg.pointer("/params/name").and_then(Value::as_str);
        if tool_name == Some("get_tool_schema") {
            respond_get_tool_schema(&msg, id, state);
            return; // curto-circuito local — specs §6.1, passo 2
        }
    }

    if let Some(id) = &id {
        let kind = match method {
            Some("tools/list") => Some(PendingKind::ToolsList),
            Some("tools/call") => Some(PendingKind::ToolsCall),
            _ => None,
        };
        if let Some(kind) = kind {
            state.pending.lock().unwrap().insert(id_key(id), kind);
        }
    }

    write_raw_line(child_stdin, line);
}

fn respond_get_tool_schema(msg: &Value, id: Option<Value>, state: &Arc<ProxyState>) {
    let requested = msg
        .pointer("/params/arguments/tool_name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let schemas = state.schemas.lock().unwrap();
    let found = schemas.get(requested).cloned();
    drop(schemas);

    let (text, is_error) = match found {
        Some(full) => (
            serde_json::to_string(&full).unwrap_or_else(|_| "{}".to_string()),
            false,
        ),
        None => (
            format!("ferramenta '{requested}' não encontrada — rode tools/list primeiro"),
            true,
        ),
    };

    write_value_to_client(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{"type": "text", "text": text}], "isError": is_error },
    }));
}

/// Servidor -> cliente. Só transforma respostas (`result`/`error` presente)
/// cujo `id` corresponde a uma requisição que registramos como
/// `tools/list`/`tools/call` — qualquer outra coisa (notificação, request do
/// próprio servidor, resposta de método sem tratamento especial) passa direto.
fn handle_server_message(line: &str, state: &Arc<ProxyState>) {
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        println!("{line}");
        let _ = std::io::stdout().flush();
        return;
    };

    let id = msg.get("id").cloned();
    let is_response = msg.get("result").is_some() || msg.get("error").is_some();

    let kind = match (&id, is_response) {
        (Some(id), true) => state.pending.lock().unwrap().remove(&id_key(id)),
        _ => None,
    };

    let out = match kind {
        Some(PendingKind::ToolsList) => schema::transform_tools_list(&msg, &state.schemas),
        Some(PendingKind::ToolsCall) => compress::compress_tools_call_result(&msg),
        None => {
            write_value_to_client(&msg);
            return;
        }
    };
    write_value_to_client(&out);
}
