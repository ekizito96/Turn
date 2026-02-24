// turn-provider-azure-anthropic/src/lib.rs
use serde::{Deserialize, Serialize};
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

#[no_mangle]
pub unsafe extern "C" fn transform_request(ptr: u32, len: u32) -> u64 {
    let req_str = read_string(ptr, len);
    
    let req: TurnInferRequest = match serde_json::from_str(&req_str) {
        Ok(r) => r,
        Err(e) => return pack_string(json!({ "error": format!("Invalid Turn Request: {}", e) }).to_string()),
    };

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

    // Azure AI Foundry Anthropic uses the standard Azure endpoint but without `/openai/deployments/deploy-name/`
    let http_config = json!({
        "url": "$env:AZURE_ANTHROPIC_ENDPOINT/chat/completions?api-version=2024-02-15-preview",
        "method": "POST",
        "headers": {
            "Content-Type": "application/json",
            "api-key": "$env:AZURE_ANTHROPIC_API_KEY",
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
        Err(e) => return pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": format!("Failed to parse Azure response: {}", e)}).to_string()),
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
        pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid structure from Azure Anthropic"}).to_string())
    }
}
