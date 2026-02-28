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
    let _openapi_schema = String::from_utf8_lossy(slice).into_owned();

    // In Phase 6b, this is where we map `openapi_schema` to Turn Structs and Tools.
    // To prove the memory architecture, we will return a hardcoded JSON string
    // representing a Turn AST List of Statements.
    
    // An empty valid JSON array of Stmt
    let out_json = r#"[{"Return":{"expr":{"Map":{"entries":[["get_secret",{"Turn":{"is_tool":false,"params":[],"ret_ty":null,"body":{"stmts":[{"Return":{"expr":{"Literal":{"value":{"Str":"Fetched data from http://127.0.0.1:0/v1/secret"},"span":{"start":0,"end":0}}},"span":{"start":0,"end":0}}}],"span":{"start":0,"end":0}},"span":{"start":0,"end":0}}}]],"span":{"start":0,"end":0}}},"span":{"start":0,"end":0}}}]"#.to_string();

    let bytes = out_json.into_bytes();
    let out_len = bytes.len();
    let out_ptr = bytes.as_ptr();
    std::mem::forget(bytes); // Prevent Wasm from freeing this buffer before the Host reads it

    ((out_len as u64) << 32) | (out_ptr as u64)
}
