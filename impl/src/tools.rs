//! Tool registry and handlers. Per spec: at least `echo` built-in.

use crate::value::Value;
use std::collections::HashMap;
use std::time::Duration;
use std::thread;
use std::env;

pub type ToolHandler = Box<dyn Fn(Value) -> Value + Send + Sync>;

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
            Box::new(|arg| arg) as ToolHandler,
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
                Value::Null
            }) as ToolHandler,
        );

        // http_get: Simple GET request
        tools.insert(
            "http_get".to_string(),
            Box::new(|arg| {
                let url = match arg {
                    Value::Str(s) => s,
                    _ => return Value::Null,
                };
                
                match reqwest::blocking::get(&url) {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.text() {
                                Ok(text) => Value::Str(text),
                                Err(_) => Value::Null,
                            }
                        } else {
                            Value::Null
                        }
                    },
                    Err(_) => Value::Null,
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
                            _ => return Value::Null,
                        };
                        let body = m.get("body").cloned().unwrap_or(Value::Null);
                        (url, body)
                    },
                    _ => return Value::Null,
                };

                let client = reqwest::blocking::Client::new();
                let json_body = serde_json::to_value(&body_val).unwrap_or(serde_json::Value::Null);

                match client.post(&url).json(&json_body).send() {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.text() {
                                Ok(text) => Value::Str(text),
                                Err(_) => Value::Null,
                            }
                        } else {
                            Value::Null
                        }
                    },
                    Err(_) => Value::Null,
                }
            }) as ToolHandler,
        );

        // llm_generate: Calls OpenAI Chat Completion
        // Arg: Map { "messages": [...], "model": "..." }
        tools.insert(
            "llm_generate".to_string(),
            Box::new(|arg| {
                let api_key = match env::var("OPENAI_API_KEY") {
                    Ok(k) => k,
                    Err(_) => return Value::Null, // Fail silently if no key
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
                    _ => return Value::Null,
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
                                        Value::Str(content.to_string())
                                    } else {
                                        Value::Null
                                    }
                                },
                                Err(_) => Value::Null,
                            }
                        } else {
                            Value::Null
                        }
                    },
                    Err(_) => Value::Null,
                }
            }) as ToolHandler,
        );

        // json_parse: Parses JSON string to Value
        tools.insert(
            "json_parse".to_string(),
            Box::new(|arg| {
                if let Value::Str(s) = arg {
                    match serde_json::from_str(&s) {
                        Ok(v) => v,
                        Err(_) => Value::Null,
                    }
                } else {
                    Value::Null
                }
            }) as ToolHandler,
        );

        // json_stringify: Converts Value to JSON string
        tools.insert(
            "json_stringify".to_string(),
            Box::new(|arg| {
                match serde_json::to_string(&arg) {
                    Ok(s) => Value::Str(s),
                    Err(_) => Value::Null,
                }
            }) as ToolHandler,
        );
        
        Self { tools }
    }

    pub fn register(&mut self, name: impl Into<String>, handler: ToolHandler) {
        self.tools.insert(name.into(), handler);
    }

    pub fn call(&self, name: &str, arg: Value) -> Option<Value> {
        self.tools.get(name).map(|h| h(arg))
    }

    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }
}
