use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[no_mangle]
pub extern "C" fn alloc(len: u32) -> u32 {
    let mut buf: Vec<u8> = Vec::with_capacity(len as usize);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr as usize as u32
}

unsafe fn read_string(ptr: u32, len: u32) -> String {
    let buf = Vec::from_raw_parts(ptr as *mut u8, len as usize, len as usize);
    String::from_utf8_lossy(&buf).into_owned()
}

fn pack_string(s: String) -> u64 {
    let len = s.len() as u64;
    let mut buf = s.into_bytes();
    let ptr = buf.as_mut_ptr() as u64;
    std::mem::forget(buf);
    (ptr << 32) | len
}

#[no_mangle]
pub unsafe extern "C" fn transform_request(_ptr: u32, _len: u32) -> u64 {
    pack_string(json!({ 
        "error": "AWS Bedrock SigV4 signing is currently unsupported inside the strict Wasm runtime sandbox (cannot access real-time clock for x-amz-date). TBD!" 
    }).to_string())
}

#[no_mangle]
pub unsafe extern "C" fn transform_response(_ptr: u32, _len: u32) -> u64 {
    pack_string(json!({"jsonrpc": "2.0", "id": 1, "error": "Unimplemented AWS Response Parser"}).to_string())
}
