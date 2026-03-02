//! Tool registry and handlers. Per spec: at least `echo` built-in.

use crate::value::Value;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// --- LLM Helpers ---

fn call_openai_chat(
    api_key: &str,
    base_url: &str,
    model: &str,
    messages: &serde_json::Value,
) -> Result<(String, u64), String> {
    let client = reqwest::blocking::Client::new();
    let payload = serde_json::json!({
        "model": model,
        "messages": messages
    });

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    match client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
    {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>() {
                    Ok(json) => {
                        if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                            let tokens = json["usage"]["total_tokens"].as_u64().unwrap_or(0);
                            Ok((content.to_string(), tokens))
                        } else {
                            Err("Invalid response format from OpenAI-compatible API".to_string())
                        }
                    }
                    Err(e) => Err(format!("Failed to parse response: {}", e)),
                }
            } else {
                Err(format!("API error: {}", resp.status()))
            }
        }
        Err(e) => Err(format!("Request failed: {}", e)),
    }
}

fn call_anthropic_chat(
    api_key: &str,
    model: &str,
    messages: &serde_json::Value,
) -> Result<(String, u64), String> {
    let client = reqwest::blocking::Client::new();

    let mut system_prompt = String::new();
    let mut anthropic_msgs = Vec::new();

    if let Some(arr) = messages.as_array() {
        for msg in arr {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

            if role == "system" {
                if !system_prompt.is_empty() {
                    system_prompt.push('\n');
                }
                system_prompt.push_str(content);
            } else {
                anthropic_msgs.push(msg.clone());
            }
        }
    }

    let mut payload = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "messages": anthropic_msgs
    });

    if !system_prompt.is_empty() {
        payload["system"] = serde_json::Value::String(system_prompt);
    }

    match client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
    {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>() {
                    Ok(json) => {
                        if let Some(content_arr) = json["content"].as_array() {
                            if let Some(text) =
                                content_arr.first().and_then(|item| item["text"].as_str())
                            {
                                let input_tok = json["usage"]["input_tokens"].as_u64().unwrap_or(0);
                                let output_tok =
                                    json["usage"]["output_tokens"].as_u64().unwrap_or(0);
                                Ok((text.to_string(), input_tok + output_tok))
                            } else {
                                Err("Invalid content format from Anthropic".to_string())
                            }
                        } else {
                            Err("Missing content array from Anthropic".to_string())
                        }
                    }
                    Err(e) => Err(format!("Failed to parse Anthropic response: {}", e)),
                }
            } else {
                let status = resp.status();
                let text = resp.text().unwrap_or_default();
                Err(format!("Anthropic API error {}: {}", status, text))
            }
        }
        Err(e) => Err(format!("Anthropic request failed: {}", e)),
    }
}

fn call_google_chat(
    api_key: &str,
    model: &str,
    messages: &serde_json::Value,
) -> Result<(String, u64), String> {
    let client = reqwest::blocking::Client::new();

    // Google Gemini API: https://generativelanguage.googleapis.com/v1beta/models/{model}:generateConten

    let mut contents = Vec::new();
    let mut system_instruction = None;

    if let Some(arr) = messages.as_array() {
        for msg in arr {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let text = msg.get("content").and_then(|c| c.as_str()).unwrap_or("");

            if role == "system" {
                // Gemini supports system_instruction at top level
                system_instruction = Some(serde_json::json!({
                    "parts": [{ "text": text }]
                }));
            } else {
                // Map roles: user -> user, assistant -> model
                let gemini_role = if role == "assistant" { "model" } else { "user" };
                contents.push(serde_json::json!({
                    "role": gemini_role,
                    "parts": [{ "text": text }]
                }));
            }
        }
    }

    let mut payload = serde_json::json!({
        "contents": contents
    });

    if let Some(sys) = system_instruction {
        payload["system_instruction"] = sys;
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    match client.post(&url).json(&payload).send() {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>() {
                    Ok(json) => {
                        // Response: { candidates: [ { content: { parts: [ { text: "..." } ] } } ] }
                        if let Some(candidates) = json["candidates"].as_array() {
                            if let Some(first) = candidates.first() {
                                if let Some(parts) = first["content"]["parts"].as_array() {
                                    if let Some(text) =
                                        parts.first().and_then(|p| p["text"].as_str())
                                    {
                                        let tokens = json["usageMetadata"]["totalTokenCount"]
                                            .as_u64()
                                            .unwrap_or(0);
                                        return Ok((text.to_string(), tokens));
                                    }
                                }
                            }
                        }
                        Err("Invalid response format from Gemini".to_string())
                    }
                    Err(e) => Err(format!("Failed to parse Gemini response: {}", e)),
                }
            } else {
                Err(format!("Gemini API error: {}", resp.status()))
            }
        }
        Err(e) => Err(format!("Gemini request failed: {}", e)),
    }
}

fn call_ollama_chat(model: &str, messages: &serde_json::Value) -> Result<(String, u64), String> {
    let client = reqwest::blocking::Client::new();

    let host = env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
    let url = format!("{}/api/chat", host);

    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": false
    });

    match client.post(&url).json(&payload).send() {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>() {
                    Ok(json) => {
                        if let Some(content) = json["message"]["content"].as_str() {
                            let tokens = json["eval_count"].as_u64().unwrap_or(0)
                                + json["prompt_eval_count"].as_u64().unwrap_or(0);
                            Ok((content.to_string(), tokens))
                        } else {
                            Err("Invalid response format from Ollama".to_string())
                        }
                    }
                    Err(e) => Err(format!("Failed to parse Ollama response: {}", e)),
                }
            } else {
                Err(format!("Ollama API error: {}", resp.status()))
            }
        }
        Err(e) => Err(format!("Ollama request failed: {}", e)),
    }
}

fn call_azure_chat(
    api_key: &str,
    endpoint: &str,
    deployment: &str,
    api_version: &str,
    messages: &serde_json::Value,
) -> Result<(String, u64), String> {
    let client = reqwest::blocking::Client::new();
    // Azure URL: {endpoint}/openai/deployments/{deployment}/chat/completions?api-version=...
    // Note: endpoint usually includes https://
    let base = endpoint.trim_end_matches('/');
    let url = format!(
        "{}/openai/deployments/{}/chat/completions?api-version={}",
        base, deployment, api_version
    );

    let payload = serde_json::json!({
        "messages": messages
    });

    match client
        .post(&url)
        .header("api-key", api_key)
        .json(&payload)
        .send()
    {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>() {
                    Ok(json) => {
                        if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
                            let tokens = json["usage"]["total_tokens"].as_u64().unwrap_or(0);
                            Ok((content.to_string(), tokens))
                        } else {
                            Err("Invalid response format from Azure OpenAI".to_string())
                        }
                    }
                    Err(e) => Err(format!("Failed to parse response: {}", e)),
                }
            } else {
                let status = resp.status();
                let text = resp.text().unwrap_or_default();
                Err(format!("Azure API error {}: {}", status, text))
            }
        }
        Err(e) => Err(format!("Request failed: {}", e)),
    }
}

fn call_azure_responses(
    api_key: &str,
    responses_url: &str,
    model: &str,
    messages: &serde_json::Value,
) -> Result<(String, u64), String> {
    let client = reqwest::blocking::Client::new();
    let payload = serde_json::json!({
        "model": model,
        "messages": messages,
        "max_completion_tokens": 16384
    });

    match client
        .post(responses_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
    {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<serde_json::Value>() {
                    Ok(json) => {
                        let tokens = json["usage"]["total_tokens"].as_u64().unwrap_or(0);
                        if let Some(text) = json.get("output_text").and_then(|v| v.as_str()) {
                            return Ok((text.to_string(), tokens));
                        }
                        if let Some(text) = json["output"][0]["content"][0]["text"].as_str() {
                            return Ok((text.to_string(), tokens));
                        }
                        if let Some(text) = json["choices"][0]["message"]["content"].as_str() {
                            return Ok((text.to_string(), tokens));
                        }
                        Err("Invalid response format from Azure Responses API".to_string())
                    }
                    Err(e) => Err(format!("Failed to parse response: {}", e)),
                }
            } else {
                let status = resp.status();
                let text = resp.text().unwrap_or_default();
                Err(format!("Azure Responses API error {}: {}", status, text))
            }
        }
        Err(e) => Err(format!("Request failed: {}", e)),
    }
}

fn call_llm_dispatch(
    model_hint: Option<&str>,
    messages: &serde_json::Value,
) -> Result<(String, u64), String> {
    let provider = env::var("TURN_LLM_PROVIDER")
        .unwrap_or_else(|_| "openai".to_string())
        .to_lowercase();
    let env_model = env::var("TURN_LLM_MODEL").ok();
    let model = model_hint.or(env_model.as_deref());

    match provider.as_str() {
        "azure" => {
            let api_key = env::var("AZURE_OPENAI_KEY")
                .or_else(|_| env::var("AZURE_API_KEY"))
                .or_else(|_| env::var("AZURE_OPENAI_API_KEY"))
                .map_err(|_| "AZURE_OPENAI_KEY or AZURE_API_KEY not set".to_string())?;

            // If full Responses API URL is provided, use it directly.
            if let Ok(responses_url) = env::var("AZURE_OPENAI_RESPONSES_URL") {
                let final_model = model.unwrap_or("gpt-5.2-codex");
                return call_azure_responses(&api_key, &responses_url, final_model, messages);
            }

            let endpoint = env::var("AZURE_OPENAI_ENDPOINT")
                .map_err(|_| "AZURE_OPENAI_ENDPOINT not set".to_string())?;
            let api_version = env::var("AZURE_OPENAI_API_VERSION")
                .unwrap_or_else(|_| "2024-12-01-preview".to_string());

            // Allow passing a full responses URL in AZURE_OPENAI_ENDPOINT too.
            if endpoint.contains("/openai/responses") {
                let final_model = model.unwrap_or("gpt-5.2-codex");
                return call_azure_responses(&api_key, &endpoint, final_model, messages);
            }

            // For deployment path mode, model maps to deployment name.
            let deployment = model.unwrap_or("gpt-5.2-chat");
            call_azure_chat(&api_key, &endpoint, deployment, &api_version, messages)
        }
        "anthropic" => {
            let api_key = env::var("ANTHROPIC_API_KEY")
                .map_err(|_| "ANTHROPIC_API_KEY not set".to_string())?;
            let final_model = model.unwrap_or("claude-3-opus-20240229");
            call_anthropic_chat(&api_key, final_model, messages)
        }
        "google" | "gemini" => {
            let api_key = env::var("GEMINI_API_KEY")
                .or_else(|_| env::var("GOOGLE_API_KEY"))
                .map_err(|_| "GEMINI_API_KEY or GOOGLE_API_KEY not set".to_string())?;
            let final_model = model.unwrap_or("gemini-1.5-pro");
            call_google_chat(&api_key, final_model, messages)
        }
        "grok" => {
            let api_key = env::var("GROK_API_KEY")
                .or_else(|_| env::var("XAI_API_KEY"))
                .map_err(|_| "GROK_API_KEY or XAI_API_KEY not set".to_string())?;
            let final_model = model.unwrap_or("grok-1");
            call_openai_chat(&api_key, "https://api.x.ai/v1", final_model, messages)
        }
        "vllm" | "openrouter" | "deepseek" | "openai-generic" => {
            let api_key = env::var("LLM_API_KEY")
                .or_else(|_| env::var("OPENAI_API_KEY"))
                .map_err(|_| "LLM_API_KEY or OPENAI_API_KEY not set".to_string())?;

            // Default to local vLLM if no base url
            let base_url = env::var("TURN_LLM_API_BASE")
                .unwrap_or_else(|_| "http://localhost:8000/v1".to_string());
            let final_model = model.unwrap_or("default");
            call_openai_chat(&api_key, &base_url, final_model, messages)
        }
        "ollama" => {
            let final_model = model.unwrap_or("llama3");
            call_ollama_chat(final_model, messages)
        }
        _ => {
            let api_key =
                env::var("OPENAI_API_KEY").map_err(|_| "OPENAI_API_KEY not set".to_string())?;
            let final_model = model.unwrap_or("gpt-4o-mini");
            call_openai_chat(&api_key, "https://api.openai.com/v1", final_model, messages)
        }
    }
}

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
                let (messages, model_opt) = match arg {
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

                match call_llm_dispatch(model_opt.as_deref(), &json_msgs) {
                    Ok((content, tokens)) => Ok((Value::Str(content), tokens)),
                    Err(e) => Err(e),
                }
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

        // json_stringify
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

                     let mut msgs = Vec::new();

                     // Helper to extract struct info from Debug string
                     // Struct("Name", {"field": Type, ...})
                     let mut json_hint = String::new();
                     if let Value::Str(s) = schema {
                         if s.contains("Struct") {
                             if let Some(start) = s.find('{') {
                                 if let Some(end) = s.rfind('}') {
                                     let fields_str = &s[start+1..end];
                                     // "score": Num, "reasoning": Str
                                     let mut field_hints = Vec::new();
                                     for part in fields_str.split(',') {
                                         let part = part.trim();
                                         if let Some(idx) = part.find(':') {
                                             let key = part[..idx].trim().trim_matches('"');
                                             let ty_raw = part[idx+1..].trim();
                                             let example_val = if ty_raw.contains("Num") { "0.8" }
                                                             else if ty_raw.contains("Str") { "\"example\"" }
                                                             else if ty_raw.contains("Bool") { "true" }
                                                             else { "null" };
                                             field_hints.push(format!("\"{}\": {}", key, example_val));
                                         }
                                     }
                                     if !field_hints.is_empty() {
                                         json_hint = format!("{{ {} }}", field_hints.join(", "));
                                     }
                                 }
                             }
                         }
                     }

                     let sys_msg = if !json_hint.is_empty() {
                         format!("You are a Turn Language Runtime. The user wants a value matching the provided schema.\n\nREQUIRED OUTPUT FORMAT:\n{{ \"value\": {}, \"confidence\": <0.0-1.0> }}\n\nEnsure 'value' is a VALID JSON object matching the example structure EXACTLY, filling fields based on the Prompt.", json_hint)
                     } else {
                         "You are a Turn Language Runtime. The user wants a value matching the provided schema. \n\nIMPORTANT: If the schema describes a Struct (e.g. Struct(\"Name\", { fields... })), you MUST return a JSON object for the 'value' field that strictly matches those fields.\n\nOutput ONLY JSON object: { \"value\": <value>, \"confidence\": <0.0-1.0> }.".to_string()
                     };

                     msgs.push(serde_json::json!({
                         "role": "system",
                         "content": sys_msg
                     }));

                     if let Value::List(items) = context {
                         for item in items {
                             if let Value::Str(s) = item {
                                 msgs.push(serde_json::json!({
                                     "role": "system",
                                     "content": format!("Context: {}", s)
                                 }));
                             }
                         }
                     }

                     let user_content = format!("Schema Type: {}\nPrompt: {}", schema, prompt);
                     msgs.push(serde_json::json!({
                         "role": "user",
                         "content": user_content
                     }));

                     let messages = serde_json::Value::Array(msgs);

                     println!("Calling LLM Dispatch with messages: {}", messages);

                     match call_llm_dispatch(None, &messages) {
                         Ok((content, tokens)) => {
                             println!("LLM Reply: {}", content);
                             let clean = content.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```").trim();
                             match serde_json::from_str::<serde_json::Value>(clean) {
                                 Ok(json) => {
                                     let val_json = json.get("value").unwrap_or(&serde_json::Value::Null);
                                     let conf = json.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.9);

                                     // If schema is Struct, try to parse val_json as Struc
                                     let turn_val = if let Value::Str(s) = schema {
                                         if s.contains("Struct") {
                                             // Best-effort struct parsing from JSON objec
                                             match val_json {
                                                 serde_json::Value::Object(map) => {
                                                     let mut fields = indexmap::IndexMap::new();
                                                     for (k, v) in map {
                                                         let tv: Value = serde_json::from_value(v.clone()).unwrap_or(Value::Null);
                                                         fields.insert(k.clone(), tv);
                                                     }
                                                     // Extract struct name from schema string "Struct(\"Name\", ...)"
                                                     let name = s.split('"').nth(1).unwrap_or("Anon").to_string();
                                                     Value::Struct(name, fields)
                                                 },
                                                 _ => serde_json::from_value(val_json.clone()).unwrap_or(Value::Null)
                                             }
                                         } else {
                                             serde_json::from_value(val_json.clone()).unwrap_or(Value::Null)
                                         }
                                     } else {
                                         serde_json::from_value(val_json.clone()).unwrap_or(Value::Null)
                                     };

                                     Ok((Value::Uncertain(Box::new(turn_val), conf), tokens))
                                 },
                                 Err(e) => {
                                    println!("Failed to parse LLM JSON: {} in '{}'", e, clean);
                                    Err(format!("Failed to parse LLM JSON: {} in '{}'", e, clean))
                                 },
                             }
                         },
                         Err(e) => {
                             println!("LLM Dispatch failed: {}", e);
                             // Fallback to Mock
                             match schema {
                                 Value::Str(s) if s.contains("Num") => {
                                     Ok((Value::Uncertain(Box::new(Value::Num(42.0)), 0.85), 0u64))
                                 }
                                 Value::Str(s) if s.contains("Bool") => {
                                     Ok((Value::Uncertain(Box::new(Value::Bool(true)), 0.9), 0u64))
                                 }
                                 Value::Str(s) if s.contains("Str") => {
                                     Ok((Value::Uncertain(Box::new(Value::Str("Mock Response".to_string())), 0.7), 0u64))
                                 }
                                 _ => Ok((Value::Uncertain(Box::new(Value::Null), 0.5), 0u64)),
                             }
                         }
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
