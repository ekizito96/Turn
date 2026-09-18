#![allow(dead_code, clippy::missing_safety_doc)]
// turn-provider-openai/src/lib.rs
use serde::Deserialize;
use serde_json::{json, Value};

/// Allocate memory in the Wasm guest for the host to write strings into.
#[no_mangle]
pub extern "C" fn alloc(len: u32) -> u32 {
    let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as usize as u32
}

/// Takes an input pointer and length, recreates the Vec to avoid leaking, and converts to a Rust String.
unsafe fn read_string(ptr: u32, len: u32) -> String {
    let buf = Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize);
    String::from_utf8_lossy(&buf).into_owned()
}

/// Packs a String into a (ptr, len) u64 to return to the host.
fn pack_string(s: String) -> u64 {
    let len = s.len() as u64;
    let mut buf = s.into_bytes();
    let ptr = buf.as_mut_ptr() as u64;
    std::mem::forget(buf);
    (ptr << 32) | len
}

#[derive(Deserialize)]
struct TurnInferRequest {
    jsonrpc: String,
    method: String,
    params: InferParams,
    id: u32,
}

#[derive(Deserialize)]
struct InferParams {
    prompt: Value,
    schema: Value,
    context: Vec<Value>,
    tools: Vec<Value>,
}

fn turn_to_openai_content(v: &Value) -> Value {
    if let Value::Object(m) = v {
        if m.get("_turn_blob").is_some() {
            let mime = m
                .get("mime_type")
                .and_then(|m| m.as_str())
                .unwrap_or("image/jpeg");
            let data = m.get("data").and_then(|m| m.as_str()).unwrap_or("");
            return json!([{
                "type": "image_url",
                "image_url": {
                    "url": format!("data:{};base64,{}", mime, data)
                }
            }]);
        }
    } else if let Value::Array(arr) = v {
        let mut content = Vec::new();
        for item in arr {
            if let Value::Object(m) = item {
                if m.get("_turn_blob").is_some() {
                    let mime = m
                        .get("mime_type")
                        .and_then(|m| m.as_str())
                        .unwrap_or("image/jpeg");
                    let data = m.get("data").and_then(|m| m.as_str()).unwrap_or("");
                    content.push(json!({
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:{};base64,{}", mime, data)
                        }
                    }));
                    continue;
                }
            }
            let text = if let Value::String(s) = item {
                s.clone()
            } else {
                item.to_string()
            };
            content.push(json!({
                "type": "text",
                "text": text
            }));
        }
        return json!(content);
    }

    let text = if let Value::String(s) = v {
        s.clone()
    } else {
        v.to_string()
    };
    json!(text)
}

/// Phase 1: Host -> Wasm -> Host (HTTP Config)
#[no_mangle]
pub unsafe extern "C" fn transform_request(ptr: u32, len: u32) -> u64 {
    let req_str = read_string(ptr, len);

    // We expect a valid JSON-RPC standard Turn request.
    let req: TurnInferRequest = match serde_json::from_str(&req_str) {
        Ok(r) => r,
        Err(e) => {
            return pack_string(
                json!({ "error": format!("Invalid Turn Request: {}", e) }).to_string(),
            )
        }
    };

    // The host guarantees env vars via some injection mechanism, but for purely Wasm
    // standard, maybe the host evaluates env vars inside `llm_tools` and passes them?
    // Wait, Wasm doesn't have `std::env::var` by default unless WASI is used.
    // We didn't enable WASI. If we don't enable WASI, `std::env::var` will panic.
    // Let's modify Wasm to just tell the Host *which* env vars to inject into headers.
    // E.g. { "url": "...", "headers": { "Authorization": { "$env": "OPENAI_API_KEY" } } }

    // Actually, setting WASI is easy (`wasmtime_wasi`), but passing credentials in the request payload is simpler!
    // Let's assume the Host passes `{ "credentials": { ... }, "request": { ... } }` into `transform_request`.
    // But we already defined the payload to just be the `rpc_request` from `llm_tools.rs`.
    // Let's map it safely without env vars inside Wasm:
    // Wasm returns the HTTP Config. The Host *knows* this is OpenAI, so the Host can attach `OPENAI_API_KEY`.
    // Wait, the WHOLE POINT of the driver is that the Host doesn't know it's OpenAI!
    // The Driver says: "Host, please make a request to api.openai.com, and please read the OPENAI_API_KEY env var from your secure context and attach it as Bearer."

    let sys_msg = "You are a cognitive runtime inference engine mapped to the Turn language. You must return pure JSON matching the user's schema.";

    let mut openai_tools = Vec::new();
    for t in req.params.tools {
        openai_tools.push(t);
    }

    let mut messages = Vec::new();
    messages.push(json!({"role": "system", "content": sys_msg}));
    for ctx in req.params.context {
        messages.push(json!({"role": "system", "content": turn_to_openai_content(&ctx)}));
    }
    messages.push(json!({"role": "user", "content": turn_to_openai_content(&req.params.prompt)}));

    let mut body = json!({
        "model": "$env:OPENAI_MODEL:gpt-4o", // Host resolves this template
        "messages": messages,
        "logprobs": true,
    });

    if req.params.schema != json!({"type": "any"}) {
        body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": {
                "name": "turn_schema",
                "schema": req.params.schema,
                "strict": true
            }
        });
    }

    if !openai_tools.is_empty() {
        body["tools"] = Value::Array(openai_tools);
    }

    let http_config = json!({
        "url": "https://api.openai.com/v1/chat/completions",
        "method": "POST",
        "headers": {
            "Content-Type": "application/json",
            "Authorization": "Bearer $env:OPENAI_API_KEY", // Host resolves this
        },
        "body": body
    });

    pack_string(http_config.to_string())
}

#[derive(Deserialize)]
struct HostHttpResponse {
    status: u16,
    body: String,
    // headers: Value
}

/// Byte span of a single sampled token, plus the log probability the model assigned it.
struct TokenSpan {
    start: usize,
    end: usize,
    logprob: f64,
}

/// `choices[0].logprobs.content` lists tokens in emission order and their byte
/// lengths concatenate to exactly the message content, so offsets are exact.
fn token_spans(entries: &[Value]) -> Vec<TokenSpan> {
    let mut spans = Vec::new();
    let mut offset = 0usize;
    for entry in entries {
        let len = entry
            .get("bytes")
            .and_then(|b| b.as_array())
            .map(|a| a.len())
            .or_else(|| entry.get("token").and_then(|t| t.as_str()).map(|s| s.len()))
            .unwrap_or(0);
        let logprob = entry.get("logprob").and_then(|l| l.as_f64()).unwrap_or(0.0);
        spans.push(TokenSpan {
            start: offset,
            end: offset + len,
            logprob,
        });
        offset += len;
    }
    spans
}

/// Reads a JSON string starting at the opening quote. Returns the key and the index past the closing quote.
fn scan_string(bytes: &[u8], mut i: usize) -> Option<(String, usize)> {
    if *bytes.get(i)? != b'"' {
        return None;
    }
    i += 1;
    let start = i;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => {
                let raw = String::from_utf8_lossy(&bytes[start..i]).into_owned();
                let unescaped = raw.replace("\\\"", "\"").replace("\\\\", "\\");
                return Some((unescaped, i + 1));
            }
            _ => i += 1,
        }
    }
    None
}

/// Returns the index one past the end of the JSON value starting at `i`.
fn scan_value(bytes: &[u8], i: usize) -> Option<usize> {
    match *bytes.get(i)? {
        b'"' => scan_string(bytes, i).map(|(_, end)| end),
        b'{' | b'[' => {
            let mut depth = 0usize;
            let mut in_string = false;
            let mut j = i;
            while j < bytes.len() {
                if in_string {
                    match bytes[j] {
                        b'\\' => j += 1,
                        b'"' => in_string = false,
                        _ => {}
                    }
                } else {
                    match bytes[j] {
                        b'"' => in_string = true,
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(j + 1);
                            }
                        }
                        _ => {}
                    }
                }
                j += 1;
            }
            None
        }
        _ => {
            let mut j = i;
            while j < bytes.len() && bytes[j] != b',' && bytes[j] != b'}' && bytes[j] != b']' {
                j += 1;
            }
            while j > i && bytes[j - 1].is_ascii_whitespace() {
                j -= 1;
            }
            Some(j)
        }
    }
}

/// Byte spans of each top-level field's *value* in a JSON object.
fn top_level_field_spans(src: &str) -> Vec<(String, usize, usize)> {
    let bytes = src.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() && bytes[i] != b'{' {
        i += 1;
    }
    if i >= bytes.len() {
        return spans;
    }
    i += 1;
    loop {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'}' {
            break;
        }
        let (key, after_key) = match scan_string(bytes, i) {
            Some(v) => v,
            None => break,
        };
        i = after_key;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b':' {
            break;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        let end = match scan_value(bytes, i) {
            Some(e) => e,
            None => break,
        };
        spans.push((key, start, end));
        i = end;
    }
    spans
}

/// Perplexity-normalised probability over the tokens overlapping `[start, end)`.
fn span_confidence(tokens: &[TokenSpan], start: usize, end: usize) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0usize;
    for token in tokens {
        if token.start < end && token.end > start {
            sum += token.logprob;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    Some((sum / count as f64).exp())
}

/// Maps token log probabilities onto the fields of a structured completion.
///
/// Under `strict` JSON schema the braces, keys and separators are schema-forced
/// and score near certainty, so they are excluded from every average. Only the
/// tokens that make up field values are measured.
fn confidence_map(content: &str, logprob_entries: &[Value]) -> Option<Value> {
    if logprob_entries.is_empty() {
        return None;
    }
    let tokens = token_spans(logprob_entries);
    let fields = top_level_field_spans(content);

    let mut map = serde_json::Map::new();
    let mut value_sum = 0.0;
    let mut value_count = 0usize;

    for (key, start, end) in &fields {
        if let Some(p) = span_confidence(&tokens, *start, *end) {
            map.insert(key.clone(), json!(p));
        }
        for token in &tokens {
            if token.start < *end && token.end > *start {
                value_sum += token.logprob;
                value_count += 1;
            }
        }
    }

    let overall = if value_count > 0 {
        (value_sum / value_count as f64).exp()
    } else {
        let total: f64 = tokens.iter().map(|t| t.logprob).sum();
        (total / tokens.len() as f64).exp()
    };
    map.insert("_overall".to_string(), json!(overall));

    Some(Value::Object(map))
}

/// Phase 2: Host (HTTP Response) -> Wasm -> Host (Turn Response)
#[no_mangle]
pub unsafe extern "C" fn transform_response(ptr: u32, len: u32) -> u64 {
    let res_str = read_string(ptr, len);

    let http_res: HostHttpResponse = match serde_json::from_str(&res_str) {
        Ok(r) => r,
        Err(_) => return pack_string(
            json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid HTTP response format from Host"})
                .to_string(),
        ),
    };

    if http_res.status != 200 {
        return pack_string(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": format!("HTTP {}: {}", http_res.status, http_res.body)
            })
            .to_string(),
        );
    }

    let gpt_json: Value = match serde_json::from_str(&http_res.body) {
        Ok(v) => v,
        Err(e) => return pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": format!("Failed to parse OpenAI response: {}", e)}).to_string()),
    };

    if let Some(err) = gpt_json.get("error") {
        return pack_string(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "error": err.to_string()
            })
            .to_string(),
        );
    }

    if let Some(choices) = gpt_json.get("choices").and_then(|c| c.as_array()) {
        if choices.is_empty() {
            return pack_string(
                json!({"jsonrpc": "2.0", "id": 1, "error": "No choices in response"}).to_string(),
            );
        }
        let message = &choices[0]["message"];

        if let Some(tools) = message.get("tool_calls").and_then(|t| t.as_array()) {
            if !tools.is_empty() {
                let t = &tools[0];
                return pack_string(
                    json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "tool_call",
                        "params": {
                            "name": t["function"]["name"].as_str().unwrap_or(""),
                            "arguments": t["function"]["arguments"].as_str().unwrap_or("{}")
                        }
                    })
                    .to_string(),
                );
            }
        }

        let content = message["content"].as_str().unwrap_or("");
        let parsed_result: Value = serde_json::from_str(content).unwrap_or_else(|_| json!(content));

        let logprob_entries = choices[0]
            .pointer("/logprobs/content")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();

        let mut response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": parsed_result
        });

        // Absent when the model does not support logprobs. The host then treats
        // the result as unmeasured rather than inventing a confidence.
        if let Some(confidence) = confidence_map(content, &logprob_entries) {
            response["confidence"] = confidence;
        }

        if let Some(usage) = gpt_json.get("usage") {
            response["usage"] = usage.clone();
        }

        pack_string(response.to_string())
    } else {
        pack_string(
            json!({"jsonrpc": "2.0", "id": 1, "error": "Invalid structure from OpenAI"})
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One token per byte, so spans line up with string indices.
    fn tokens_per_byte(content: &str, logprob_at: impl Fn(usize) -> f64) -> Vec<Value> {
        content
            .bytes()
            .enumerate()
            .map(|(i, b)| {
                json!({
                    "token": String::from_utf8_lossy(&[b]).into_owned(),
                    "logprob": logprob_at(i),
                    "bytes": [b],
                })
            })
            .collect()
    }

    #[test]
    fn field_spans_cover_values_not_keys() {
        let src = r#"{"name":"Acme","revenue":42}"#;
        let spans = top_level_field_spans(src);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].0, "name");
        assert_eq!(&src[spans[0].1..spans[0].2], "\"Acme\"");
        assert_eq!(spans[1].0, "revenue");
        assert_eq!(&src[spans[1].1..spans[1].2], "42");
    }

    #[test]
    fn field_spans_handle_nested_objects_and_arrays() {
        let src = r#"{"a":{"b":[1,2],"c":"}"},"d":null}"#;
        let spans = top_level_field_spans(src);
        assert_eq!(spans.len(), 2);
        assert_eq!(&src[spans[0].1..spans[0].2], r#"{"b":[1,2],"c":"}"}"#);
        assert_eq!(&src[spans[1].1..spans[1].2], "null");
    }

    #[test]
    fn confidence_is_per_field_and_ignores_schema_forced_tokens() {
        let content = r#"{"name":"Acme","revenue":42}"#;
        // Certain name, quarter-probability revenue, and deliberately terrible
        // scores on the structural tokens the schema forced anyway.
        let quarter = 0.25f64.ln();
        let entries = tokens_per_byte(content, |i| match i {
            8..=13 => 0.0,
            25..=26 => quarter,
            _ => -10.0,
        });

        let map = confidence_map(content, &entries).expect("logprobs present");
        let name = map["name"].as_f64().unwrap();
        let revenue = map["revenue"].as_f64().unwrap();
        let overall = map["_overall"].as_f64().unwrap();

        assert!((name - 1.0).abs() < 1e-9);
        assert!((revenue - 0.25).abs() < 1e-9);
        // Mean over the eight value tokens only, not the twenty structural ones.
        assert!((overall - 0.25f64.powf(0.25)).abs() < 1e-9);
    }

    #[test]
    fn no_logprobs_means_no_confidence() {
        assert!(confidence_map(r#"{"a":1}"#, &[]).is_none());
    }

    #[test]
    fn non_object_result_falls_back_to_whole_sequence() {
        let content = "\"hello\"";
        let entries = tokens_per_byte(content, |_| 0.5f64.ln());
        let map = confidence_map(content, &entries).expect("logprobs present");
        assert!(map.get("_overall").is_some());
        assert!((map["_overall"].as_f64().unwrap() - 0.5).abs() < 1e-9);
    }
}
