//! Tool registry and handlers. Per spec: at least `echo` built-in.

use crate::value::Value;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Update ToolHandler to return Result<Value, String>
pub type ToolHandler = Box<dyn Fn(Value) -> Result<(Value, u64), String> + Send + Sync>;

pub struct ToolRegistry {
    tools: HashMap<String, ToolHandler>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut tools = HashMap::new();

        // echo
        tools.insert(
            "echo".to_string(),
            Box::new(|arg| {
                println!("{}", arg);
                Ok((arg, 0u64))
            }) as ToolHandler,
        );

        // sleep
        tools.insert(
            "sleep".to_string(),
            Box::new(|arg| {
                let seconds = match arg {
                    Value::Num(n) => n,
                    _ => 0.0,
                };
                if seconds > 0.0 {
                    thread::sleep(Duration::from_secs_f64(seconds));
                }
                Ok((Value::Null, 0u64))
            }) as ToolHandler,
        );

        // fs_read
        tools.insert(
            "fs_read".to_string(),
            Box::new(|arg| {
                let path = match arg {
                    Value::Str(s) => s,
                    _ => return Err("Argument must be a string path".to_string()),
                };
                match fs::read_to_string(&path) {
                    Ok(content) => Ok((Value::Str(content), 0u64)),
                    Err(e) => Err(format!("Failed to read file {}: {}", path, e)),
                }
            }) as ToolHandler,
        );

        // fs_write
        tools.insert(
            "fs_write".to_string(),
            Box::new(|arg| {
                let (path, content) = match arg {
                    Value::Map(m) => {
                        let path = match m.get("path") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'path' in argument map".to_string()),
                        };
                        let content = match m.get("content") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'content' in argument map".to_string()),
                        };
                        (path, content)
                    }
                    _ => return Err("Argument must be a map {path, content}".to_string()),
                };

                match fs::write(&path, &content) {
                    Ok(_) => Ok((Value::Null, 0u64)),
                    Err(e) => Err(format!("Failed to write file {}: {}", path, e)),
                }
            }) as ToolHandler,
        );

        // env_ge
        tools.insert(
            "env_get".to_string(),
            Box::new(|arg| {
                let key = match arg {
                    Value::Str(s) => s,
                    _ => return Err("Argument must be a string key".to_string()),
                };
                match env::var(&key) {
                    Ok(val) => Ok((Value::Str(val), 0u64)),
                    Err(_) => Ok((Value::Null, 0u64)),
                }
            }) as ToolHandler,
        );

        // env_se
        tools.insert(
            "env_set".to_string(),
            Box::new(|arg| {
                let (key, val) = match arg {
                    Value::Map(m) => {
                        let k = match m.get("key") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'key' in argument map".to_string()),
                        };
                        let v = match m.get("value") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'value' in argument map".to_string()),
                        };
                        (k, v)
                    }
                    _ => return Err("Argument must be a map {key, value}".to_string()),
                };
                env::set_var(key, val);
                Ok((Value::Null, 0u64))
            }) as ToolHandler,
        );

        // http_ge
        tools.insert(
            "http_get".to_string(),
            Box::new(|arg| {
                let url = match arg {
                    Value::Str(s) => s,
                    _ => return Err("Argument must be a string URL".to_string()),
                };

                let client = reqwest::blocking::Client::builder()
                    .user_agent("TurnLang/1.0 (https://turn-lang.dev)")
                    .build()
                    .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

                match client.get(&url).send() {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.text() {
                                Ok(text) => Ok((Value::Str(text), 0u64)),
                                Err(e) => Err(format!("Failed to read response text: {}", e)),
                            }
                        } else {
                            Err(format!(
                                "HTTP request failed with status: {}",
                                resp.status()
                            ))
                        }
                    }
                    Err(e) => Err(format!("HTTP request error: {}", e)),
                }
            }) as ToolHandler,
        );

        // http_pos
        tools.insert(
            "http_post".to_string(),
            Box::new(|arg| {
                let (url, body_val) = match arg {
                    Value::Map(m) => {
                        let url = match m.get("url") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'url'".to_string()),
                        };
                        let body = m.get("body").cloned().unwrap_or(Value::Null);
                        (url, body)
                    }
                    _ => return Err("Argument must be a map {url, body}".to_string()),
                };

                let client = reqwest::blocking::Client::new();
                let json_body = serde_json::to_value(&body_val).unwrap_or(serde_json::Value::Null);

                match client.post(&url).json(&json_body).send() {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.text() {
                                Ok(text) => Ok((Value::Str(text), 0u64)),
                                Err(e) => Err(format!("Failed to read response text: {}", e)),
                            }
                        } else {
                            Err(format!(
                                "HTTP request failed with status: {}",
                                resp.status()
                            ))
                        }
                    }
                    Err(e) => Err(format!("HTTP request error: {}", e)),
                }
            }) as ToolHandler,
        );

        // llm_generate
        tools.insert(
            "llm_generate".to_string(),
            Box::new(|arg| {
                let (messages, _model_opt) = match arg {
                    Value::Map(m) => {
                        let msgs = m.get("messages").cloned().unwrap_or(Value::List(vec![]));
                        let model = match m.get("model") {
                            Some(Value::Str(s)) => Some(s.clone()),
                            _ => None,
                        };
                        (msgs, model)
                    }
                    _ => return Err("Argument must be a map {messages, model?}".to_string()),
                };

                let json_msgs =
                    serde_json::to_value(&messages).unwrap_or(serde_json::Value::Array(vec![]));

                let provider =
                    env::var("TURN_LLM_PROVIDER").unwrap_or_else(|_| "openai".to_string());
                if provider == "mock" {
                    return Ok((Value::Str("Mock response".to_string()), 10));
                }

                let wasm_file = format!("{}_provider.wasm", provider);
                let mut wasm_path = std::path::Path::new(".turn_modules").join(&wasm_file);

                if !wasm_path.exists() {
                    let mut p = std::env::current_dir().unwrap_or_default();
                    for _ in 0..10 {
                        let check = p.join(".turn_modules").join(&wasm_file);
                        if check.exists() {
                            wasm_path = check;
                            break;
                        }
                        if !p.pop() {
                            break;
                        }
                    }
                }

                if wasm_path.exists() {
                    let mut params = serde_json::Map::new();
                    // for llm_generate we just wrap messages as a prompt
                    params.insert("prompt".to_string(), json_msgs);
                    params.insert("schema".to_string(), serde_json::json!({"type": "any"}));
                    params.insert("context".to_string(), serde_json::json!([]));
                    params.insert("tools".to_string(), serde_json::json!([]));

                    let req = serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "llm_infer",
                        "params": params,
                        "id": 2
                    });

                    match crate::wasm_host::WasmProvider::new(&wasm_path) {
                        Ok(wasm_provider) => {
                            match wasm_provider.execute_inference(&req.to_string()) {
                                Ok(json_res) => {
                                    let parsed: serde_json::Value = serde_json::from_str(&json_res)
                                        .unwrap_or(serde_json::Value::Null);
                                    if let Some(err) = parsed.get("error") {
                                        return Err(format!("WASM Driver Error: {}", err));
                                    }
                                    let content = parsed["choices"][0]["message"]["content"]
                                        .as_str()
                                        .unwrap_or("{}");
                                    let tokens =
                                        parsed["usage"]["total_tokens"].as_u64().unwrap_or(0);
                                    return Ok((Value::Str(content.to_string()), tokens));
                                }
                                Err(e) => return Err(format!("WASM Execution failed: {}", e)),
                            }
                        }
                        Err(e) => return Err(format!("Failed to load WASM provider: {}", e)),
                    }
                }

                Err(format!("WASM driver not found at {}", wasm_path.display()))
            }) as ToolHandler,
        );

        // json_parse
        tools.insert(
            "json_parse".to_string(),
            Box::new(|arg| {
                if let Value::Str(s) = arg {
                    match serde_json::from_str(&s) {
                        Ok(v) => Ok((v, 0u64)),
                        Err(e) => Err(format!("JSON parse error: {}", e)),
                    }
                } else {
                    Err("json_parse expects a string argument".to_string())
                }
            }) as ToolHandler,
        );

        // len
        tools.insert(
            "len".to_string(),
            Box::new(|arg| match arg {
                Value::List(items) => Ok((Value::Num(items.len() as f64), 0u64)),
                Value::Str(s) => Ok((Value::Num(s.len() as f64), 0u64)),
                Value::Map(entries) => Ok((Value::Num(entries.len() as f64), 0u64)),
                Value::Vec(items) => Ok((Value::Num(items.len() as f64), 0u64)),
                _ => Ok((Value::Num(0.0), 0u64)),
            }) as ToolHandler,
        );

        tools.insert(
            "list_push".to_string(),
            Box::new(|arg| {
                if let Value::List(args) = arg {
                    if args.len() == 2 {
                        if let Value::List(mut items) = args[0].clone() {
                            items.push(args[1].clone());
                            return Ok((Value::List(items), 0u64));
                        }
                    }
                }
                Ok((Value::Null, 0u64))
            }) as ToolHandler,
        );

        tools.insert(
            "list_contains".to_string(),
            Box::new(|arg| {
                if let Value::List(args) = arg {
                    if args.len() == 2 {
                        if let Value::List(items) = &args[0] {
                            let contains = items.contains(&args[1]);
                            return Ok((Value::Bool(contains), 0u64));
                        }
                    }
                }
                Ok((Value::Bool(false), 0u64))
            }) as ToolHandler,
        );

        tools.insert(
            "list_map".to_string(),
            Box::new(|_arg| {
                // ...
                Ok((Value::Null, 0u64)) // Handled in VM or compiler natively.
            }) as ToolHandler,
        );
        tools.insert(
            "json_stringify".to_string(),
            Box::new(|arg| match serde_json::to_string(&arg) {
                Ok(s) => Ok((Value::Str(s), 0u64)),
                Err(e) => Err(format!("JSON stringify error: {}", e)),
            }) as ToolHandler,
        );

        // time_now
        tools.insert(
            "time_now".to_string(),
            Box::new(|_arg| {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| format!("System time error: {}", e))?;
                Ok((Value::Num(now.as_secs_f64()), 0u64))
            }) as ToolHandler,
        );

        // regex_match
        tools.insert(
            "regex_match".to_string(),
            Box::new(|arg| {
                let (pattern, text) = match arg {
                    Value::Map(m) => {
                        let pattern = match m.get("pattern") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'pattern' in argument map".to_string()),
                        };
                        let text = match m.get("text") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'text' in argument map".to_string()),
                        };
                        (pattern, text)
                    }
                    _ => return Err("Argument must be a map {pattern, text}".to_string()),
                };

                let re =
                    Regex::new(&pattern).map_err(|e| format!("Invalid regex pattern: {}", e))?;
                Ok((Value::Bool(re.is_match(&text)), 0u64))
            }) as ToolHandler,
        );

        // regex_replace
        tools.insert(
            "regex_replace".to_string(),
            Box::new(|arg| {
                let (pattern, text, replacement) = match arg {
                    Value::Map(m) => {
                        let pattern = match m.get("pattern") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'pattern' in argument map".to_string()),
                        };
                        let text = match m.get("text") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'text' in argument map".to_string()),
                        };
                        let replacement = match m.get("replacement") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'replacement' in argument map".to_string()),
                        };
                        (pattern, text, replacement)
                    }
                    _ => {
                        return Err(
                            "Argument must be a map {pattern, text, replacement}".to_string()
                        )
                    }
                };

                let re =
                    Regex::new(&pattern).map_err(|e| format!("Invalid regex pattern: {}", e))?;
                Ok((
                    Value::Str(re.replace_all(&text, replacement.as_str()).to_string()),
                    0u64,
                ))
            }) as ToolHandler,
        );

        // llm_infer
        tools.insert(
            "llm_infer".to_string(),
            Box::new(|arg| {
                if let Value::Map(m) = arg {
                    let schema = m.get("schema").unwrap_or(&Value::Null);
                    let prompt = m.get("prompt").unwrap_or(&Value::Null);
                    let context = m.get("context").unwrap_or(&Value::Null);

                    // 1. Try WASM Provider first (The Architecturally Correct Way)
                    let provider =
                        env::var("TURN_LLM_PROVIDER").unwrap_or_else(|_| "openai".to_string());

                    if provider == "mock" {
                        match schema {
                            Value::Str(s) if s.contains("Num") => {
                                return Ok((
                                    Value::Uncertain(Box::new(Value::Num(42.0)), 0.85),
                                    0u64,
                                ));
                            }
                            Value::Str(s) if s.contains("Bool") => {
                                return Ok((
                                    Value::Uncertain(Box::new(Value::Bool(true)), 0.9),
                                    0u64,
                                ));
                            }
                            Value::Str(s) if s.contains("Str") => {
                                return Ok((
                                    Value::Uncertain(
                                        Box::new(Value::Str("Mock Response".to_string())),
                                        0.7,
                                    ),
                                    0u64,
                                ));
                            }
                            _ => {
                                let map = indexmap::IndexMap::new();
                                let mock_val = Value::Struct("Mock".to_string(), map);
                                return Ok((Value::Uncertain(Box::new(mock_val), 1.0), 10));
                            }
                        }
                    }

                    let wasm_file = format!("{}_provider.wasm", provider);
                    let mut wasm_path = std::path::Path::new(".turn_modules").join(&wasm_file);

                    if !wasm_path.exists() {
                        let mut p = std::env::current_dir().unwrap_or_default();
                        for _ in 0..10 {
                            let check = p.join(".turn_modules").join(&wasm_file);
                            if check.exists() {
                                wasm_path = check;
                                break;
                            }
                            if !p.pop() {
                                break;
                            }
                        }
                    }

                    if wasm_path.exists() {
                        println!("🔌 Using WASM Inference Driver: {}", wasm_path.display());

                        // WASM drivers expect a JSON-RPC TurnInferRequest matching the exact inputs
                        let mut params = serde_json::Map::new();
                        params.insert(
                            "prompt".to_string(),
                            serde_json::to_value(prompt).unwrap_or(serde_json::Value::Null),
                        );
                        params.insert(
                            "schema".to_string(),
                            serde_json::to_value(schema).unwrap_or(serde_json::Value::Null),
                        );
                        params.insert(
                            "context".to_string(),
                            serde_json::to_value(context).unwrap_or(serde_json::json!([])),
                        );
                        if let Some(tools_val) = m.get("tools") {
                            params.insert(
                                "tools".to_string(),
                                serde_json::to_value(tools_val).unwrap_or(serde_json::json!([])),
                            );
                        } else {
                            params.insert("tools".to_string(), serde_json::json!([]));
                        }

                        let req = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "llm_infer",
                            "params": params,
                            "id": 1
                        });

                        match crate::wasm_host::WasmProvider::new(&wasm_path) {
                            Ok(wasm_provider) => {
                                match wasm_provider.execute_inference(&req.to_string()) {
                                    Ok(json_res) => {
                                        let parsed: serde_json::Value =
                                            serde_json::from_str(&json_res)
                                                .unwrap_or(serde_json::Value::Null);

                                        // If it's an error from the driver
                                        if let Some(err) = parsed.get("error") {
                                            return Err(format!("WASM Driver Error: {}", err));
                                        }

                                        // WASM provides standard OpenAI format output
                                        let content = parsed["choices"][0]["message"]["content"]
                                            .as_str()
                                            .unwrap_or("{}");
                                        let tokens =
                                            parsed["usage"]["total_tokens"].as_u64().unwrap_or(0);

                                        let raw_json: serde_json::Value =
                                            match serde_json::from_str(content) {
                                                Ok(v) => v,
                                                Err(_) => {
                                                    let cleaned = content
                                                        .trim()
                                                        .trim_start_matches("```json")
                                                        .trim_start_matches("```")
                                                        .trim_end_matches("```")
                                                        .trim();
                                                    serde_json::from_str(cleaned)
                                                        .unwrap_or(serde_json::Value::Null)
                                                }
                                            };

                                        let turn_val = if let Value::Str(s) = schema {
                                            if s.contains("Struct") {
                                                match raw_json {
                                                    serde_json::Value::Object(map) => {
                                                        let mut fields = indexmap::IndexMap::new();
                                                        for (k, v) in map {
                                                            let tv: Value =
                                                                serde_json::from_value(v.clone())
                                                                    .unwrap_or(Value::Null);
                                                            fields.insert(k.clone(), tv);
                                                        }
                                                        let name = s
                                                            .split('"')
                                                            .nth(1)
                                                            .unwrap_or("Anon")
                                                            .to_string();
                                                        Value::Struct(name, fields)
                                                    }
                                                    _ => serde_json::from_value(raw_json)
                                                        .unwrap_or(Value::Null),
                                                }
                                            } else {
                                                serde_json::from_value(raw_json)
                                                    .unwrap_or(Value::Null)
                                            }
                                        } else {
                                            serde_json::from_value(raw_json).unwrap_or(Value::Null)
                                        };

                                        return Ok((
                                            Value::Uncertain(Box::new(turn_val), 0.95),
                                            tokens,
                                        ));
                                    }
                                    Err(e) => return Err(format!("WASM Execution failed: {}", e)),
                                }
                            }
                            Err(e) => return Err(format!("Failed to load WASM provider: {}", e)),
                        }
                    }

                    Err(format!("WASM driver not found at {}", wasm_path.display()))
                } else {
                    Err("Invalid args for llm_infer".to_string())
                }
            }) as ToolHandler,
        );

        Self { tools }
    }

    pub fn register(&mut self, name: impl Into<String>, handler: ToolHandler) {
        self.tools.insert(name.into(), handler);
    }

    pub fn call(&self, name: &str, arg: Value) -> Result<(Value, u64), String> {
        match self.tools.get(name) {
            Some(h) => h(arg),
            None => Err(format!("Tool not found: {}", name)),
        }
    }

    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}
