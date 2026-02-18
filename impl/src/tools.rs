//! Tool registry and handlers. Per spec: at least `echo` built-in.

use crate::value::Value;
use std::collections::HashMap;
use std::time::Duration;
use std::thread;
use std::env;
use std::fs;

// Update ToolHandler to return Result<Value, String>
pub type ToolHandler = Box<dyn Fn(Value) -> Result<Value, String> + Send + Sync>;

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
        
        // echo: Identity function
        tools.insert(
            "echo".to_string(),
            Box::new(|arg| Ok(arg)) as ToolHandler,
        );

        // sleep: Pauses execution for N seconds
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
                Ok(Value::Null)
            }) as ToolHandler,
        );

        // fs_read: Read file content
        tools.insert(
            "fs_read".to_string(),
            Box::new(|arg| {
                let path = match arg {
                    Value::Str(s) => s,
                    _ => return Err("Argument must be a string path".to_string()),
                };
                match fs::read_to_string(&path) {
                    Ok(content) => Ok(Value::Str(content)),
                    Err(e) => Err(format!("Failed to read file {}: {}", path, e)),
                }
            }) as ToolHandler,
        );

        // fs_write: Write file content (arg: { path, content })
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
                    },
                    _ => return Err("Argument must be a map {path, content}".to_string()),
                };
                
                match fs::write(&path, &content) {
                    Ok(_) => Ok(Value::Null),
                    Err(e) => Err(format!("Failed to write file {}: {}", path, e)),
                }
            }) as ToolHandler,
        );

        // env_get: Get environment variable
        tools.insert(
            "env_get".to_string(),
            Box::new(|arg| {
                let key = match arg {
                    Value::Str(s) => s,
                    _ => return Err("Argument must be a string key".to_string()),
                };
                match env::var(&key) {
                    Ok(val) => Ok(Value::Str(val)),
                    Err(_) => Ok(Value::Null), // Typical env behavior is null/undefined if missing
                }
            }) as ToolHandler,
        );

        // env_set: Set environment variable (arg: { key, value })
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
                    },
                    _ => return Err("Argument must be a map {key, value}".to_string()),
                };
                env::set_var(key, val);
                Ok(Value::Null)
            }) as ToolHandler,
        );

        // http_get: Simple GET request
        tools.insert(
            "http_get".to_string(),
            Box::new(|arg| {
                let url = match arg {
                    Value::Str(s) => s,
                    _ => return Err("Argument must be a string URL".to_string()),
                };
                
                match reqwest::blocking::get(&url) {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.text() {
                                Ok(text) => Ok(Value::Str(text)),
                                Err(e) => Err(format!("Failed to read response text: {}", e)),
                            }
                        } else {
                            Err(format!("HTTP request failed with status: {}", resp.status()))
                        }
                    },
                    Err(e) => Err(format!("HTTP request error: {}", e)),
                }
            }) as ToolHandler,
        );

        // http_post: Simple POST request
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
                    },
                    _ => return Err("Argument must be a map {url, body}".to_string()),
                };

                let client = reqwest::blocking::Client::new();
                let json_body = serde_json::to_value(&body_val).unwrap_or(serde_json::Value::Null);

                match client.post(&url).json(&json_body).send() {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.text() {
                                Ok(text) => Ok(Value::Str(text)),
                                Err(e) => Err(format!("Failed to read response text: {}", e)),
                            }
                        } else {
                            Err(format!("HTTP request failed with status: {}", resp.status()))
                        }
                    },
                    Err(e) => Err(format!("HTTP request error: {}", e)),
                }
            }) as ToolHandler,
        );

        // llm_generate: Calls OpenAI Chat Completion
        tools.insert(
            "llm_generate".to_string(),
            Box::new(|arg| {
                let api_key = match env::var("OPENAI_API_KEY") {
                    Ok(k) => k,
                    Err(_) => return Err("OPENAI_API_KEY environment variable not set".to_string()),
                };

                let (messages, model) = match arg {
                    Value::Map(m) => {
                        let msgs = m.get("messages").cloned().unwrap_or(Value::List(vec![]));
                        let model = match m.get("model") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => "gpt-4o-mini".to_string(),
                        };
                        (msgs, model)
                    },
                    _ => return Err("Argument must be a map {messages, model?}".to_string()),
                };

                let client = reqwest::blocking::Client::new();
                let json_msgs = serde_json::to_value(&messages).unwrap_or(serde_json::Value::Array(vec![]));
                
                let payload = serde_json::json!({
                    "model": model,
                    "messages": json_msgs
                });

                match client.post("https://api.openai.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&payload)
                    .send() {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.json::<serde_json::Value>() {
                                Ok(json) => {
                                    if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                                        Ok(Value::Str(content.to_string()))
                                    } else {
                                        Err("Invalid response format from OpenAI".to_string())
                                    }
                                },
                                Err(e) => Err(format!("Failed to parse OpenAI response: {}", e)),
                            }
                        } else {
                            Err(format!("OpenAI API error: {}", resp.status()))
                        }
                    },
                    Err(e) => Err(format!("OpenAI request failed: {}", e)),
                }
            }) as ToolHandler,
        );

        // json_parse
        tools.insert(
            "json_parse".to_string(),
            Box::new(|arg| {
                if let Value::Str(s) = arg {
                    match serde_json::from_str(&s) {
                        Ok(v) => Ok(v),
                        Err(e) => Err(format!("JSON parse error: {}", e)),
                    }
                } else {
                    Err("json_parse expects a string argument".to_string())
                }
            }) as ToolHandler,
        );

        // json_stringify
        tools.insert(
            "json_stringify".to_string(),
            Box::new(|arg| {
                match serde_json::to_string(&arg) {
                    Ok(s) => Ok(Value::Str(s)),
                    Err(e) => Err(format!("JSON stringify error: {}", e)),
                }
            }) as ToolHandler,
        );

        // llm_infer: Handled by `infer` keyword
        // Returns Value::Uncertain based on schema
        tools.insert(
            "llm_infer".to_string(),
            Box::new(|arg| {
                 if let Value::Map(m) = arg {
                     let schema = m.get("schema").unwrap_or(&Value::Null);
                     // Simple Mock Logic
                     match schema {
                         Value::Str(s) if s.contains("Num") => {
                             Ok(Value::Uncertain(Box::new(Value::Num(42.0)), 0.85))
                         }
                         Value::Str(s) if s.contains("Bool") => {
                             Ok(Value::Uncertain(Box::new(Value::Bool(true)), 0.9))
                         }
                         Value::Str(s) if s.contains("Str") => {
                             Ok(Value::Uncertain(Box::new(Value::Str("Mock Response".to_string())), 0.7))
                         }
                         _ => Ok(Value::Uncertain(Box::new(Value::Null), 0.5)),
                     }
                 } else {
                     Err("Invalid args for llm_infer".to_string())
                 }
            }) as ToolHandler
        );
        
        Self { tools }
    }

    pub fn register(&mut self, name: impl Into<String>, handler: ToolHandler) {
        self.tools.insert(name.into(), handler);
    }

    pub fn call(&self, name: &str, arg: Value) -> Result<Value, String> {
        match self.tools.get(name) {
            Some(h) => h(arg),
            None => Err(format!("Tool not found: {}", name)),
        }
    }

    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}
