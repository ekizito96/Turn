//! turn-provider-aws-anthropic
//! Anthropic Claude via AWS Bedrock inference driver for Turn.
//!
//! Required env vars:
//!   AWS_ACCESS_KEY_ID      — IAM access key
//!   AWS_SECRET_ACCESS_KEY  — IAM secret key
//!   AWS_REGION             — e.g. us-east-1
//!   BEDROCK_MODEL_ID       — e.g. anthropic.claude-3-5-sonnet-20241022-v2:0
//!
//! Protocol: JSON-RPC 2.0 over stdio. See PROVIDERS.md.

use chrono::Utc;
use hex;
use hmac::{Hmac, Mac};
use reqwest::blocking::Client;
use serde_json::{json, Value as JsonValue};
use sha2::{Digest, Sha256};
use std::env;
use std::io::{self, BufRead, Write};

type HmacSha256 = Hmac<Sha256>;

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn sigv4_signing_key(secret_key: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date    = hmac_sha256(format!("AWS4{}", secret_key).as_bytes(), date.as_bytes());
    let k_region  = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn call_bedrock_anthropic(
    access_key: &str,
    secret_key: &str,
    region: &str,
    model_id: &str,
    messages: &[JsonValue],
    schema: &JsonValue,
) -> Result<JsonValue, String> {
    let service = "bedrock";
    let host    = format!("bedrock-runtime.{}.amazonaws.com", region);
    let path    = format!("/model/{}/invoke", model_id);
    let url     = format!("https://{}{}", host, path);

    // Build request body (Anthropic Messages API format)
    let sys_content = messages.iter()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .and_then(|m| m.get("content").and_then(|c| c.as_str()))
        .unwrap_or("You are a helpful assistant.")
        .to_string();

    let user_messages: Vec<JsonValue> = messages.iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) != Some("system"))
        .cloned()
        .collect();

    let mut body = json!({
        "anthropic_version": "bedrock-2023-05-31",
        "max_tokens": 2048,
        "system": sys_content,
        "messages": user_messages,
    });

    if schema != &JsonValue::Null {
        let tool = json!({
            "name": "structured_output",
            "description": "Return the result conforming to the requested JSON schema.",
            "input_schema": schema
        });
        body.as_object_mut().unwrap().insert("tools".to_string(), json!([tool]));
        body.as_object_mut().unwrap().insert("tool_choice".to_string(), json!({"type": "tool", "name": "structured_output"}));
    }

    let payload = body.to_string();
    let payload_hash = sha256_hex(payload.as_bytes());

    let now = Utc::now();
    let amz_date  = now.format("%Y%m%dT%H%M%SZ").to_string();
    let date_str  = now.format("%Y%m%d").to_string();

    // Canonical request
    let canonical_headers = format!(
        "content-type:application/json\nhost:{}\nx-amz-date:{}\n",
        host, amz_date
    );
    let signed_headers = "content-type;host;x-amz-date";
    let canonical_req = format!(
        "POST\n{}\n\n{}\n{}\n{}",
        path, canonical_headers, signed_headers, payload_hash
    );

    // String to sign
    let cred_scope  = format!("{}/{}/{}/aws4_request", date_str, region, service);
    let str_to_sign = format!("AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date, cred_scope, sha256_hex(canonical_req.as_bytes()));

    // Signature
    let signing_key = sigv4_signing_key(secret_key, &date_str, region, service);
    let signature   = hex::encode(hmac_sha256(&signing_key, str_to_sign.as_bytes()));

    let auth_header = format!(
        "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
        access_key, cred_scope, signed_headers, signature
    );

    let client = Client::new();
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-amz-date", &amz_date)
        .header("Authorization", &auth_header)
        .body(payload)
        .send()
        .map_err(|e| format!("HTTP error: {}", e))?;

    let status = resp.status();
    let text   = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Bedrock API error {}: {}", status, text));
    }

    serde_json::from_str::<JsonValue>(&text).map_err(|_| format!("Invalid JSON: {}", text))
}

fn handle_request(req: JsonValue) {
    let req_id = req.get("id").cloned().unwrap_or(json!(0));
    let params = match req.get("params") {
        Some(p) => p,
        None => { send_error(req_id, "Missing 'params'"); return; }
    };

    let prompt  = params.get("prompt").and_then(|p| p.as_str()).unwrap_or("");
    let schema  = params.get("schema").cloned().unwrap_or(JsonValue::Null);
    let context = params.get("context").and_then(|c| c.as_array()).cloned().unwrap_or_default();

    let access_key = env::var("AWS_ACCESS_KEY_ID").unwrap_or_default();
    let secret_key = env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default();
    let region     = env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let model_id   = env::var("BEDROCK_MODEL_ID")
        .unwrap_or_else(|_| "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string());

    if access_key.is_empty() || secret_key.is_empty() {
        send_error(req_id, "Set AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY");
        return;
    }

    let sys = "You are a sovereign intelligent agent. Strictly follow instructions and satisfy all constraints.";
    let mut messages: Vec<JsonValue> = vec![json!({"role": "system", "content": sys})];

    if !context.is_empty() {
        let ctx = context.iter().enumerate()
            .map(|(i, v)| format!("[{}] {}", i, v.as_str().unwrap_or("")))
            .collect::<Vec<_>>().join("\n");
        messages.push(json!({"role":"system","content": format!("WORKING MEMORY:\n{}", ctx)}));
    }
    messages.push(json!({"role": "user", "content": prompt}));

    match call_bedrock_anthropic(&access_key, &secret_key, &region, &model_id, &messages, &schema) {
        Ok(j) => process_response(req_id, j, schema),
        Err(e) => send_error(req_id, &e),
    }
}

fn process_response(req_id: JsonValue, j: JsonValue, schema: JsonValue) {
    // Bedrock returns Anthropic-native format: content[].type = "tool_use" | "text"
    if let Some(content_arr) = j.get("content").and_then(|c| c.as_array()) {
        for block in content_arr {
            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                if let Some(input) = block.get("input") {
                    send_result(req_id, input.clone());
                    return;
                }
            }
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                if schema != JsonValue::Null {
                    match serde_json::from_str::<JsonValue>(text) {
                        Ok(v) => { send_result(req_id, v); return; }
                        Err(e) => { send_error(req_id, &format!("JSON parse error: {}", e)); return; }
                    }
                } else {
                    send_result(req_id, json!(text));
                    return;
                }
            }
        }
    }
    send_error(req_id, "Unrecognised response format from AWS Bedrock");
}

fn send_result(id: JsonValue, result: JsonValue) {
    println!("{}", json!({"jsonrpc":"2.0","result":result,"id":id}));
    io::stdout().flush().unwrap();
}

fn send_error(id: JsonValue, msg: &str) {
    println!("{}", json!({"jsonrpc":"2.0","error":msg,"id":id}));
    io::stdout().flush().unwrap();
}

fn main() {
    for line in io::stdin().lock().lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        if line.trim().is_empty() { continue; }
        if let Ok(req) = serde_json::from_str::<JsonValue>(&line) {
            if req.get("method").and_then(|m| m.as_str()) == Some("infer") {
                handle_request(req);
            } else {
                send_error(req.get("id").cloned().unwrap_or(json!(0)), "Unknown method");
            }
        }
    }
}
