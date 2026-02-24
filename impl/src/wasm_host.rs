use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::path::Path;
use std::time::Duration;
use wasmtime::*;

/// A loaded Wasm inference driver.
#[derive(Clone)]
pub struct WasmProvider {
    engine: Engine,
    module: Module,
}

impl WasmProvider {
    pub fn new(wasm_path: impl AsRef<Path>) -> Result<Self> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_path)?;
        Ok(Self { engine, module })
    }

    /// Complete pipeline:
    /// 1. Host evaluates Turn code -> JSON Inference Request
    /// 2. Wasm `transform_request` -> JSON HTTP Request Configuration
    /// 3. Host makes HTTP Request locally (reqwest)
    /// 4. Host gets HTTP Response -> Wasm `transform_response` -> JSON Inference Response
    pub fn execute_inference(&self, turn_request_json: &str) -> Result<String> {
        let mut store = Store::new(&self.engine, ());

        // Instantiate for this run. Purely computational, no host imports needed.
        let instance = Instance::new(&mut store, &self.module, &[])?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .context("Wasm module must export memory")?;

        // Extract the Wasm exports.
        // The provider SDK exports:
        // `alloc(u32) -> u32`
        // `transform_request(u32, u32) -> u64`
        // `transform_response(u32, u32) -> u64`
        let alloc = instance.get_typed_func::<u32, u32>(&mut store, "alloc")?;
        let transform_req =
            instance.get_typed_func::<(u32, u32), u64>(&mut store, "transform_request")?;
        let transform_res =
            instance.get_typed_func::<(u32, u32), u64>(&mut store, "transform_response")?;

        // 1. Transform Request
        let req_json = Self::call_wasm_string_func(
            &mut store,
            memory,
            &alloc,
            &transform_req,
            turn_request_json,
        )?;

        // Parse HTTP Request Config
        let http_req: serde_json::Value =
            serde_json::from_str(&req_json).context("Wasm returned invalid HTTP config JSON")?;

        // 2. Host HTTP Execution
        let http_res_json = Self::execute_http(http_req);

        // If HTTP failed locally (e.g. timeout), formulate a 500 error JSON for the Wasm to parse.
        let http_res_json = match http_res_json {
            Ok(json_str) => json_str,
            Err(e) => {
                let err_obj = serde_json::json!({
                    "status": 500,
                    "body": format!("Host network execution failed: {}", e),
                    "headers": {}
                });
                err_obj.to_string()
            }
        };

        // 3. Transform Response
        let final_json = Self::call_wasm_string_func(
            &mut store,
            memory,
            &alloc,
            &transform_res,
            &http_res_json,
        )?;

        Ok(final_json)
    }

    fn call_wasm_string_func(
        store: &mut Store<()>,
        memory: Memory,
        alloc: &TypedFunc<u32, u32>,
        func: &TypedFunc<(u32, u32), u64>,
        input: &str,
    ) -> Result<String> {
        let bytes = input.as_bytes();
        let len = bytes.len() as u32;

        let ptr = alloc.call(&mut *store, len)?;
        memory.write(&mut *store, ptr as usize, bytes)?;

        let packed = func.call(&mut *store, (ptr, len))?;

        let out_ptr = (packed >> 32) as u32;
        let out_len = (packed & 0xFFFFFFFF) as u32;

        let mut out_buf = vec![0; out_len as usize];
        memory.read(&*store, out_ptr as usize, &mut out_buf)?;

        Ok(String::from_utf8(out_buf)?)
    }

    fn execute_http(req: serde_json::Value) -> Result<String> {
        let client = Client::builder().timeout(Duration::from_secs(60)).build()?;

        let mut url = req["url"]
            .as_str()
            .context("Missing 'url' in HTTP config")?
            .to_string();
        url = Self::resolve_env_vars(&url);

        let method_str = req["method"].as_str().unwrap_or("POST");

        let method = match method_str.to_uppercase().as_str() {
            "GET" => reqwest::Method::GET,
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "DELETE" => reqwest::Method::DELETE,
            _ => reqwest::Method::POST,
        };

        let mut request = client.request(method, url);

        if let Some(headers) = req["headers"].as_object() {
            for (k, v) in headers {
                if let Some(s) = v.as_str() {
                    request = request.header(k, Self::resolve_env_vars(s));
                }
            }
        }

        if let Some(body) = req.get("body") {
            let body_str = if body.is_string() {
                body.as_str().unwrap().to_string()
            } else {
                body.to_string()
            };
            request = request.body(Self::resolve_env_vars(&body_str));
        }

        let response = request.send()?;
        let status = response.status().as_u16();

        let mut out = serde_json::Map::new();
        out.insert("status".to_string(), serde_json::json!(status));

        let mut headers_map = serde_json::Map::new();
        for (k, v) in response.headers().iter() {
            if let Ok(s) = v.to_str() {
                headers_map.insert(k.as_str().to_string(), serde_json::json!(s));
            }
        }
        out.insert(
            "headers".to_string(),
            serde_json::Value::Object(headers_map),
        );

        let body_text = response.text()?;
        out.insert("body".to_string(), serde_json::json!(body_text));

        Ok(serde_json::Value::Object(out).to_string())
    }

    /// Replaces occurrences of `$env:VAR_NAME` or `$env:VAR_NAME:DEFAULT` with actual environment variables
    fn resolve_env_vars(input: &str) -> String {
        let mut result = input.to_string();
        while let Some(start) = result.find("$env:") {
            let rest_idx = start + 5;
            let rest = &result[rest_idx..];

            // Find the end of the variable pattern. It ends at the first non-alphanumeric/underscore/colon character.
            let mut end_offset = 0;
            for c in rest.chars() {
                if !c.is_alphanumeric() && c != '_' && c != ':' {
                    break;
                }
                end_offset += c.len_utf8();
            }

            if end_offset == 0 {
                // Malformed, skip replacing this instance by breaking to prevent infinite loop
                break;
            }

            let var_pattern = &rest[..end_offset];
            let parts: Vec<&str> = var_pattern.splitn(2, ':').collect();
            let var_name = parts[0];
            let default_val = if parts.len() > 1 { parts[1] } else { "" };

            let val = std::env::var(var_name).unwrap_or_else(|_| default_val.to_string());

            result.replace_range(start..rest_idx + end_offset, &val);
        }
        result
    }
}
