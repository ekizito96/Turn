use serde_json::json;
use std::slice;

#[no_mangle]
pub extern "C" fn allocate_memory(len: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn deallocate_memory(ptr: *mut u8, len: usize) {
    let _ = Vec::from_raw_parts(ptr, 0, len);
}

#[no_mangle]
pub extern "C" fn expand_schema(ptr: *const u8, len: usize) -> u64 {
    let slice = unsafe { slice::from_raw_parts(ptr, len) };
    let schema_text = String::from_utf8_lossy(slice).into_owned();

    // 1. Parse GraphQL Introspection JSON
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&schema_text);
    
    let mut stmts = Vec::new();

    if let Ok(root) = parsed {
        // Highly simplified extraction of types
        if let Some(types) = root.pointer("/data/__schema/types").and_then(|t| t.as_array()) {
            for ty_obj in types {
                if let (Some(kind), Some(name)) = (
                    ty_obj.get("kind").and_then(|k| k.as_str()),
                    ty_obj.get("name").and_then(|n| n.as_str()),
                ) {
                    if kind == "OBJECT" && !name.starts_with("__") {
                        let mut fields_map = serde_json::Map::new();
                        
                        if let Some(fields) = ty_obj.get("fields").and_then(|f| f.as_array()) {
                            for f in fields {
                                if let Some(f_name) = f.get("name").and_then(|n| n.as_str()) {
                                    // Map everything to Any for simplicity in MVP
                                    fields_map.insert(f_name.to_string(), json!("Any"));
                                }
                            }
                        }

                        // Emit StructDef AST Node
                        stmts.push(json!({
                            "StructDef": {
                                "name": name,
                                "fields": fields_map,
                                "span": { "start": 0, "end": 0 }
                            }
                        }));
                        
                        // If it's the Root Query or Mutation type, emit Tools for each field
                        if name == "Query" || name == "Mutation" {
                            if let Some(fields) = ty_obj.get("fields").and_then(|f| f.as_array()) {
                                for f in fields {
                                    if let Some(f_name) = f.get("name").and_then(|n| n.as_str()) {
                                        
                                        // Emit Let Tool AST Node
                                        stmts.push(json!({
                                            "Let": {
                                                "name": f_name,
                                                "ty": null,
                                                "init": {
                                                    "Turn": {
                                                        "is_tool": true,
                                                        "params": [], // Ignoring args for MVP
                                                        "ret_ty": "Any",
                                                        "body": {
                                                            "stmts": [],
                                                            "span": { "start": 0, "end": 0 }
                                                        },
                                                        "span": { "start": 0, "end": 0 }
                                                    }
                                                },
                                                "is_persistent": false,
                                                "span": { "start": 0, "end": 0 }
                                            }
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let out_json = serde_json::to_string(&stmts).unwrap_or_else(|_| "[]".to_string());

    let bytes = out_json.into_bytes();
    let out_len = bytes.len();
    let out_ptr = bytes.as_ptr();
    std::mem::forget(bytes); 

    ((out_len as u64) << 32) | (out_ptr as u64)
}
