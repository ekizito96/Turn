//! turn-provider-azure-anthropic
//! Anthropic Claude via Azure AI Foundry inference driver for Turn.
//!
//! Required env vars:
//!   AZURE_ANTHROPIC_ENDPOINT  — e.g. https://<resource>.services.ai.azure.com/models
//!   AZURE_ANTHROPIC_API_KEY   — Azure AI Foundry key
//!   AZURE_ANTHROPIC_MODEL     — model name, default: claude-3-5-sonnet (as deployed in Azure)
//!
//! Protocol: JSON-RPC 2.0 over stdio. See PROVIDERS.md.

use serde_json::{json, Value as JsonValue};
use std::env;
use std::io::{self, BufRead, Write};

fn call_azure_anthropic(
    api_key: &str,
    endpoint: &str,
    model: &str,
    messages: &[JsonValue],
    schema: &JsonValue,
) -> Result<JsonValue, String> {
    // Azure AI Foundry exposes Anthropic models behind a compatible Messages API
    // endpoint: POST {endpoint}/chat/completions  OR  {endpoint}/{model}/chat/completions
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

    let client = reqwest::blocking::Client::new();

    // Build system message from first system entry if present
    let system_msg = messages.iter()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()))
        .unwrap_or("You are a helpful assistant.")
        .to_string();

    let user_messages: Vec<JsonValue> = messages.iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) != Some("system"))
        .cloned()
        .collect();

    let mut body = json!({
        "model": model,
        "messages": user_messages,
        "system": system_msg,
        "max_tokens": 2048,
    });

    // Attach schema as tool_use tool if structured output is requested
    if schema != &JsonValue::Null {
        let tool = json!({
            "name": "structured_output",
            "description": "Return the result in the requested JSON schema.",
            "input_schema": schema
        });
        body.as_object_mut().unwrap().insert("tools".to_string(), json!([tool]));
        body.as_object_mut().unwrap().insert("tool_choice".to_string(), json!({"type": "tool", "name": "structured_output"}));
    }

    let resp = client
        .post(&url)
        .header("api-key", api_key)
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
    let context = params.get("context").and_then(|c| c.as_array()).cloned().unwrap_or_default();

    let api_key  = env::var("AZURE_ANTHROPIC_API_KEY").unwrap_or_default();
    let endpoint = env::var("AZURE_ANTHROPIC_ENDPOINT").unwrap_or_default();
    let model    = env::var("AZURE_ANTHROPIC_MODEL").unwrap_or_else(|_| "claude-3-5-sonnet".to_string());

    if api_key.is_empty() || endpoint.is_empty() {
        send_error(req_id, "Set AZURE_ANTHROPIC_ENDPOINT and AZURE_ANTHROPIC_API_KEY");
        return;
    }

    let sys = "You are a sovereign intelligent agent. Strictly follow instructions and satisfy all constraints.";
    let mut messages: Vec<JsonValue> = vec![json!({"role": "system", "content": sys})];

    if !context.is_empty() {
        let ctx = context.iter().enumerate()
            .map(|(i, v)| format!("[{}] {}", i, v.as_str().unwrap_or("")))
            .collect::<Vec<_>>().join("\n");
        messages.push(json!({"role":"system","content": format!("WORKING MEMORY:\n{}", ctx)}));
    }
    messages.push(json!({"role": "user", "content": prompt}));

    match call_azure_anthropic(&api_key, &endpoint, &model, &messages, &schema) {
        Ok(j) => process_response(req_id, j, schema),
        Err(e) => send_error(req_id, &e),
    }
}

fn process_response(req_id: JsonValue, j: JsonValue, schema: JsonValue) {
    // Azure Anthropic compat may return in openai-style OR anthropic-style; try both
    // 1. OpenAI-style: choices[0].message.content
    if let Some(content_str) = j.get("choices")
        .and_then(|c| c.as_array()).and_then(|a| a.first())
        .and_then(|c| c.get("message")).and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
    {
        return emit_content(req_id, content_str, &schema);
    }

    // 2. Anthropic-style: content[0].text  OR content[0].input (tool_use)
    if let Some(content_arr) = j.get("content").and_then(|c| c.as_array()) {
        for block in content_arr {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                if let Some(input) = block.get("input") {
                    send_result(req_id, input.clone());
                    return;
                }
            }
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                return emit_content(req_id, text, &schema);
            }
        }
    }

    send_error(req_id, "Unrecognised response format from Azure Anthropic");
}

fn emit_content(req_id: JsonValue, content: &str, schema: &JsonValue) {
    if schema != &JsonValue::Null {
        match serde_json::from_str::<JsonValue>(content) {
            Ok(v) => send_result(req_id, v),
            Err(e) => send_error(req_id, &format!("JSON parse error: {}", e)),
        }
    } else {
        send_result(req_id, json!(content));
    }
}

fn send_result(id: JsonValue, result: JsonValue) {
    println!("{}", json!({"jsonrpc":"2.0","result":result,"id":id}));
    io::stdout().flush().unwrap();
}

fn send_error(id: JsonValue, msg: &str) {
    println!("{}", json!({"jsonrpc":"2.0","error":msg,"id":id}));
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
