use crate::{compiler::Compiler, lexer::Lexer, parser::Parser, value::Value, vm::Vm};

pub async fn expand_schema_macro(arg: Value) -> anyhow::Result<Value> {
    let map = match arg {
        Value::Struct(_, map) => map,
        Value::Map(map) => map,
        _ => anyhow::bail!("sys_schema_adapter expected Map argument"),
    };

    let protocol_val = map.get("protocol").ok_or_else(|| anyhow::anyhow!("Missing protocol"))?;
    let url_val = map.get("url").ok_or_else(|| anyhow::anyhow!("Missing url"))?;

    let protocol: &str = match protocol_val {
        Value::Str(s) => s.as_ref(),
        _ => anyhow::bail!("sys_schema_adapter protocol must be string"),
    };

    let url = match url_val {
        Value::Str(s) => s.to_string(),
        _ => anyhow::bail!("sys_schema_adapter url must be string"),
    };

    // We simulate the Wasm Compilation Engine.
    // Instead of executing the Wasm binary, we natively parse the Schema and 
    // generate pure Turn source code, which is then dynamically parsed and compiled
    // back into the existing VM module context.
    // This allows Zero-Context Bloat natively typed LLM bindings.

    if protocol == "openapi" {
        let response = reqwest::get(&url).await.map_err(|e| anyhow::anyhow!("Failed to fetch schema: {}", e))?;
        let text = response.text().await?;
        let spec: serde_json::Value = serde_json::from_str(&text)?;

        let mut turn_code = String::new();
        let mut exported_methods = Vec::new();
        
        // Base URL handling
        let mut base_url = url.clone();
        if let Some(servers) = spec.get("servers").and_then(|s| s.as_array()) {
            if let Some(first_server) = servers.first() {
                if let Some(s_url) = first_server.get("url").and_then(|u| u.as_str()) {
                    base_url = s_url.to_string();
                }
            }
        }

        if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
            for (path, methods) in paths {
                if let Some(get_op) = methods.get("get").and_then(|m| m.as_object()) {
                    if let Some(op_id) = get_op.get("operationId").and_then(|id| id.as_str()) {
                        
                        let safe_id = op_id.replace('-', "_");
                        let full_url = format!("{}{}", base_url, path);
                        
                        // Generate a native Turn tool function
                        turn_code.push_str(&format!(r#"
let {} = turn {{
    // Simulated fetch hook for Turn Agent execution
    return "Fetched data from {}";
}};
"#, safe_id, full_url));
                        exported_methods.push(safe_id);
                    }
                }
            }
        }

        // Generate the return module struct
        turn_code.push_str("return {\n");
        for method in &exported_methods {
            turn_code.push_str(&format!(r#"    "{}": {},"#, method, method));
            turn_code.push('\n');
        }
        turn_code.push_str("};\n");

        // Now we compile the synthesized Turn code into a native Value block
        let lexer = Lexer::new(&turn_code);
        let tokens = lexer.tokenize().map_err(|e| anyhow::anyhow!("Schema macro synteny error: {}", e))?;
        let mut parser = Parser::new(tokens);
        let program = parser.parse().map_err(|e| anyhow::anyhow!("Schema macro parse error: {}", e))?;
        let mut compiler = Compiler::new();
        let code = compiler.compile(&program);
        
        // We evaluate strictly dynamically inside a sandbox VM to produce the closure structure
        let mut vm = Vm::new(&code);
        
        let mut result_val = Value::Null;
        loop {
            match vm.run() {
                crate::vm::VmResult::Complete(result) => {
                    result_val = result;
                    break;
                }
                crate::vm::VmResult::Error(error) => {
                    anyhow::bail!("Schema macro evaluation error: {}", error);
                }
                _ => break, // We don't expect suspends here as it's purely declarative
            }
        }
        
        return Ok(result_val);
    }

    anyhow::bail!("Unsupported schema protocol: {}", protocol);
}