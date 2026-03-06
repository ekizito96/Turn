use crate::{compiler::Compiler, lexer::Lexer, parser::Parser, value::Value, vm::Vm};

pub fn expand_ast(program: &mut crate::ast::Program) -> anyhow::Result<()> {
    for stmt in program.stmts.iter_mut() {
        expand_stmt(stmt)?;
    }
    Ok(())
}

fn expand_stmt(stmt: &mut crate::ast::Stmt) -> anyhow::Result<()> {
    use crate::ast::Stmt;
    match stmt {
        Stmt::Turn { body, .. } => {
            for s in &mut body.stmts {
                expand_stmt(s)?;
            }
        }
        Stmt::Let { init, .. } => {
            expand_expr(init)?;
        }
        Stmt::ContextAppend { expr, .. } => {
            expand_expr(expr)?;
        }
        Stmt::ContextSystem { expr, .. } => {
            expand_expr(expr)?;
        }
        Stmt::Remember { key, value, .. } => {
            expand_expr(key)?;
            expand_expr(value)?;
        }
        Stmt::ExprStmt { expr, .. } => {
            expand_expr(expr)?;
        }
        Stmt::While { cond, body, .. } => {
            expand_expr(cond)?;
            for s in &mut body.stmts {
                expand_stmt(s)?;
            }
        }
        Stmt::Return { expr, .. } => {
            expand_expr(expr)?;
        }
        Stmt::ImplDef { methods, .. } => {
            for m in methods {
                expand_stmt(m)?;
            }
        }
        Stmt::TryCatch {
            try_block,
            catch_block,
            ..
        } => {
            for s in &mut try_block.stmts {
                expand_stmt(s)?;
            }
            for s in &mut catch_block.stmts {
                expand_stmt(s)?;
            }
        }
        Stmt::Throw { expr, .. } => {
            expand_expr(expr)?;
        }
        _ => {}
    }
    Ok(())
}

fn expand_expr(expr: &mut crate::ast::Expr) -> anyhow::Result<()> {
    use crate::ast::Expr;
    match expr {
        Expr::Binary { left, right, .. } => {
            expand_expr(left)?;
            expand_expr(right)?;
        }
        Expr::Unary { expr: inner, .. } => {
            expand_expr(inner)?;
        }
        Expr::Paren(inner) => {
            expand_expr(inner)?;
        }
        Expr::Call { name, args, .. } => {
            expand_expr(name)?;
            for arg in args {
                expand_expr(arg)?;
            }
        }
        Expr::MethodCall { target, args, .. } => {
            expand_expr(target)?;
            for arg in args {
                expand_expr(arg)?;
            }
        }
        Expr::Index { target, index, .. } => {
            expand_expr(target)?;
            expand_expr(index)?;
        }
        Expr::List { items, .. } | Expr::Vec { items, .. } => {
            for item in items {
                expand_expr(item)?;
            }
        }
        Expr::Map { entries, .. } => {
            for (_, val) in entries {
                expand_expr(val)?;
            }
        }
        Expr::StructInit { fields, .. } => {
            for (_, val) in fields {
                expand_expr(val)?;
            }
        }
        Expr::If {
            cond,
            then_block,
            else_block,
            ..
        } => {
            expand_expr(cond)?;
            for s in &mut then_block.stmts {
                expand_stmt(s)?;
            }
            if let Some(e) = else_block {
                for s in &mut e.stmts {
                    expand_stmt(s)?;
                }
            }
        }
        Expr::Turn { body, .. } => {
            for s in &mut body.stmts {
                expand_stmt(s)?;
            }
        }
        Expr::Confidence { expr: val, .. } => {
            expand_expr(val)?;
        }
        Expr::Infer { body, .. } => {
            for s in &mut body.stmts {
                expand_stmt(s)?;
            }
        }
        Expr::Recall { key, .. } => {
            expand_expr(key)?;
        }
        Expr::Use { module, .. } => {
            expand_expr(module)?;
        }
        Expr::UseSchema {
            protocol,
            url,
            span,
        } => {
            if let Expr::Literal {
                value: crate::ast::Literal::Str(url_str),
                ..
            } = &**url
            {
                // Compile-time expansion
                let expanded_stmts = compile_schema_to_ast(protocol, url_str)?;

                // Replace this Expr::UseSchema with an IIFE that returns the module
                let body = crate::ast::Block {
                    stmts: expanded_stmts,
                    span: *span,
                };

                let closure = Box::new(Expr::Turn {
                    params: Vec::new(),
                    ret_ty: None,
                    body,
                    span: *span,
                });

                *expr = Expr::Call {
                    name: closure,
                    args: Vec::new(),
                    span: *span,
                };
            } else {
                anyhow::bail!("use schema url must be a literal string for deep static analysis");
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn compile_schema_to_ast(protocol: &str, url: &str) -> anyhow::Result<Vec<crate::ast::Stmt>> {
    if protocol != "openapi" {
        anyhow::bail!("Unsupported protocol: {}", protocol);
    }

    // Try fetching if it's a URL, otherwise read from file system
    let text = if url.starts_with("http") {
        reqwest::blocking::get(url)
            .map_err(|e| anyhow::anyhow!("Failed to fetch schema: {}", e))?
            .text()?
    } else {
        std::fs::read_to_string(url)
            .map_err(|e| anyhow::anyhow!("Failed to read schema file: {}", e))?
    };

    let spec: serde_json::Value = serde_json::from_str(&text)?;

    let mut turn_code = String::new();
    let mut exported_methods = Vec::new();

    // Parse components.schemas into Turn structs
    if let Some(components) = spec.get("components").and_then(|c| c.as_object()) {
        if let Some(schemas) = components.get("schemas").and_then(|s| s.as_object()) {
            for (schema_name, schema_def) in schemas {
                let safe_name = schema_name.replace('-', "_");
                turn_code.push_str(&format!("struct {} {{\n", safe_name));

                if let Some(props) = schema_def.get("properties").and_then(|p| p.as_object()) {
                    for (prop_name, prop_def) in props {
                        let prop_type = match prop_def.get("type").and_then(|t| t.as_str()) {
                            Some("string") => "Str",
                            Some("integer") | Some("number") => "Num",
                            Some("boolean") => "Bool",
                            Some("array") => "List",
                            Some("object") => "Map",
                            _ => "Any", // fallback
                        };
                        turn_code.push_str(&format!("    {}: {},\n", prop_name, prop_type));
                    }
                }
                turn_code.push_str("};\n\n");
            }
        }
    }

    // Base URL handling
    let mut base_url = url.to_string();
    if let Some(servers) = spec.get("servers").and_then(|s| s.as_array()) {
        if let Some(first_server) = servers.first() {
            if let Some(s_url) = first_server.get("url").and_then(|u| u.as_str()) {
                base_url = s_url.to_string();
            }
        }
    }

    if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
        for (path, methods_val) in paths {
            if let Some(methods) = methods_val.as_object() {
                // For now, we only handle POST as that's typical for actions
                for (method_name, op_def) in methods {
                    if let Some(op_id) = op_def.get("operationId").and_then(|id| id.as_str()) {
                        let safe_id = op_id.replace('-', "_");
                        let full_url = format!("{}{}", base_url, path);

                        // Look for requestBody schema reference to enforce type
                        let mut input_type = "Any".to_string();
                        if let Some(req_body) =
                            op_def.get("requestBody").and_then(|b| b.as_object())
                        {
                            if let Some(content) =
                                req_body.get("content").and_then(|c| c.as_object())
                            {
                                if let Some(json_content) =
                                    content.get("application/json").and_then(|j| j.as_object())
                                {
                                    if let Some(schema) =
                                        json_content.get("schema").and_then(|s| s.as_object())
                                    {
                                        if let Some(ref_path) =
                                            schema.get("$ref").and_then(|r| r.as_str())
                                        {
                                            if let Some(name) =
                                                ref_path.strip_prefix("#/components/schemas/")
                                            {
                                                input_type = name.replace('-', "_");
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Generate a typed native Turn tool function
                        turn_code.push_str(&format!(
                            r#"let {} = turn (payload: {}) {{
    let res = call("http_{}", "{}", payload);
    return res;
}};
"#,
                            safe_id, input_type, method_name, full_url
                        ));
                        exported_methods.push(safe_id);
                    }
                }
            }
        }
    }

    // Parse it into AST
    let lexer = crate::lexer::Lexer::new(&turn_code);
    let tokens = lexer
        .tokenize()
        .map_err(|e| anyhow::anyhow!("Schema macro synteny error: {}", e))?;
    let mut parser = crate::parser::Parser::new(tokens);
    let program = parser
        .parse()
        .map_err(|e| anyhow::anyhow!("Schema macro parse error: {}", e))?;

    // the program is a Vec<Stmt>.
    Ok(program.stmts)
}

pub async fn expand_schema_macro(arg: Value) -> anyhow::Result<Value> {
    let map = match arg {
        Value::Struct(_, map) => map,
        Value::Map(map) => map,
        _ => anyhow::bail!("sys_schema_adapter expected Map argument"),
    };

    let protocol_val = map
        .get("protocol")
        .ok_or_else(|| anyhow::anyhow!("Missing protocol"))?;
    let url_val = map
        .get("url")
        .ok_or_else(|| anyhow::anyhow!("Missing url"))?;

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
        let response = reqwest::get(&url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch schema: {}", e))?;
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
                        turn_code.push_str(&format!(
                            r#"
let {} = turn {{
    // Simulated fetch hook for Turn Agent execution
    return "Fetched data from {}";
}};
"#,
                            safe_id, full_url
                        ));
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
        let tokens = lexer
            .tokenize()
            .map_err(|e| anyhow::anyhow!("Schema macro synteny error: {}", e))?;
        let mut parser = Parser::new(tokens);
        let program = parser
            .parse()
            .map_err(|e| anyhow::anyhow!("Schema macro parse error: {}", e))?;
        let mut compiler = Compiler::new();
        let code = compiler.compile(&program);

        // We evaluate strictly dynamically inside a sandbox VM to produce the closure structure
        let mut vm = Vm::new(&code);

        let result_val = match vm.run() {
            crate::vm::VmResult::Complete(result) => result,
            crate::vm::VmResult::Error(error) => {
                anyhow::bail!("Schema macro evaluation error: {}", error);
            }
            _ => Value::Null,
        };

        return Ok(result_val);
    }

    anyhow::bail!("Unsupported schema protocol: {}", protocol);
}
