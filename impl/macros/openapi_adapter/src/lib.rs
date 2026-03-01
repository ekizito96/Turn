use std::slice;

/// Ask the Wasm guest to allocate memory for the host to write the HTTP payload into
#[no_mangle]
pub extern "C" fn allocate_memory(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Allows the host to drop the memory inside the guest when it's done reading
#[no_mangle]
pub unsafe extern "C" fn deallocate_memory(ptr: *mut u8, len: usize) {
    let _ = Vec::from_raw_parts(ptr, 0, len);
}

/// Main entrypoint called by the Turn compiler host.
/// `ptr` and `len` refer to the OpenAPI schema payload JSON string stored in Wasm memory.
/// Returns a `u64` where the top 32 bits are the output length, and the bottom 32 bits are the output pointer.
#[no_mangle]
pub extern "C" fn expand_schema(ptr: *const u8, len: usize) -> u64 {
    let slice = unsafe { slice::from_raw_parts(ptr, len) };
    let openapi_schema_str = String::from_utf8_lossy(slice).into_owned();

    // 1. Parse the injected JSON schema payload natively in the Wasm sandbox
    let schema: serde_json::Value = serde_json::from_str(&openapi_schema_str).unwrap_or(serde_json::json!({}));
    
    // Fallback base URL
    let base_url = schema["servers"][0]["url"].as_str().unwrap_or("https://api.github.com").to_string();

    let mut generated_tools = Vec::new();
    
    // 2. Iterate over the OpenAPI paths and dynamically generate Turn AST tools
    if let serde_json::Value::Object(paths) = &schema["paths"] {
        for (path, methods) in paths {
            if let serde_json::Value::Object(methods) = methods {
                for (method, operation) in methods {
                    let op_id = operation["operationId"].as_str().unwrap_or("");
                    if op_id == "" { continue; }

                    let mut params_ast = vec![];
                    
                    // Parse parameters
                    if let serde_json::Value::Array(params) = &operation["parameters"] {
                        for param in params {
                            let name = param["name"].as_str().unwrap_or("");
                            let p_type = param["schema"]["type"].as_str().unwrap_or("string");
                            
                            let turn_type = match p_type {
                                "integer" | "number" => "Num",
                                "boolean" => "Bool",
                                _ => "Str",
                            };

                            params_ast.push(serde_json::json!([
                                name,
                                {"start": 0, "end": 0},
                                turn_type,
                                false
                            ]));
                        }
                    }

                    // Build URL interpolation AST
                    let mut url_chunks = Vec::new();
                    let base_url = schema["servers"][0]["url"].as_str().unwrap_or("https://api.github.com");
                    
                    // Parse path like "/repos/{owner}/{repo}"
                    let mut current_literal = String::from(base_url);
                    let mut in_var = false;
                    let mut current_var = String::new();

                    for c in path.chars() {
                        if c == '{' {
                            if !current_literal.is_empty() {
                                url_chunks.push(serde_json::json!({
                                    "Literal": {
                                        "value": {"Str": current_literal},
                                        "span": {"start": 0, "end": 0}
                                    }
                                }));
                                current_literal.clear();
                            }
                            in_var = true;
                        } else if c == '}' {
                            url_chunks.push(serde_json::json!({
                                "Id": {
                                    "name": current_var,
                                    "span": {"start": 0, "end": 0}
                                }
                            }));
                            current_var.clear();
                            in_var = false;
                        } else {
                            if in_var {
                                current_var.push(c);
                            } else {
                                current_literal.push(c);
                            }
                        }
                    }
                    
                    if !current_literal.is_empty() {
                        url_chunks.push(serde_json::json!({
                            "Literal": {
                                "value": {"Str": current_literal},
                                "span": {"start": 0, "end": 0}
                            }
                        }));
                    }

                    // Fold chunks into nested Binary Add AST
                    let mut url_ast = url_chunks[0].clone();
                    for i in 1..url_chunks.len() {
                        url_ast = serde_json::json!({
                            "Binary": {
                                "op": "Add",
                                "left": url_ast,
                                "right": url_chunks[i],
                                "span": {"start": 0, "end": 0}
                            }
                        });
                    }

                    // Determine the underlying HTTP tool to call based on the method
                    let http_tool = if method.eq_ignore_ascii_case("post") {
                        "http_post"
                    } else {
                        "http_get"
                    };

                    let tool_ast = serde_json::json!([
                        {
                            "Literal": {
                                "value": {"Str": op_id},
                                "span": {"start": 0, "end": 0}
                            }
                        },
                        {
                            "Turn": {
                                "is_tool": false,
                                "params": params_ast,
                                "ret_ty": "Any",
                                "body": {
                                    "stmts": [{
                                        "Return": {
                                            "expr": {
                                                "Call": {
                                                    "name": {"Id": {"name": http_tool, "span": {"start": 0, "end": 0}}},
                                                    "arg": url_ast,
                                                    "span": {"start": 0, "end": 0}
                                                }
                                            },
                                            "span": {"start": 0, "end": 0}
                                        }
                                    }],
                                    "span": {"start": 0, "end": 0}
                                },
                                "span": {"start": 0, "end": 0}
                            }
                        }
                    ]);

                    if !tool_ast.is_null() {
                        generated_tools.push(tool_ast.to_string());
                    }
                }
            }
        }
    }

    // Wrap the injected tools into a final Turn Map structure
    let out_json = format!(r#"[{{"Return":{{"expr":{{"Map":{{"entries":[{}],"span":{{"start":0,"end":0}}}}}},"span":{{"start":0,"end":0}}}}}}]"#, generated_tools.join(","));

    let mut boxed_bytes = out_json.into_bytes().into_boxed_slice();
    let out_len = boxed_bytes.len();
    let out_ptr = boxed_bytes.as_mut_ptr();
    std::mem::forget(boxed_bytes);

    ((out_len as u64) << 32) | (out_ptr as u64)
}
