use serde_json::{json, Value as JsonValue};
use std::env;
use std::io::{self, BufRead, Write};

fn call_azure_openai(
    api_key: &str,
    endpoint: &str,
    deployment: &str,
    messages: &JsonValue,
    tools: &JsonValue,
    schema: &JsonValue,
) -> Result<JsonValue, String> {
    let url = format!(
        "{}/openai/deployments/{}/chat/completions?api-version=2024-10-21",
        endpoint.trim_end_matches('/'),
        deployment
    );

    let client = reqwest::blocking::Client::new();
    let mut body = json!({
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
                    "name": "structured_schema",
                    "strict": true,
                    "schema": schema
                }
            })
        );
    } else if schema != &JsonValue::Null {
        let wrapped = json!({
            "type": "object",
            "properties": {
                "result": schema
            },
            "required": ["result"],
            "additionalProperties": false
        });
        body.as_object_mut().unwrap().insert(
            "response_format".to_string(),
            json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "structured_schema",
                    "strict": true,
                    "schema": wrapped
                }
            })
        );
    }

    let resp = match client
        .post(&url)
        .header("api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
    {
        Ok(r) => r,
        Err(e) => return Err(format!("HTTP error: {}", e)),
    };

    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("API error {}: {}", status, text));
    }

    match serde_json::from_str::<JsonValue>(&text) {
        Ok(j) => Ok(j),
        Err(_) => Err(format!("Invalid JSON: {}", text)),
    }
}

fn handle_request(req: JsonValue) {
    let req_id = req.get("id").cloned().unwrap_or(json!(0));
    
    let params = match req.get("params") {
        Some(p) => p,
        None => {
            send_error(req_id, "Missing 'params'");
            return;
        }
    };

    let prompt = params.get("prompt").and_then(|p| p.as_str()).unwrap_or("");
    let schema = params.get("schema").cloned().unwrap_or(JsonValue::Null);
    let tools = params.get("tools").cloned().unwrap_or(json!([]));
    let context = params.get("context").and_then(|c| c.as_array()).cloned().unwrap_or_default();

    let endpoint = env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default();
    let api_key = env::var("AZURE_OPENAI_API_KEY").unwrap_or_default();
    let deployment = env::var("AZURE_OPENAI_DEPLOYMENT").unwrap_or_else(|_| "gpt-4o".to_string());

    if endpoint.is_empty() || api_key.is_empty() {
        send_error(req_id, "Set AZURE_OPENAI_ENDPOINT and AZURE_OPENAI_API_KEY");
        return;
    }

    let sys_msg = "You are a sovereign intelligent agent. Your goal is to strictly follow instructions and satisfy constraints.";
    
    let mut messages: Vec<JsonValue> = vec![
        json!({"role": "system", "content": sys_msg}),
    ];

    if !context.is_empty() {
        let mut ctx_str = String::from("EPISODIC WORKING MEMORY CONTEXT:\n");
        for (idx, ctx_val) in context.iter().enumerate() {
            ctx_str.push_str(&format!("[{}] {}\n", idx, ctx_val.as_str().unwrap_or("")));
        }
        messages.push(json!({"role": "system", "content": ctx_str}));
    }

    messages.push(json!({"role": "user", "content": prompt}));

    // Start inference loop (to handle tool calls if any exist, though the VM currently handles it poorly, the provider can attempt it)
    match call_azure_openai(&api_key, &endpoint, &deployment, &json!(messages), &tools, &schema) {
        Ok(j) => {
            let choice = j.get("choices").and_then(|c| c.as_array()).and_then(|a| a.first());
            let msg = match choice.and_then(|c| c.get("message")) {
                Some(m) => m,
                None => {
                    send_error(req_id, "No message in response");
                    return;
                }
            };

            let tool_calls = msg.get("tool_calls").and_then(|t| t.as_array());
            if let Some(calls) = tool_calls {
                if !calls.is_empty() {
                    let first_call = &calls[0];
                    let func = first_call.get("function").unwrap();
                    let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let arguments = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
                    
                    // Ask the Turn VM to execute the tool
                    let req = json!({
                        "jsonrpc": "2.0",
                        "method": "tool_call",
                        "params": {
                            "name": name,
                            "arguments": arguments
                        },
                        "id": req_id
                    });
                    println!("{}", req.to_string());
                    return; // We stop here. Turn VM hasn't built iterative looping yet with the provider.
                }
            }

            let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");
            if schema != JsonValue::Null {
                match serde_json::from_str::<JsonValue>(content) {
                    Ok(parsed_json) => {
                        let result_json = if schema.get("type").and_then(|t| t.as_str()) != Some("object") {
                            match parsed_json {
                                JsonValue::Object(mut o) => o.remove("result").unwrap_or(JsonValue::Null),
                                other => other,
                            }
                        } else {
                            parsed_json
                        };
                        send_result(req_id, result_json);
                    }
                    Err(e) => send_error(req_id.clone(), &format!("Failed to parse strictly: {}", e)),
                }
            } else {
                send_result(req_id, json!(content));
            }
        }
        Err(e) => send_error(req_id.clone(), &e),
    }
}

fn send_result(id: JsonValue, result: JsonValue) {
    let res = json!({
        "jsonrpc": "2.0",
        "result": result,
        "id": id
    });
    println!("{}", res.to_string());
    io::stdout().flush().unwrap();
}

fn send_error(id: JsonValue, msg: &str) {
    let res = json!({
        "jsonrpc": "2.0",
        "error": msg,
        "id": id
    });
    println!("{}", res.to_string());
    io::stdout().flush().unwrap();
}

fn main() {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        if let Ok(req) = serde_json::from_str::<JsonValue>(&line) {
            if req.get("method").and_then(|m| m.as_str()) == Some("infer") {
                handle_request(req);
            } else {
                send_error(req.get("id").cloned().unwrap_or(json!(0)), "Unknown method");
            }
        }
    }
}
