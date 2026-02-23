//! turn-provider-openai
//! Standard OpenAI inference driver for Turn.
//!
//! Required env vars:
//!   OPENAI_API_KEY     — your api.openai.com secret key
//!   OPENAI_MODEL       — model ID (default: gpt-4o)
//!
//! Protocol: JSON-RPC 2.0 over stdio. See PROVIDERS.md.

use serde_json::{json, Value as JsonValue};
use std::env;
use std::io::{self, BufRead, Write};

fn call_openai(
    api_key: &str,
    model: &str,
    messages: &JsonValue,
    tools: &JsonValue,
    schema: &JsonValue,
) -> Result<JsonValue, String> {
    let url = "https://api.openai.com/v1/chat/completions";

    let client = reqwest::blocking::Client::new();
    let mut body = json!({
        "model": model,
        "messages": messages,
        "max_tokens": 1500,
    });

    if let Some(tool_arr) = tools.as_array() {
        if !tool_arr.is_empty() {
            body.as_object_mut().unwrap().insert("tools".to_string(), tools.clone());
            body.as_object_mut().unwrap().insert("tool_choice".to_string(), json!("auto"));
        }
    }

    if schema.get("type").and_then(|t| t.as_str()) == Some("object") {
        body.as_object_mut().unwrap().insert(
            "response_format".to_string(),
            json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "structured_output",
                    "strict": true,
                    "schema": schema
                }
            }),
        );
    } else if schema != &JsonValue::Null {
        let wrapped = json!({
            "type": "object",
            "properties": { "result": schema },
            "required": ["result"],
            "additionalProperties": false
        });
        body.as_object_mut().unwrap().insert(
            "response_format".to_string(),
            json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "structured_output",
                    "strict": true,
                    "schema": wrapped
                }
            }),
        );
    }

    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("HTTP error: {}", e))?;

    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("API error {}: {}", status, text));
    }

    serde_json::from_str::<JsonValue>(&text).map_err(|_| format!("Invalid JSON: {}", text))
}

fn handle_request(req: JsonValue) {
    let req_id = req.get("id").cloned().unwrap_or(json!(0));
    let params = match req.get("params") {
        Some(p) => p,
        None => { send_error(req_id, "Missing 'params'"); return; }
    };

    let prompt  = params.get("prompt").and_then(|p| p.as_str()).unwrap_or("");
    let schema  = params.get("schema").cloned().unwrap_or(JsonValue::Null);
    let tools   = params.get("tools").cloned().unwrap_or(json!([]));
    let context = params.get("context").and_then(|c| c.as_array()).cloned().unwrap_or_default();

    let api_key = env::var("OPENAI_API_KEY").unwrap_or_default();
    let model   = env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());

    if api_key.is_empty() {
        send_error(req_id, "OPENAI_API_KEY environment variable is not set");
        return;
    }

    let sys_msg = "You are a sovereign intelligent agent. Strictly follow instructions and satisfy constraints.";
    let mut messages: Vec<JsonValue> = vec![json!({"role": "system", "content": sys_msg})];

    if !context.is_empty() {
        let ctx_str = context.iter().enumerate()
            .map(|(i, v)| format!("[{}] {}", i, v.as_str().unwrap_or("")))
            .collect::<Vec<_>>().join("\n");
        messages.push(json!({"role": "system", "content": format!("WORKING MEMORY:\n{}", ctx_str)}));
    }
    messages.push(json!({"role": "user", "content": prompt}));

    match call_openai(&api_key, &model, &json!(messages), &tools, &schema) {
        Ok(j) => process_response(req_id, j, schema),
        Err(e) => send_error(req_id, &e),
    }
}

fn process_response(req_id: JsonValue, j: JsonValue, schema: JsonValue) {
    let choice = j.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first());
    let msg = match choice.and_then(|c| c.get("message")) {
        Some(m) => m,
        None => { send_error(req_id, "No message in response"); return; }
    };

    if let Some(calls) = msg.get("tool_calls").and_then(|t| t.as_array()) {
        if !calls.is_empty() {
            let func = calls[0].get("function").unwrap();
            let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
            let rpc = json!({"jsonrpc":"2.0","method":"tool_call","params":{"name":name,"arguments":arguments},"id":req_id});
            println!("{}", rpc);
            return;
        }
    }

    let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
    if schema != JsonValue::Null {
        match serde_json::from_str::<JsonValue>(content) {
            Ok(parsed) => {
                let result = if schema.get("type").and_then(|t| t.as_str()) != Some("object") {
                    match parsed { JsonValue::Object(mut o) => o.remove("result").unwrap_or(JsonValue::Null), other => other }
                } else { parsed };
                send_result(req_id, result);
            }
            Err(e) => send_error(req_id, &format!("JSON parse error: {}", e)),
        }
    } else {
        send_result(req_id, json!(content));
    }
}

fn send_result(id: JsonValue, result: JsonValue) {
    let res = json!({"jsonrpc":"2.0","result":result,"id":id});
    println!("{}", res);
    io::stdout().flush().unwrap();
}

fn send_error(id: JsonValue, msg: &str) {
    let res = json!({"jsonrpc":"2.0","error":msg,"id":id});
    println!("{}", res);
    io::stdout().flush().unwrap();
}

fn main() {
    for line in io::stdin().lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        if line.trim().is_empty() { continue; }
        if let Ok(req) = serde_json::from_str::<JsonValue>(&line) {
            if req.get("method").and_then(|m| m.as_str()) == Some("infer") {
                handle_request(req);
            } else {
                send_error(req.get("id").cloned().unwrap_or(json!(0)), "Unknown method");
            }
        }
    }
}
