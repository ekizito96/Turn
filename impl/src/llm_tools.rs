use crate::ast::Type;
use crate::tools::{ToolHandler, ToolRegistry};
use crate::value::Value;
use serde_json::{json, Value as JsonValue};
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::Arc;

pub fn turn_type_to_json_schema(ty: &Type) -> JsonValue {
    match ty {
        Type::Num => json!({ "type": "number" }),
        Type::Str => json!({ "type": "string" }),
        Type::Bool => json!({ "type": "boolean" }),
        Type::List(inner) => json!({
            "type": "array",
            "items": turn_type_to_json_schema(inner)
        }),
        Type::Struct(_name, fields) => {
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();
            for (k, v) in fields {
                properties.insert(k.clone(), turn_type_to_json_schema(v));
                required.push(JsonValue::String(k.clone()));
            }
            json!({
                "type": "object",
                "properties": properties,
                "required": required,
                "additionalProperties": false
            })
        }
        Type::Map(_key_ty, val_ty) => json!({
            "type": "object",
            "additionalProperties": turn_type_to_json_schema(val_ty)
        }),
        _ => json!({ "type": ["string", "number", "boolean", "object", "array", "null"] }),
    }
}

pub fn json_value_to_turn_value(ty: &Type, j: &JsonValue) -> Result<Value, String> {
    match (ty, j) {
        (Type::Num, JsonValue::Number(n)) => Ok(Value::Num(n.as_f64().unwrap_or(0.0))),
        (Type::Str, JsonValue::String(s)) => Ok(Value::Str(std::sync::Arc::new(s.clone()))),
        (Type::Bool, JsonValue::Bool(b)) => Ok(Value::Bool(*b)),
        (Type::List(inner_ty), JsonValue::Array(arr)) => {
            let mut items = Vec::new();
            for item in arr {
                items.push(json_value_to_turn_value(inner_ty, item)?);
            }
            Ok(Value::List(std::sync::Arc::new(items)))
        }
        (Type::Struct(name, fields), JsonValue::Object(obj)) => {
            let mut map = indexmap::IndexMap::new();
            for (k, expected_ty) in fields {
                if let Some(field_val) = obj.get(k) {
                    map.insert(k.clone(), json_value_to_turn_value(expected_ty, field_val)?);
                } else {
                    return Err(format!("Missing required field '{}' in struct '{}'", k, name));
                }
            }
            Ok(Value::Struct(
                std::sync::Arc::new(name.clone()),
                std::sync::Arc::new(map),
            ))
        }
        (Type::Map(_k_ty, v_ty), JsonValue::Object(obj)) => {
            let mut map = indexmap::IndexMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), json_value_to_turn_value(v_ty, v)?);
            }
            Ok(Value::Map(std::sync::Arc::new(map)))
        }
        (Type::Any, _) => Ok(Value::Str(std::sync::Arc::new(j.to_string()))),
        (Type::Cap, _) => Err("PrivilegeViolation: LLMs cannot forge a Capability".to_string()),
        _ => Err(format!("Type mismatch: expected {:?}, got {:?}", ty, j)),
    }
}

fn fetch_weather_curl(lat: f64, lon: f64) -> String {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true",
        lat, lon
    );
    let output = Command::new("curl")
        .arg("-s")
        .arg(&url)
        .output();
    
    match output {
        Ok(out) if out.status.success() => {
            if let Ok(j) = serde_json::from_slice::<JsonValue>(&out.stdout) {
                let cw = j.get("current_weather").and_then(|c| c.as_object());
                let temp = cw.and_then(|c| c.get("temperature")).and_then(|t| t.as_f64()).unwrap_or(0.0);
                let code = cw.and_then(|c| c.get("weathercode")).and_then(|c| c.as_u64()).unwrap_or(0);
                
                let desc = match code {
                    0 => "clear",
                    1..=3 => "partly cloudy",
                    45 | 48 => "foggy",
                    51..=67 => "rainy",
                    71..=77 => "snowy",
                    80..=82 => "rain showers",
                    85 | 86 => "snow showers",
                    95..=99 => "thunderstorm",
                    _ => "unknown",
                };
                return json!({"temp": temp, "conditions": desc}).to_string();
            }
            "{\"error\":\"failed to parse weather\"}".to_string()
        }
        _ => "{\"error\":\"curl request failed\"}".to_string(),
    }
}

/// Fallback embedding using curl (used by memory primitive if configured to use API)
pub fn get_embedding(text: &str) -> Option<Vec<f64>> {
    let endpoint = env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default();
    let api_key = env::var("AZURE_OPENAI_API_KEY").unwrap_or_default();
    let deployment = env::var("AZURE_OPENAI_EMBEDDING_DEPLOYMENT")
        .unwrap_or_else(|_| "text-embedding-3-small".to_string());

    if endpoint.is_empty() || api_key.is_empty() {
        return None;
    }

    let url = format!(
        "{}/openai/deployments/{}/embeddings?api-version=2024-10-21",
        endpoint.trim_end_matches('/'),
        deployment
    );

    let body = json!({
        "input": text,
        "model": deployment
    });

    let output = Command::new("curl")
        .arg("-s")
        .arg("-X").arg("POST")
        .arg("-H").arg(format!("api-key: {}", api_key))
        .arg("-H").arg("Content-Type: application/json")
        .arg("-d").arg(body.to_string())
        .arg(&url)
        .output()
        .ok()?;

    if output.status.success() {
        let j: JsonValue = serde_json::from_slice(&output.stdout).ok()?;
        j.get("data")
         .and_then(|d| d.as_array())
         .and_then(|a| a.first())
         .and_then(|e| e.get("embedding"))
         .and_then(|v| v.as_array())
         .map(|a| a.iter().filter_map(|x| x.as_f64()).collect())
    } else {
        None
    }
}

pub fn register_advanced_llm(tools: &mut ToolRegistry) {
    tools.register(
        "weather",
        Box::new(|arg: Value| {
            let s = match &arg {
                Value::Str(s) => s.to_string(),
                Value::Num(n) => n.to_string(),
                _ => return Ok(Value::Str(Arc::new("{\"error\":\"expected lat,lon string\"}".to_string()))),
            };
            let parts: Vec<&str> = s.split(',').map(|x| x.trim()).collect();
            let (lat, lon) = match (parts.first(), parts.get(1)) {
                (Some(a), Some(b)) => (a.parse::<f64>().unwrap_or(0.0), b.parse::<f64>().unwrap_or(0.0)),
                _ => (37.77, -122.42),
            };
            Ok(Value::Str(Arc::new(fetch_weather_curl(lat, lon))))
        }) as ToolHandler,
    );

    tools.register(
        "llm_infer",
        Box::new(|arg: Value| {
            let mut user_msg = String::new();
            let mut schema_str = String::new();
            let mut context_list = Vec::new();
            let mut tool_list = Vec::new();

            if let Value::Map(m) = arg.clone() {
                if let Some(Value::Str(s)) = m.get("prompt") {
                    user_msg = s.to_string();
                }
                if let Some(Value::Str(s)) = m.get("schema") {
                    schema_str = s.to_string();
                }
                if let Some(Value::List(l)) = m.get("context") {
                    context_list = l.to_vec();
                }
                if let Some(Value::List(l)) = m.get("tools") {
                    tool_list = l.to_vec();
                }
            } else {
                return Ok(Value::Str(Arc::new("Expected Map argument".to_string())));
            }

            let turn_type: Type = serde_json::from_str(&schema_str).unwrap_or(Type::Any);
            let schema_json = turn_type_to_json_schema(&turn_type);

            // Serialize tools into JSON schemas
            let mut serialized_tools = Vec::new();
            for (idx, t) in tool_list.iter().enumerate() {
                if let Value::Closure { is_tool, params, .. } = t {
                    if *is_tool {
                        let mut properties = serde_json::Map::new();
                        let mut required = Vec::new();

                        for (p_name, ty_opt, is_secret) in params {
                            if *is_secret { continue; }
                            if matches!(ty_opt, Some(Type::Cap)) { continue; }

                            let p_ty_json = if let Some(ty) = ty_opt {
                                turn_type_to_json_schema(ty)
                            } else {
                                json!({"type":"string"})
                            };
                            properties.insert(p_name.clone(), p_ty_json);
                            required.push(p_name.clone());
                        }

                        serialized_tools.push(json!({
                            "type": "function",
                            "function": {
                                "name": format!("tool_{}", idx),
                                "description": format!("Turn dynamically injected tool {}", idx),
                                "parameters": {
                                    "type": "object",
                                    "properties": properties,
                                    "required": required,
                                    "additionalProperties": false
                                }
                            }
                        }));
                    }
                }
            }

            // Determine Inference Provider Provider Executable
            let provider_cmd = env::var("TURN_INFER_PROVIDER")
                .unwrap_or_else(|_| "turn-provider-openai".to_string());

            let mut child = match Command::new(&provider_cmd)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => return Ok(Value::Str(Arc::new(format!("Failed to spawn inference provider '{}': {}", provider_cmd, e)))),
            };

            let mut stdin = child.stdin.take().expect("Failed to open child stdin");
            let stdout = child.stdout.take().expect("Failed to open child stdout");
            let mut reader = BufReader::new(stdout);

            let rpc_request = json!({
                "jsonrpc": "2.0",
                "method": "infer",
                "params": {
                    "prompt": user_msg,
                    "schema": schema_json,
                    "context": context_list.iter().map(|v| format!("{:?}", v)).collect::<Vec<_>>(),
                    "tools": serialized_tools
                },
                "id": 1
            });

            if let Err(e) = writeln!(stdin, "{}", rpc_request.to_string()) {
                return Ok(Value::Str(Arc::new(format!("RPC Write Error: {}", e))));
            }

            // Wait for responses
            let mut line = String::new();
            while let Ok(bytes_read) = reader.read_line(&mut line) {
                if bytes_read == 0 { break; } // EOF

                if let Ok(msg) = serde_json::from_str::<JsonValue>(&line) {
                    if msg.get("method").and_then(|m| m.as_str()) == Some("tool_call") {
                        // Provider wants us to execute a tool
                        let params = msg.get("params").unwrap();
                        let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let tool_args_str = params.get("arguments").and_then(|n| n.as_str()).unwrap_or("{}");
                        let req_id = msg.get("id").unwrap();

                        // Parse the tool index
                        if tool_name.starts_with("tool_") {
                            if let Ok(idx) = tool_name[5..].parse::<usize>() {
                                if let Some(Value::Closure { .. }) = tool_list.get(idx) {
                                    // Normally we would invoke the closure here via the VM runner.
                                    // But llm_tools is just a standalone host function, it does not have back-ref to Runner.
                                    // For phase 1 refactor, we mock tool execution or pass it out to be executed by VM.
                                    // Wait, the original `llm_infer` couldn't execute generic closures either!
                                    // The original bypassed VM completely and just called `fetch_weather` internally!
                                    // Since we're separating, we will return an error instructing the VM to handle tool calls later.
                                    let result_str = "Turn AST execution from within llm_infer is currently unsupported.".to_string();
                                    let rpc_res = json!({
                                        "jsonrpc": "2.0",
                                        "result": result_str,
                                        "id": req_id
                                    });
                                    writeln!(stdin, "{}", rpc_res.to_string()).unwrap();
                                }
                            }
                        }
                    } else if let Some(result) = msg.get("result") {
                        // Success! Return value
                        match json_value_to_turn_value(&turn_type, result) {
                            Ok(val) => return Ok(val),
                            Err(e) => return Ok(Value::Str(Arc::new(format!("Provider type map error: {}", e)))),
                        }
                    } else if let Some(error) = msg.get("error") {
                        // Provider error
                        return Ok(Value::Str(Arc::new(format!("Provider Error: {}", error))));
                    }
                }
                
                line.clear();
            }

            let _ = child.wait();
            Ok(Value::Str(Arc::new("Inference provider disconnected without returning a result".to_string())))
        }) as ToolHandler,
    );
}
