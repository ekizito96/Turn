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

fn pack_string(value: String) -> u64 {
    let len = value.len() as u64;
    let mut buf = value.into_bytes();
    let ptr = buf.as_mut_ptr() as u64;
    std::mem::forget(buf);
    (ptr << 32) | len
}

#[derive(Deserialize)]
struct TurnDecisionRequest {
    params: DecisionParams,
}

#[derive(Deserialize)]
struct DecisionParams {
    state: Value,
    questions: Value,
    #[serde(default)]
    context: Vec<Value>,
}

#[derive(Deserialize)]
struct HostHttpResponse {
    status: u16,
    body: String,
}

fn build_http_request(request: &str) -> Result<String, String> {
    let request: TurnDecisionRequest =
        serde_json::from_str(request).map_err(|error| format!("Invalid Turn request: {error}"))?;

    if !request.params.questions.is_object() {
        return Err("Decision questions must be a map".to_string());
    }

    let state = if request.params.context.is_empty() {
        request.params.state
    } else {
        json!({
            "input": request.params.state,
            "turn_context": request.params.context,
        })
    };

    Ok(json!({
        "url": "https://api.typesafe.ai/v1/systemone",
        "method": "POST",
        "headers": {
            "Authorization": "Bearer $env:TYPESAFE_API_KEY",
            "Content-Type": "application/json",
        },
        "body": {
            "state": state,
            "model": "$env:TYPESAFE_MODEL:jev-latest",
            "questions": request.params.questions,
        }
    })
    .to_string())
}

fn normalize_response(response: &str) -> Result<String, String> {
    let response: HostHttpResponse = serde_json::from_str(response)
        .map_err(|error| format!("Invalid HTTP response from host: {error}"))?;

    if response.status != 200 {
        return Err(format!("HTTP {}: {}", response.status, response.body));
    }

    let body: Value = serde_json::from_str(&response.body)
        .map_err(|error| format!("Failed to parse TypeSafe response: {error}"))?;
    if !body.get("answers").is_some_and(Value::is_object) {
        return Err("TypeSafe response is missing an answers map".to_string());
    }

    let input_tokens = body
        .pointer("/usage/input_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output_tokens = body
        .pointer("/usage/output_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    Ok(json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": body,
        "usage": { "total_tokens": input_tokens + output_tokens },
    })
    .to_string())
}

#[no_mangle]
pub unsafe extern "C" fn transform_request(ptr: u32, len: u32) -> u64 {
    let request = read_string(ptr, len);
    let result =
        build_http_request(&request).unwrap_or_else(|error| json!({ "error": error }).to_string());
    pack_string(result)
}

#[no_mangle]
pub unsafe extern "C" fn transform_response(ptr: u32, len: u32) -> u64 {
    let response = read_string(ptr, len);
    let result = normalize_response(&response)
        .unwrap_or_else(|error| json!({ "jsonrpc": "2.0", "id": 1, "error": error }).to_string());
    pack_string(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_system_one_request_with_turn_context() {
        let request = json!({
            "jsonrpc": "2.0",
            "method": "ai_decide",
            "id": 1,
            "params": {
                "state": "A duplicate charge was reported",
                "questions": {
                    "urgent": {
                        "type": "noul",
                        "instructions": "Does this require urgent review?"
                    }
                },
                "context": ["Account tier: enterprise"]
            }
        });

        let transformed: Value =
            serde_json::from_str(&build_http_request(&request.to_string()).unwrap()).unwrap();
        assert_eq!(transformed["url"], "https://api.typesafe.ai/v1/systemone");
        assert_eq!(
            transformed["body"]["model"],
            "$env:TYPESAFE_MODEL:jev-latest"
        );
        assert_eq!(
            transformed["body"]["state"]["turn_context"][0],
            "Account tier: enterprise"
        );
        assert_eq!(transformed["body"]["questions"]["urgent"]["type"], "noul");
    }

    #[test]
    fn preserves_answers_and_normalizes_usage() {
        let response = json!({
            "status": 200,
            "body": json!({
                "model": "jev-latest",
                "answers": {
                    "department": {
                        "type": "choice",
                        "choice": "billing",
                        "probabilities": { "billing": 0.9, "technical": 0.1 },
                        "confidence": 0.8
                    }
                },
                "usage": { "input_tokens": 20, "output_tokens": 5 }
            }).to_string()
        });

        let normalized: Value =
            serde_json::from_str(&normalize_response(&response.to_string()).unwrap()).unwrap();
        assert_eq!(
            normalized["result"]["answers"]["department"]["choice"],
            "billing"
        );
        assert_eq!(normalized["usage"]["total_tokens"], 25);
    }

    #[test]
    fn reports_http_status_and_body() {
        let response = json!({
            "status": 401,
            "body": "invalid API key"
        });

        let error = normalize_response(&response.to_string()).unwrap_err();
        assert_eq!(error, "HTTP 401: invalid API key");
    }

    #[test]
    fn rejects_success_responses_without_answers() {
        let response = json!({
            "status": 200,
            "body": json!({ "model": "jev-latest" }).to_string()
        });

        let error = normalize_response(&response.to_string()).unwrap_err();
        assert_eq!(error, "TypeSafe response is missing an answers map");
    }
}
