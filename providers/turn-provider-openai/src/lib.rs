// turn-provider-openai/src/lib.rs
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Allocate memory in the Wasm guest for the host to write strings into.
#[no_mangle]
pub extern "C" fn alloc(len: u32) -> u32 {
    let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as usize as u32
}

/// Takes an input pointer and length, recreates the Vec to avoid leaking, and converts to a Rust String.
unsafe fn read_string(ptr: u32, len: u32) -> String {
    let buf = Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize);
    String::from_utf8_lossy(&buf).into_owned()
}

/// Packs a String into a (ptr, len) u64 to return to the host.
fn pack_string(s: String) -> u64 {
    let len = s.len() as u64;
    let mut buf = s.into_bytes();
    let ptr = buf.as_mut_ptr() as u64;
    std::mem::forget(buf);
    (ptr << 32) | len
}

#[derive(Deserialize)]
struct TurnInferRequest {
    jsonrpc: String,
    method: String,
    params: InferParams,
    id: u32,
}

#[derive(Deserialize)]
struct InferParams {
    prompt: String,
    schema: Value,
    context: Vec<String>,
    tools: Vec<Value>,
}

/// Phase 1: Host -> Wasm -> Host (HTTP Config)
#[no_mangle]
pub unsafe extern "C" fn transform_request(ptr: u32, len: u32) -> u64 {
    let req_str = read_string(ptr, len);
    
    // We expect a valid JSON-RPC standard Turn request.
    let req: TurnInferRequest = match serde_json::from_str(&req_str) {
        Ok(r) => r,
        Err(e) => return pack_string(json!({ "error": format!("Invalid Turn Request: {}", e) }).to_string()),
    };

    // The host guarantees env vars via some injection mechanism, but for purely Wasm
    // standard, maybe the host evaluates env vars inside `llm_tools` and passes them?
    // Wait, Wasm doesn't have `std::env::var` by default unless WASI is used. 
    // We didn't enable WASI. If we don't enable WASI, `std::env::var` will panic.
    // Let's modify Wasm to just tell the Host *which* env vars to inject into headers.
    // E.g. { "url": "...", "headers": { "Authorization": { "$env": "OPENAI_API_KEY" } } }
    
    // Actually, setting WASI is easy (`wasmtime_wasi`), but passing credentials in the request payload is simpler!
    // Let's assume the Host passes `{ "credentials": { ... }, "request": { ... } }` into `transform_request`.
    // But we already defined the payload to just be the `rpc_request` from `llm_tools.rs`.
    // Let's map it safely without env vars inside Wasm:
    // Wasm returns the HTTP Config. The Host *knows* this is OpenAI, so the Host can attach `OPENAI_API_KEY`.
    // Wait, the WHOLE POINT of the driver is that the Host doesn't know it's OpenAI! 
    // The Driver says: "Host, please make a request to api.openai.com, and please read the OPENAI_API_KEY env var from your secure context and attach it as Bearer."
    
    let sys_msg = "You are a cognitive runtime inference engine mapped to the Turn language. You must return pure JSON matching the user's schema.";
    
    let mut openai_tools = Vec::new();
    for t in req.params.tools {
        openai_tools.push(t);
    }

    let mut messages = Vec::new();
    messages.push(json!({"role": "system", "content": sys_msg}));
    for ctx in req.params.context {
        messages.push(json!({"role": "system", "content": ctx}));
    }
    messages.push(json!({"role": "user", "content": req.params.prompt}));

    let mut body = json!({
        "model": "$env:OPENAI_MODEL:gpt-4o", // Host resolves this template
        "messages": messages,
        "temperature": 0.0,
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

    if !openai_tools.is_empty() {
        body["tools"] = Value::Array(openai_tools);
    }

    let http_config = json!({
        "url": "https://api.openai.com/v1/chat/completions",
        "method": "POST",
        "headers": {
            "Content-Type": "application/json",
            "Authorization": "Bearer $env:OPENAI_API_KEY", // Host resolves this
        },
        "body": body
    });

    pack_string(http_config.to_string())
}

#[derive(Deserialize)]
struct HostHttpResponse {
    status: u16,
    body: String,
    // headers: Value
}

/// Phase 2: Host (HTTP Response) -> Wasm -> Host (Turn Response)
#[no_mangle]
pub unsafe extern "C" fn transform_response(ptr: u32, len: u32) -> u64 {
    let res_str = read_string(ptr, len);
    
    let http_res: HostHttpResponse = match serde_json::from_str(&res_str) {
        Ok(r) => r,
        Err(_) => return pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid HTTP response format from Host"}).to_string()),
    };

    if http_res.status != 200 {
        return pack_string(json!({
            "jsonrpc": "2.0", 
            "id": 1, 
            "error": format!("HTTP {}: {}", http_res.status, http_res.body)
        }).to_string());
    }

    let gpt_json: Value = match serde_json::from_str(&http_res.body) {
        Ok(v) => v,
        Err(e) => return pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": format!("Failed to parse OpenAI response: {}", e)}).to_string()),
    };

    if let Some(err) = gpt_json.get("error") {
        return pack_string(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": err.to_string()
        }).to_string());
    }

    if let Some(choices) = gpt_json.get("choices").and_then(|c| c.as_array()) {
        if choices.is_empty() {
             return pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": "No choices in response"}).to_string());
        }
        let message = &choices[0]["message"];

        if let Some(tools) = message.get("tool_calls").and_then(|t| t.as_array()) {
            if !tools.is_empty() {
                let t = &tools[0];
                return pack_string(json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tool_call",
                    "params": {
                        "name": t["function"]["name"].as_str().unwrap_or(""),
                        "arguments": t["function"]["arguments"].as_str().unwrap_or("{}")
                    }
                }).to_string());
            }
        }

        let content = message["content"].as_str().unwrap_or("");
        let parsed_result: Value = serde_json::from_str(content).unwrap_or_else(|_| json!(content));

        pack_string(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": parsed_result
        }).to_string())
    } else {
        pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid structure from OpenAI"}).to_string())
    }
}
