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
        // FHIR CapabilityStatement parsing: /rest/0/resource
        if let Some(rests) = root.get("rest").and_then(|r| r.as_array()) {
            if let Some(rest_node) = rests.get(0) {
                if let Some(resources) = rest_node.get("resource").and_then(|res| res.as_array()) {
                    for res in resources {
                        if let Some(res_type) = res.get("type").and_then(|t| t.as_str()) {
                            // Emit Struct for FHIR Resource type
                            stmts.push(json!({
                                "StructDef": {
                                    "name": res_type,
                                    "fields": {},
                                    "span": { "start": 0, "end": 0 }
                                }
                            }));

                            // Emit Tool for standard FHIR operations e.g. read
                            let tool_name = format!("read_{}", res_type);
                            stmts.push(json!({
                                "Let": {
                                    "name": tool_name,
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

    let out_json = serde_json::to_string(&stmts).unwrap_or_else(|_| "[]".to_string());
    let bytes = out_json.into_bytes();
    let out_len = bytes.len();
    let out_ptr = bytes.as_ptr();
    std::mem::forget(bytes); 
    ((out_len as u64) << 32) | (out_ptr as u64)
}
