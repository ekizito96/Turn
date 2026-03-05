use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
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
        let mut url = req["url"]
            .as_str()
            .context("Missing 'url' in HTTP config")?
            .to_string();
        url = Self::resolve_env_vars(&url);
        let method_str = req["method"].as_str().unwrap_or("POST");

        let mut cmd = Command::new("curl");
        let headers_file = tempfile::NamedTempFile::new()?;
        let headers_path = headers_file.path().to_str().unwrap();

        cmd.args(["-s", "-D", headers_path, "-X", method_str]);

        if let Some(headers) = req["headers"].as_object() {
            for (k, v) in headers {
                if let Some(s) = v.as_str() {
                    cmd.arg("-H");
                    cmd.arg(format!("{}: {}", k, Self::resolve_env_vars(s)));
                }
            }
        }

        if let Some(body) = req.get("body") {
            let body_str = if body.is_string() {
                body.as_str().unwrap().to_string()
            } else {
                body.to_string()
            };
            cmd.arg("-d");
            cmd.arg(Self::resolve_env_vars(&body_str));
        }

        cmd.arg(&url);

        let output = cmd.output()?;
        let body_part = String::from_utf8_lossy(&output.stdout).to_string();

        let header_part = std::fs::read_to_string(headers_path).unwrap_or_default();

        // Extract HTTP status code from the last block of headers (in case of 100 Continue)
        let mut status = 200;
        let mut headers_map = serde_json::Map::new();

        // Split by \r\n\r\n to handle multiple header blocks (e.g., 100 Continue)
        let blocks: Vec<&str> = header_part.split("\r\n\r\n").collect();
        let last_header_block = blocks
            .iter()
            .rev()
            .find(|b| !b.trim().is_empty())
            .unwrap_or(&"");

        let mut lines = last_header_block.lines();
        if let Some(status_line) = lines.next() {
            status = status_line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(200);
        }

        let mut out = serde_json::Map::new();
        out.insert("status".to_string(), serde_json::json!(status));

        for line in lines {
            if let Some((k, v)) = line.split_once(':') {
                headers_map.insert(k.trim().to_lowercase(), serde_json::json!(v.trim()));
            }
        }
        out.insert(
            "headers".to_string(),
            serde_json::Value::Object(headers_map),
        );
        out.insert("body".to_string(), serde_json::json!(body_part.clone()));

        println!("[WASM_HOST DEBUG] Raw Azure body:\n{}", body_part);

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
