//! Tool registry and handlers. Per spec: at least `echo` built-in.

use crate::value::Value;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use std::process::Command;

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

        // echo
        tools.insert(
            "echo".to_string(),
            Box::new(|arg| {
                println!("{}", arg);
                Ok(arg)
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
                Ok(Value::Null)
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
                match fs::read_to_string(path.as_ref()) {
                    Ok(content) => Ok(Value::Str(std::sync::Arc::new(content))),
                    Err(e) => Err(format!("Failed to read file {}: {}", path, e)),
                }
            }) as ToolHandler,
        );

        // fs_read_blob
        tools.insert(
            "fs_read_blob".to_string(),
            Box::new(|arg| {
                let path = match arg {
                    Value::Str(s) => s,
                    _ => return Err("Argument must be a string path".to_string()),
                };
                match fs::read(path.as_ref()) {
                    Ok(bytes) => {
                        let mime_type = match std::path::Path::new(path.as_ref()).extension().and_then(|ext| ext.to_str()) {
                            Some("png") => "image/png",
                            Some("jpg") | Some("jpeg") => "image/jpeg",
                            Some("gif") => "image/gif",
                            Some("webp") => "image/webp",
                            Some("pdf") => "application/pdf",
                            Some("txt") | Some("md") | Some("csv") => "text/plain",
                            Some("mp3") => "audio/mpeg",
                            Some("wav") => "audio/wav",
                            Some("json") => "application/json",
                            _ => "application/octet-stream",
                        }.to_string();
                        Ok(Value::Blob {
                            mime_type,
                            data: std::sync::Arc::new(bytes),
                        })
                    }
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

                match fs::write(path.as_ref(), content.as_ref()) {
                    Ok(_) => Ok(Value::Null),
                    Err(e) => Err(format!("Failed to write file {}: {}", path, e)),
                }
            }) as ToolHandler,
        );
        // load_driver
        tools.insert(
            "load_driver".to_string(),
            Box::new(|arg| {
                let (path, config) = match arg {
                    Value::Map(m) => {
                        let path = match m.get("path") {
                            Some(Value::Str(s)) => s.clone(),
                            _ => return Err("Missing 'path' string in driver spec".to_string()),
                        };
                        let config = match m.get("config") {
                            Some(Value::Map(c)) => c.clone(),
                            _ => std::sync::Arc::new(indexmap::IndexMap::new()),
                        };
                        (path, config)
                    }
                    Value::Str(s) => (s, std::sync::Arc::new(indexmap::IndexMap::new())),
                    _ => return Err("Argument must be a path string or {path, config} map".to_string()),
                };

                // Inject config values securely into the current environment process context
                for (k, v) in config.iter() {
                    if let Value::Str(val) = v {
                        env::set_var(k, val.as_ref());
                    } else {
                        env::set_var(k, v.to_string());
                    }
                }

                Ok(Value::Str(path))
            }) as ToolHandler,
        );

        // env_get
        tools.insert(
            "env_get".to_string(),
            Box::new(|arg| {
                let key = match arg {
                    Value::Str(s) => s.clone(),
                    _ => return Err("env_get expects a string key".to_string()),
                };
                match env::var(key.as_ref()) {
                    Ok(val) => Ok(Value::Str(std::sync::Arc::new(val))),
                    Err(_) => Ok(Value::Null),
                }
            }) as ToolHandler,
        );

        // env_set
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
                env::set_var(key.as_ref(), val.as_ref());
                Ok(Value::Null)
            }) as ToolHandler,
        );

        // http_get
        tools.insert(
            "http_get".to_string(),
            Box::new(|arg| {
                let url = match arg {
                    Value::Str(s) => s,
                    _ => return Err("Argument must be a string URL".to_string()),
                };

                let mut cmd = Command::new("curl");
                cmd.arg("-s").arg("-L").arg(url.as_ref());

                // Inject auth and user agent for GitHub API compatibility
                if url.contains("github.com") {
                    cmd.arg("-H").arg("User-Agent: Turn Lang Agent");
                    if let Ok(token) = env::var("GITHUB_TOKEN") {
                         cmd.arg("-H").arg(format!("Authorization: token {}", token));
                    }
                }

                let output = cmd.output()
                    .map_err(|e| format!("Failed to execute curl: {}", e))?;

                if output.status.success() {
                    let text = String::from_utf8(output.stdout)
                        .map_err(|e| format!("Invalid UTF-8: {}", e))?;
                    Ok(Value::Str(std::sync::Arc::new(text)))
                } else {
                    Err(format!(
                        "HTTP request failed with status: {}",
                        output.status
                    ))
                }
            }) as ToolHandler,
        );

        // http_post
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

                let json_body =
                    serde_json::to_string(&body_val).unwrap_or_else(|_| "{}".to_string());

                let output = Command::new("curl")
                    .arg("-s")
                    .arg("-L")
                    .arg("-X")
                    .arg("POST")
                    .arg("-H")
                    .arg("Content-Type: application/json")
                    .arg("-d")
                    .arg(&json_body)
                    .arg(url.as_ref())
                    .output()
                    .map_err(|e| format!("Failed to execute curl: {}", e))?;

                if output.status.success() {
                    let text = String::from_utf8(output.stdout)
                        .map_err(|e| format!("Invalid UTF-8: {}", e))?;
                    Ok(Value::Str(std::sync::Arc::new(text)))
                } else {
                    Err(format!(
                        "HTTP request failed with status: {}",
                        output.status
                    ))
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
            Box::new(|arg| match serde_json::to_string(&arg) {
                Ok(s) => Ok(Value::Str(std::sync::Arc::new(s))),
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
                Ok(Value::Num(now.as_secs_f64()))
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
                Ok(Value::Bool(re.is_match(&text)))
            }) as ToolHandler,
        );

        // sys_exec
        // Phase 6c: CLI Domestication primitive boundary
        tools.insert(
            "sys_exec".to_string(),
            Box::new(|arg| {
                let args_str = match arg {
                    Value::Map(m) => {
                        let mut strings = Vec::new();
                        for v in m.values() {
                            match v {
                                Value::Str(s) => strings.push(s.to_string()),
                                _ => return Err("sys_exec map values must be strictly Strings".to_string()),
                            }
                        }
                        if strings.is_empty() {
                            return Err("sys_exec expects at least the binary name as the first map value".to_string());
                        }
                        strings
                    }
                    _ => return Err("sys_exec expects a Map of Strings: { 'bin': 'python3', 'args': '...' }".to_string()),
                };
                
                let binary = &args_str[0];
                let cmd_args = &args_str[1..];
                
                match Command::new(binary).args(cmd_args).output() {
                    Ok(output) => {
                        if output.status.success() {
                            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                            Ok(Value::Str(std::sync::Arc::new(stdout)))
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                            Err(format!("Process failed with status {}: {}", output.status, stderr))
                        }
                    }
                    Err(e) => Err(format!("Failed to execute '{}': {}", binary, e)),
                }
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
                Ok(Value::Str(std::sync::Arc::new(
                    re.replace_all(&text, replacement.as_str()).to_string(),
                )))
            }) as ToolHandler,
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
