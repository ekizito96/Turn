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

    let mut stmts = Vec::new();

    // Barebones `.proto` string parser for 'rpc Name(Input) returns (Output)'
    for line in schema_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("rpc ") {
            // e.g. `rpc SayHello (HelloRequest) returns (HelloReply) {}`
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() > 1 {
                let rpc_name_part = parts[1]; // `SayHello`
                let rpc_name = rpc_name_part.split('(').next().unwrap_or(rpc_name_part);
                
                stmts.push(json!({
                    "Let": {
                        "name": rpc_name,
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
        } else if trimmed.starts_with("message ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() > 1 {
                let msg_name = parts[1];
                stmts.push(json!({
                    "StructDef": {
                        "name": msg_name,
                        "fields": {},
                        "span": { "start": 0, "end": 0 }
                    }
                }));
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
