//! Azure OpenAI + weather tools for LLM agents.

use crate::ast::Type;
use crate::tools::{ToolHandler, ToolRegistry};
use crate::value::Value;
use serde_json::{json, Value as JsonValue};
use std::env;

fn fetch_weather(lat: f64, lon: f64) -> String {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current_weather=true",
        lat, lon
    );
    match reqwest::blocking::get(&url) {
        Ok(r) => match r.json::<serde_json::Value>() {
            Ok(j) => {
                let cw = j.get("current_weather").and_then(|c| c.as_object());
                let temp = cw
                    .and_then(|c| c.get("temperature"))
                    .and_then(|t| t.as_f64())
                    .unwrap_or(0.0);
                let code = cw
                    .and_then(|c| c.get("weathercode"))
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0);
                let desc = weather_code_to_desc(code);
                json!({"temp": temp, "conditions": desc}).to_string()
            }
            Err(_) => "{\"error\":\"failed to parse weather\"}".to_string(),
        },
        Err(e) => format!("{{\"error\":\"{}\"}}", e),
    }
}

fn weather_code_to_desc(code: u64) -> &'static str {
    match code {
        0 => "clear",
        1..=3 => "partly cloudy",
        45 | 48 => "foggy",
        51..=67 => "rainy",
        71..=77 => "snowy",
        80..=82 => "rain showers",
        85 | 86 => "snow showers",
        95..=99 => "thunderstorm",
        _ => "unknown",
    }
}

fn turn_type_to_json_schema(ty: &Type) -> JsonValue {
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

fn json_value_to_turn_value(ty: &Type, j: &JsonValue) -> Result<Value, String> {
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
        (Type::Any, _) => {
            // fallback
            Ok(Value::Str(std::sync::Arc::new(j.to_string())))
        }
        (Type::Cap, _) => Err("PrivilegeViolation: LLMs cannot forge a Capability".to_string()),
        _ => Err(format!("Type mismatch: expected {:?}, got {:?}", ty, j)),
    }
}

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

    let client = reqwest::blocking::Client::new();
    let body = json!({
        "input": text,
        "model": deployment
    });

    match client
        .post(&url)
        .header("api-key", &api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            let j: JsonValue = resp.json().unwrap_or(json!({}));
            let vec: Option<Vec<f64>> = j
                .get("data")
                .and_then(|d| d.as_array())
                .and_then(|a| a.first())
                .and_then(|e| e.get("embedding"))
                .and_then(|v| v.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_f64()).collect());
            vec
        }
        _ => None,
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
                    return Ok(Value::Str(std::sync::Arc::new(
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
            Ok(Value::Str(std::sync::Arc::new(fetch_weather(lat, lon))))
        }) as ToolHandler,
    );

    tools.register(
        "llm_infer",
        Box::new(|arg: Value| {
            let mut user_msg = String::new();
            let mut schema_str = String::new();
            let mut context_list = Vec::new();
            let mut tool_list = Vec::new();

            if let Value::Map(m) = arg {
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
                return Ok(Value::Str(std::sync::Arc::new("Expected Map argument".to_string())));
            }

            let turn_type: Type = serde_json::from_str(&schema_str).unwrap_or(Type::Any);

            let endpoint = env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_else(|_| String::new());
            let api_key = env::var("AZURE_OPENAI_API_KEY").unwrap_or_else(|_| String::new());
            let deployment =
                env::var("AZURE_OPENAI_DEPLOYMENT").unwrap_or_else(|_| "gpt-4o".to_string());

            if endpoint.is_empty() || api_key.is_empty() {
                return Ok(Value::Str(std::sync::Arc::new(
                    "Set AZURE_OPENAI_ENDPOINT, AZURE_OPENAI_API_KEY, AZURE_OPENAI_DEPLOYMENT"
                        .to_string(),
                )));
            }

            let url = format!(
                "{}/openai/deployments/{}/chat/completions?api-version=2024-10-21",
                endpoint.trim_end_matches('/'),
                deployment
            );

            let mut oai_tools = Vec::new();
            for (idx, t) in tool_list.iter().enumerate() {
                if let Value::Closure { is_tool, params, .. } = t {
                    if *is_tool {
                        let mut properties = serde_json::Map::new();
                        let mut required = Vec::new();

                        for (p_name, ty_opt, is_secret) in params {
                            // Phase 2: First-Class Tooling & Cognitive Offloading
                            // Exclude secret parameters entirely so LLMs do not need to generate them.
                            if *is_secret { continue; }
                            if matches!(ty_opt, Some(Type::Cap)) { continue; } // Phase 8: Hard capability omission

                            let p_ty_json = if let Some(ty) = ty_opt {
                                turn_type_to_json_schema(ty)
                            } else {
                                json!({"type":"string"})
                            };
                            properties.insert(p_name.clone(), p_ty_json);
                            required.push(p_name.clone());
                        }

                        let tool_schema = json!({
                            "type": "function",
                            "function": {
                                "name": format!("tool_{}", idx),
                                "description": format!("Dynamically injected tool {}", idx),
                                "parameters": {
                                    "type": "object",
                                    "properties": properties,
                                    "required": required,
                                    "additionalProperties": false
                                }
                            }
                        });
                        oai_tools.push(tool_schema);
                    }
                }
            }

            let client = reqwest::blocking::Client::new();
            let mut system_prompt = "You are a sovereign intelligent agent. Your goal is to strictly follow instructions and satisfy constraints.".to_string();
            if !context_list.is_empty() {
                system_prompt.push_str("\n\nEPISODIC WORKING MEMORY CONTEXT:\n");
                for (idx, ctx_val) in context_list.iter().enumerate() {
                    system_prompt.push_str(&format!("[{}] {:?}\n", idx, ctx_val));
                }
            }

            let mut messages: Vec<JsonValue> = vec![
                json!({
                    "role": "system",
                    "content": system_prompt
                }),
                json!({"role": "user", "content": user_msg}),
            ];

            for _ in 0..5 {
                let mut body_obj = json!({
                    "messages": messages,
                    "max_tokens": 500,
                });

                if !oai_tools.is_empty() {
                    body_obj.as_object_mut().unwrap().insert("tools".to_string(), json!(oai_tools));
                    body_obj.as_object_mut().unwrap().insert("tool_choice".to_string(), json!("auto"));
                }

                if turn_type != Type::Any {
                    let mut schema = turn_type_to_json_schema(&turn_type);
                    if schema.get("type").and_then(|t| t.as_str()) != Some("object") {
                         schema = json!({
                            "type": "object",
                            "properties": {
                                "result": schema
                            },
                            "required": ["result"],
                            "additionalProperties": false
                         });
                    }
                    body_obj.as_object_mut().unwrap().insert(
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
                }

                println!("== LLM PAYLOAD ==\n{}\n==================", serde_json::to_string_pretty(&body_obj).unwrap_or_default());
                let resp: reqwest::blocking::Response = match client
                    .post(&url)
                    .header("api-key", &api_key)
                    .header("Content-Type", "application/json")
                    .json(&body_obj)
                    .send()
                {
                    Ok(r) => r,
                    Err(e) => return Ok(Value::Str(std::sync::Arc::new(format!("HTTP error: {}", e)))),
                };

                let status = resp.status();
                let text = resp.text().unwrap_or_default();
                if !status.is_success() {
                    return Ok(Value::Str(std::sync::Arc::new(format!("API error {}: {}", status, text))));
                }

                let j: JsonValue = match serde_json::from_str(&text) {
                    Ok(j) => j,
                    Err(_) => return Ok(Value::Str(std::sync::Arc::new(format!("Invalid JSON: {}", text)))),
                };

                let choice = j
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|a| a.first());
                let msg = match choice.and_then(|c| c.get("message")) {
                    Some(m) => m,
                    None => return Ok(Value::Str(std::sync::Arc::new("No message in response".to_string()))),
                };

                messages.push(msg.clone());

                let tool_calls = msg.get("tool_calls").and_then(|t| t.as_array());
                if tool_calls.map(|a| a.is_empty()).unwrap_or(true) {
                    let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

                    if turn_type != Type::Any {
                        match serde_json::from_str::<JsonValue>(content) {
                            Ok(parsed_json) => {
                                let actual_json = if turn_type_to_json_schema(&turn_type).get("type").and_then(|t| t.as_str()) != Some("object") {
                                    match parsed_json {
                                        JsonValue::Object(mut o) => o.remove("result").unwrap_or(JsonValue::Null),
                                        other => other,
                                    }
                                } else {
                                    parsed_json
                                };

                                match json_value_to_turn_value(&turn_type, &actual_json) {
                                    Ok(turn_val) => return Ok(turn_val),
                                    Err(err) => {
                                        messages.push(json!({
                                            "role": "user",
                                            "content": format!("Type mismatch warning. Your response did not match the strict schema. Error: {}. Please fix the JSON payload and strictly adhere to the schema.", err)
                                        }));
                                        continue;
                                    }
                                }
                            },
                            Err(e) => {
                                messages.push(json!({
                                    "role": "user",
                                    "content": format!("Invalid JSON response. Error: {}. Please output strictly valid JSON matching the schema.", e)
                                }));
                                continue;
                            }
                        }
                    } else {
                        return Ok(Value::Str(std::sync::Arc::new(content.to_string())));
                    }
                }

                for tc in tool_calls.unwrap_or(&vec![]) {
                    let name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    let args_str = tc
                        .get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                        .unwrap_or("{}");
                    let id = tc.get("id").and_then(|i| i.as_str()).unwrap_or("");

                    if name == "get_weather" {
                        let args: JsonValue = serde_json::from_str(args_str).unwrap_or(json!({}));
                        let lat = args.get("latitude").and_then(|v| v.as_f64()).unwrap_or(37.77);
                        let lon = args.get("longitude").and_then(|v| v.as_f64()).unwrap_or(-122.42);
                        let result_str = fetch_weather(lat, lon);

                        messages.push(json!({
                            "role": "tool",
                            "tool_call_id": id,
                            "content": result_str
                        }));
                    }
                }
            }

            Ok(Value::Str(std::sync::Arc::new("Max turns reached".to_string())))
        }) as ToolHandler,
    );
}
