#![allow(dead_code, clippy::missing_safety_doc)]
// turn-provider-gemini/src/lib.rs
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
}

fn turn_to_gemini_content(v: &Value) -> Value {
    if let Value::Object(m) = v {
        if m.get("_turn_blob").is_some() {
            let mime = m
                .get("mime_type")
                .and_then(|m| m.as_str())
                .unwrap_or("image/jpeg");
            let data = m.get("data").and_then(|m| m.as_str()).unwrap_or("");
            return json!([{
                "inlineData": {
                    "mimeType": mime,
                    "data": data
                }
            }]);
        }
    } else if let Value::Array(arr) = v {
        let mut parts = Vec::new();
        for item in arr {
            if let Value::Object(m) = item {
                if m.get("_turn_blob").is_some() {
                    let mime = m
                        .get("mime_type")
                        .and_then(|m| m.as_str())
                        .unwrap_or("image/jpeg");
                    let data = m.get("data").and_then(|m| m.as_str()).unwrap_or("");
                    parts.push(json!({
                        "inlineData": {
                            "mimeType": mime,
                            "data": data
                        }
                    }));
                    continue;
                }
            }
            let text = if let Value::String(s) = item {
                s.clone()
            } else {
                item.to_string()
            };
            parts.push(json!({ "text": text }));
        }
        return json!(parts);
    }

    let text = if let Value::String(s) = v {
        s.clone()
    } else {
        v.to_string()
    };
    json!([{ "text": text }])
}

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

    let schema_str = if req.params.schema == json!({"type": "any"}) {
        "None (Freeform text)".to_string()
    } else {
        req.params.schema.to_string()
    };

    let sys_msg_text = format!("You are a cognitive runtime inference engine mapped to the Turn language. You must return pure JSON strictly matching the following schema:\n{}", schema_str);

    let mut parts = Vec::new();

    for ctx in req.params.context {
        let ctx_parts = turn_to_gemini_content(&ctx);
        if let Some(arr) = ctx_parts.as_array() {
            parts.push(json!({"text": "Context: "}));
            parts.extend(arr.iter().cloned());
            parts.push(json!({"text": "\n"}));
        }
    }

    parts.push(json!({"text": "User Prompt: "}));
    let prompt_parts = turn_to_gemini_content(&req.params.prompt);
    if let Some(arr) = prompt_parts.as_array() {
        parts.extend(arr.iter().cloned());
    }

    let body = json!({
        "contents": [{
            "role": "user",
            "parts": parts
        }],
        "systemInstruction": {
            "parts": [{ "text": sys_msg_text }]
        },
        "generationConfig": {
            "temperature": 0.0,
            "responseMimeType": "application/json"
        }
    });

    let http_config = json!({
        "url": "https://generativelanguage.googleapis.com/v1beta/models/$env:GEMINI_MODEL:gemini-1.5-pro:generateContent?key=$env:GEMINI_API_KEY",
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

#[no_mangle]
pub unsafe extern "C" fn transform_response(ptr: u32, len: u32) -> u64 {
    let res_str = read_string(ptr, len);

    let http_res: HostHttpResponse = match serde_json::from_str(&res_str) {
        Ok(r) => r,
        Err(_) => return pack_string(
            json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid HTTP response format from Host"})
                .to_string(),
        ),
    };

    if http_res.status != 200 {
        return pack_string(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": format!("Gemini API HTTP {}: {}", http_res.status, http_res.body)
            })
            .to_string(),
        );
    }

    let gemini_json: Value = match serde_json::from_str(&http_res.body) {
        Ok(v) => v,
        Err(e) => return pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": format!("Failed to parse Gemini response: {}", e)}).to_string()),
    };

    if let Some(err) = gemini_json.get("error") {
        return pack_string(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": err["message"].as_str().unwrap_or("Unknown Gemini Error")
            })
            .to_string(),
        );
    }

    if let Some(candidates) = gemini_json.get("candidates").and_then(|c| c.as_array()) {
        if candidates.is_empty() {
            return pack_string(
                json!({"jsonrpc": "2.0", "id": 1, "error": "No candidates in Gemini response"})
                    .to_string(),
            );
        }
        let content = &candidates[0]["content"];
        let parts = content.get("parts").and_then(|p| p.as_array());

        if let Some(parts_array) = parts {
            if !parts_array.is_empty() {
                let text = parts_array[0]["text"].as_str().unwrap_or("");
                let parsed_result: Value =
                    serde_json::from_str(text).unwrap_or_else(|_| json!(text));

                return pack_string(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "result": parsed_result
                    })
                    .to_string(),
                );
            }
        }
    }
    pack_string(
        json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid structure from Gemini"}).to_string(),
    )
}
