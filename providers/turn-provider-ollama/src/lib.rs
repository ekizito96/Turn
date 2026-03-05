#![allow(dead_code, clippy::missing_safety_doc)]

use serde::Deserialize;
use serde_json::{json, Value};

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> u32 {
    let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as usize as u32
}

unsafe fn read_string(ptr: u32, len: u32) -> String {
    let buf = Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize);
    String::from_utf8_lossy(&buf).into_owned()
}

fn pack_string(s: String) -> u64 {
    let len = s.len() as u64;
    let mut buf = s.into_bytes();
    let ptr = buf.as_mut_ptr() as u64;
    std::mem::forget(buf);
    (ptr << 32) | len
}

#[derive(Deserialize)]
struct TurnInferRequest {
    params: InferParams,
}

#[derive(Deserialize)]
struct InferParams {
    prompt: Value,
    schema: Value,
    context: Vec<Value>,
    tools: Vec<Value>,
}

fn value_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

/// Phase 1: Transform Turn request into Ollama HTTP config.
/// Ollama's /api/chat endpoint is OpenAI-compatible.
/// The host resolves `$env:VAR_NAME` templates at call time.
/// No API key is required for Ollama; set OLLAMA_HOST to override the default.
#[no_mangle]
pub unsafe extern "C" fn transform_request(ptr: u32, len: u32) -> u64 {
    let req_str = read_string(ptr, len);
    let req: TurnInferRequest = match serde_json::from_str(&req_str) {
        Ok(r) => r,
        Err(e) => {
            return pack_string(
                json!({ "error": format!("Invalid Turn Request: {}", e) }).to_string(),
            )
        }
    };

    let mut sys_content = "You are a cognitive runtime inference engine mapped to the Turn language. You must return pure JSON matching the user's schema.".to_string();

    if req.params.schema != json!({"type": "any"}) {
        sys_content.push_str(&format!(
            "\n\nIMPORTANT: Respond ONLY with a valid JSON object matching this schema:\n{}",
            req.params.schema
        ));
    }

    let mut messages = vec![json!({"role": "system", "content": sys_content})];

    for ctx in &req.params.context {
        messages.push(json!({"role": "system", "content": value_to_text(ctx)}));
    }
    messages.push(json!({"role": "user", "content": value_to_text(&req.params.prompt)}));

    let mut body = json!({
        "model": "$env:OLLAMA_MODEL:llama3",
        "messages": messages,
        "stream": false
    });

    if !req.params.tools.is_empty() {
        body["tools"] = Value::Array(req.params.tools.clone());
    }

    // OLLAMA_HOST defaults to http://localhost:11434 — the host resolves the $env template.
    let http_config = json!({
        "url": "$env:OLLAMA_HOST:http://localhost:11434/api/chat",
        "method": "POST",
        "headers": {
            "Content-Type": "application/json"
        },
        "body": body
    });

    pack_string(http_config.to_string())
}

#[derive(Deserialize)]
struct HostHttpResponse {
    status: u16,
    body: String,
}

/// Phase 2: Transform Ollama HTTP response back into standard Turn format.
#[no_mangle]
pub unsafe extern "C" fn transform_response(ptr: u32, len: u32) -> u64 {
    let res_str = read_string(ptr, len);
    let http_res: HostHttpResponse = match serde_json::from_str(&res_str) {
        Ok(r) => r,
        Err(_) => {
            return pack_string(
                json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid HTTP response from host"})
                    .to_string(),
            )
        }
    };

    if http_res.status != 200 {
        return pack_string(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": format!("HTTP {}: {}", http_res.status, http_res.body)
            })
            .to_string(),
        );
    }

    let ollama_json: Value = match serde_json::from_str(&http_res.body) {
        Ok(v) => v,
        Err(e) => {
            return pack_string(
                json!({"jsonrpc": "2.0", "id": 1, "error": format!("Failed to parse Ollama response: {}", e)})
                    .to_string(),
            )
        }
    };

    if let Some(err) = ollama_json.get("error") {
        return pack_string(
            json!({"jsonrpc": "2.0", "id": 1, "error": err.to_string()}).to_string(),
        );
    }

    // Ollama /api/chat response: { message: { role, content }, eval_count, prompt_eval_count }
    if let Some(content) = ollama_json
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        let prompt_tokens = ollama_json["prompt_eval_count"].as_u64().unwrap_or(0);
        let eval_tokens = ollama_json["eval_count"].as_u64().unwrap_or(0);
        let total_tokens = prompt_tokens + eval_tokens;

        return pack_string(
            json!({
                "choices": [{"message": {"role": "assistant", "content": content}}],
                "usage": {"total_tokens": total_tokens}
            })
            .to_string(),
        );
    }

    pack_string(
        json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid structure from Ollama"}).to_string(),
    )
}
