// turn-provider-azure-openai-responses/src/lib.rs
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
        "input": messages,
        "model": "$env:AZURE_OPENAI_DEPLOYMENT",
        "max_output_tokens": 16384,
    });

    if req.params.schema != json!({"type": "any"}) {
        body["text"] = json!({
            "format": {
                "type": "json_schema",
                "name": "turn_schema",
                "schema": req.params.schema,
                "strict": true
            }
        });
    }

    if !openai_tools.is_empty() {
        body["tools"] = Value::Array(openai_tools);
    }

    // Azure Serverless Endpoint (GPT-5 series)
    let http_config = json!({
        "url": "$env:AZURE_OPENAI_ENDPOINT/openai/responses?api-version=2025-04-01-preview",
        "method": "POST",
        "headers": {
            "Content-Type": "application/json",
            "Authorization": "Bearer $env:AZURE_OPENAI_API_KEY"
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
        if !err.is_null() {
            return pack_string(json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": err.to_string()
            }).to_string());
        }
    }

    if let Some(outputs) = gpt_json.get("output").and_then(|o| o.as_array()) {
        if outputs.is_empty() {
             return pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": "No outputs in response"}).to_string());
        }

        for output in outputs {
            if output.get("type").and_then(|t| t.as_str()) == Some("message") {
                if let Some(content_arr) = output.get("content").and_then(|c| c.as_array()) {
                    for item in content_arr {
                        // Check for tool calls first
                        if item.get("type").and_then(|t| t.as_str()) == Some("tool_call") {
                            if let Some(t) = item.get("tool_call") {
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

                        // Check for actual text completion
                        if item.get("type").and_then(|t| t.as_str()) == Some("output_text") {
                            let content = item["text"].as_str().unwrap_or("");
                            let parsed_result: Value = serde_json::from_str(content).unwrap_or_else(|_| json!(content));

                            return pack_string(json!({
                                "jsonrpc": "2.0",
                                "id": 1,
                                "result": parsed_result
                            }).to_string());
                        }
                    }
                }
            }
        }
        
        // If we reach here, we didn't find any message output text
        pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": "No valid text or tool calls found in the Azure GPT-5 output payload"}).to_string())
    } else {
        pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid structure from Azure GPT-5 Responses API: missing 'output' array"}).to_string())
    }
}
