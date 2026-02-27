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

    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&schema_text);
    let mut stmts = Vec::new();

    if let Ok(root) = parsed {
        if let Some(paths) = root.get("paths").and_then(|p| p.as_object()) {
            for (_path, methods) in paths {
                if let Some(methods_obj) = methods.as_object() {
                    for (_method, op) in methods_obj {
                        if let Some(op_id) = op.get("operationId").and_then(|id| id.as_str()) {
                            // Map each Swagger Operation into a native Turn Tool
                            stmts.push(json!({
                                "Let": {
                                    "name": op_id,
                                    "ty": null,
                                    "init": {
                                        "Turn": {
                                            "is_tool": true,
                                            "params": [], // Ignoring params for MVP
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

    let out_json = serde_json::to_string(&stmts).unwrap_or_else(|_| "[]".to_string());
    let bytes = out_json.into_bytes();
    let out_len = bytes.len();
    let out_ptr = bytes.as_ptr();
    std::mem::forget(bytes); 
    ((out_len as u64) << 32) | (out_ptr as u64)
}
