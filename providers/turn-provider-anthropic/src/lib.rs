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

/// Phase 1: Transform Turn request into Anthropic HTTP config.
/// The host resolves `$env:VAR_NAME` templates in headers/URLs at call time.
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

    // Build system prompt — Anthropic uses a top-level "system" field.
    let mut system_parts = vec![
        "You are a cognitive runtime inference engine mapped to the Turn language. You must return pure JSON matching the user's schema.".to_string()
    ];
    for ctx in &req.params.context {
        system_parts.push(value_to_text(ctx));
    }
    let system_prompt = system_parts.join("\n\n");

    let user_text = value_to_text(&req.params.prompt);

    let mut body = json!({
        "model": "$env:ANTHROPIC_MODEL:claude-3-5-sonnet-20241022",
        "max_tokens": 4096,
        "system": system_prompt,
        "messages": [
            {"role": "user", "content": user_text}
        ]
    });

    if !req.params.tools.is_empty() {
        body["tools"] = Value::Array(req.params.tools.clone());
    }

    // If a strict schema is required, instruct the model to return structured JSON.
    if req.params.schema != json!({"type": "any"}) {
        let current_system = body["system"].as_str().unwrap_or("").to_string();
        body["system"] = json!(format!(
            "{}\n\nIMPORTANT: You MUST respond with ONLY a valid JSON object matching this schema:\n{}",
            current_system,
            req.params.schema
        ));
    }

    let http_config = json!({
        "url": "https://api.anthropic.com/v1/messages",
        "method": "POST",
        "headers": {
            "Content-Type": "application/json",
            "x-api-key": "$env:ANTHROPIC_API_KEY",
            "anthropic-version": "2023-06-01"
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

/// Phase 2: Transform Anthropic HTTP response back into standard Turn format.
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

    let anthropic_json: Value = match serde_json::from_str(&http_res.body) {
        Ok(v) => v,
        Err(e) => {
            return pack_string(
                json!({"jsonrpc": "2.0", "id": 1, "error": format!("Failed to parse Anthropic response: {}", e)})
                    .to_string(),
            )
        }
    };

    if let Some(err) = anthropic_json.get("error") {
        return pack_string(
            json!({"jsonrpc": "2.0", "id": 1, "error": err.to_string()}).to_string(),
        );
    }

    // Anthropic response: { content: [{ type: "text", text: "..." }], usage: { input_tokens, output_tokens } }
    if let Some(content_arr) = anthropic_json.get("content").and_then(|c| c.as_array()) {
        let text = content_arr
            .iter()
            .find(|item| item.get("type").and_then(|t| t.as_str()) == Some("text"))
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        let input_tokens = anthropic_json["usage"]["input_tokens"].as_u64().unwrap_or(0);
        let output_tokens = anthropic_json["usage"]["output_tokens"].as_u64().unwrap_or(0);
        let total_tokens = input_tokens + output_tokens;

        // Normalise to the OpenAI-compatible format Turn's VM expects
        return pack_string(
            json!({
                "choices": [{"message": {"role": "assistant", "content": text}}],
                "usage": {"total_tokens": total_tokens}
            })
            .to_string(),
        );
    }

    pack_string(
        json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid structure from Anthropic"}).to_string(),
    )
}
