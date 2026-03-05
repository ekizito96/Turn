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

fn value_to_content(v: &Value) -> Value {
    match v {
        Value::String(s) => json!(s),
        _ => json!(v.to_string()),
    }
}

/// Phase 1: Transform Turn request into xAI Grok HTTP config.
/// Grok uses the OpenAI-compatible chat completions API at api.x.ai.
/// The host resolves `$env:VAR_NAME` templates at call time.
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

    let mut messages = vec![json!({
        "role": "system",
        "content": "You are a cognitive runtime inference engine mapped to the Turn language. You must return pure JSON matching the user's schema."
    })];

    for ctx in &req.params.context {
        messages.push(json!({"role": "system", "content": value_to_content(ctx)}));
    }
    messages.push(json!({"role": "user", "content": value_to_content(&req.params.prompt)}));

    let mut body = json!({
        "model": "$env:GROK_MODEL:grok-3",
        "messages": messages
    });

    if req.params.schema != json!({"type": "any"}) {
        body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": {
                "name": "turn_schema",
                "schema": req.params.schema,
                "strict": true
            }
        });
    }

    if !req.params.tools.is_empty() {
        body["tools"] = Value::Array(req.params.tools.clone());
    }

    let http_config = json!({
        "url": "https://api.x.ai/v1/chat/completions",
        "method": "POST",
        "headers": {
            "Content-Type": "application/json",
            "Authorization": "Bearer $env:XAI_API_KEY"
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

/// Phase 2: Transform xAI Grok HTTP response back into standard Turn format.
/// Grok returns OpenAI-compatible responses, so this is straightforward.
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

    let grok_json: Value = match serde_json::from_str(&http_res.body) {
        Ok(v) => v,
        Err(e) => {
            return pack_string(
                json!({"jsonrpc": "2.0", "id": 1, "error": format!("Failed to parse Grok response: {}", e)})
                    .to_string(),
            )
        }
    };

    if let Some(err) = grok_json.get("error") {
        return pack_string(
            json!({"jsonrpc": "2.0", "id": 1, "error": err.to_string()}).to_string(),
        );
    }

    // Grok returns the OpenAI-compatible choices array
    if let Some(choices) = grok_json.get("choices").and_then(|c| c.as_array()) {
        if choices.is_empty() {
            return pack_string(
                json!({"jsonrpc": "2.0", "id": 1, "error": "No choices in Grok response"})
                    .to_string(),
            );
        }
        let message = &choices[0]["message"];
        let content = message["content"].as_str().unwrap_or("");
        let total_tokens = grok_json["usage"]["total_tokens"].as_u64().unwrap_or(0);

        return pack_string(
            json!({
                "choices": [{"message": {"role": "assistant", "content": content}}],
                "usage": {"total_tokens": total_tokens}
            })
            .to_string(),
        );
    }

    pack_string(
        json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid structure from Grok"}).to_string(),
    )
}
