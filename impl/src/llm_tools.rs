use crate::ast::Type;
use crate::tools::{ToolHandler, ToolRegistry};
use crate::value::Value;
use serde_json::{json, Value as JsonValue};
use std::env;
use std::process::Command;
use std::sync::Arc;

pub fn turn_value_to_json_rpc(v: &Value) -> JsonValue {
    match v {
        Value::Blob { mime_type, data } => {
            use base64::{engine::general_purpose, Engine as _};
            json!({
                "_turn_blob": true,
                "mime_type": mime_type,
                "data": general_purpose::STANDARD.encode(&**data)
            })
        }
        Value::List(l) => {
            let arr = l.iter().map(turn_value_to_json_rpc).collect::<Vec<_>>();
            JsonValue::Array(arr)
        }
        Value::Map(m) | Value::Struct(_, m) => {
            let mut map = serde_json::Map::new();
            for (k, val) in m.iter() {
                map.insert(k.clone(), turn_value_to_json_rpc(val));
            }
            JsonValue::Object(map)
        }
        Value::Str(s) => json!(&**s),
        Value::Num(n) => json!(n),
        Value::Bool(b) => json!(b),
        Value::Null => json!(null),
        Value::Uncertain(inner, _) => turn_value_to_json_rpc(inner),
        _ => json!(v.to_string()),
    }
}

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
                    return Err(format!(
                        "Missing required field '{}' in struct '{}'",
                        k, name
                    ));
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
    let output = Command::new("curl").arg("-s").arg(&url).output();

    match output {
        Ok(out) if out.status.success() => {
            if let Ok(j) = serde_json::from_slice::<JsonValue>(&out.stdout) {
                let cw = j.get("current_weather").and_then(|c| c.as_object());
                let temp = cw
                    .and_then(|c| c.get("temperature"))
                    .and_then(|t| t.as_f64())
                    .unwrap_or(0.0);
                let code = cw
                    .and_then(|c| c.get("weathercode"))
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0);

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
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg(format!("api-key: {}", api_key))
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-d")
        .arg(body.to_string())
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
                _ => {
                    return Ok(Value::Str(Arc::new(
                        "{\"error\":\"expected lat,lon string\"}".to_string(),
                    )))
                }
            };
            let parts: Vec<&str> = s.split(',').map(|x| x.trim()).collect();
            let (lat, lon) = match (parts.first(), parts.get(1)) {
                (Some(a), Some(b)) => (
                    a.parse::<f64>().unwrap_or(0.0),
                    b.parse::<f64>().unwrap_or(0.0),
                ),
                _ => (37.77, -122.42),
            };
            Ok(Value::Str(Arc::new(fetch_weather_curl(lat, lon))))
        }) as ToolHandler,
    );

    tools.register(
        "llm_infer",
        Box::new(|arg: Value| {
            let mut user_msg_json = JsonValue::Null;
            let mut schema_str = String::new();
            let mut context_list = Vec::new();
            let mut tool_list = Vec::new();
            let mut driver_opt = None;

            if let Value::Map(m) = arg.clone() {
                if let Some(prompt_val) = m.get("prompt") {
                    user_msg_json = turn_value_to_json_rpc(prompt_val);
                }
                if let Some(Value::Str(s)) = m.get("driver") {
                    driver_opt = Some(s.to_string());
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

            let provider_file = if let Some(drv) = driver_opt {
                drv
            } else {
                env::var("TURN_INFER_PROVIDER")
                    .unwrap_or_else(|_| "turn-provider-openai.wasm".to_string())
            };
            
            // We load the provider once to avoid recompiling Wasm overhead per retry
            let provider = match crate::wasm_host::WasmProvider::new(&provider_file) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[Turn VM] WasmProvider load failed: {}", e);
                    return Ok(Value::Null);
                },
            };

            let mut retries = 0;
            const MAX_RETRIES: usize = 3;
            let mut current_prompt_json = user_msg_json.clone();
            let mut context_history_json = context_list.iter().map(turn_value_to_json_rpc).collect::<Vec<_>>();

            loop {
                let rpc_request = json!({
                    "jsonrpc": "2.0",
                    "method": "infer",
                    "params": {
                        "prompt": current_prompt_json,
                        "schema": schema_json,
                        "context": context_history_json,
                        "tools": serialized_tools
                    },
                    "id": 1
                });

                let response_str = match provider.execute_inference(&rpc_request.to_string()) {
                    Ok(res) => res,
                    Err(e) => {
                        eprintln!("[Turn VM] Provider execute_inference failed: {}", e);
                        return Ok(Value::Null);
                    },
                };

                if let Ok(msg) = serde_json::from_str::<JsonValue>(&response_str) {
                    if msg.get("method").and_then(|m| m.as_str()) == Some("tool_call") {
                        // Extract tool parameters from Turn JSON-RPC layer
                        let func_name = msg.get("params").and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or("unknown_tool").to_string();
                        let func_args = msg.get("params").and_then(|p| p.get("arguments")).and_then(|a| a.as_str()).unwrap_or("{}").to_string();
                        return Ok(Value::ToolCallRequest(func_name, func_args));
                    } else if let Some(result) = msg.get("result") {
                        
                        // Pillar 1: Native Boundary Markdown Sanity Coercion
                        let mut sanitized_result = result.clone();
                        if let Some(s) = result.as_str() {
                            let mut content = s;
                            if content.starts_with("```json") {
                                content = &content[7..];
                            } else if content.starts_with("```") {
                                content = &content[3..];
                            }
                            if content.ends_with("```") {
                                content = &content[..content.len() - 3];
                            }
                            let content = content.trim();

                            if let Ok(parsed) = serde_json::from_str::<JsonValue>(content) {
                                sanitized_result = parsed;
                            } else {
                                sanitized_result = JsonValue::String(content.to_string());
                            }
                        }

                        // Pillar 1: Native Boundary Hidden Recovery Loop
                        match json_value_to_turn_value(&turn_type, &sanitized_result) {
                            Ok(val) => return Ok(val),
                            Err(e) => {
                                if retries < MAX_RETRIES {
                                    retries += 1;
                                    context_history_json.push(json!(format!("Assistant Draft: {:?}", result)));
                                    current_prompt_json = json!(format!("Your previous response was structurally invalid and failed coercion. DO NOT format with markdown syntax if it causes errors. Fix this specific schema error and return ONLY raw JSON:\n{}", e));
                                } else {
                                    return Ok(Value::Null);
                                }
                            }
                        }
                    } else if let Some(error) = msg.get("error") {
                        if retries < MAX_RETRIES {
                            retries += 1;
                            let err_str = error.as_str().unwrap_or("Unknown provider error");
                            context_history_json.push(json!(format!("Provider API Error: {}", err_str)));
                            current_prompt_json = json!(format!("The API rejected your previous payload request string. Ensure you perfectly map tools and JSON schema. Fix this error:\n{}", err_str));
                            continue;
                        } else {
                            return Ok(Value::Null);
                        }
                    }
                }

                if retries < MAX_RETRIES {
                    retries += 1;
                    context_history_json.push(json!("Host Error: Invalid provider response format. Returning to user loop."));
                    current_prompt_json = json!("Your output was unparseable. Return valid output.");
                    continue;
                }
                return Ok(Value::Null);
            }
        }) as ToolHandler,
    );
}
