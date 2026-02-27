use turn::{FileStore, Runner, ToolRegistry, Value};
use std::path::PathBuf;

#[allow(clippy::approx_constant)]
#[tokio::test]
async fn test_wasm_sandbox_ffi_mounting() -> anyhow::Result<()> {
    // 1. Create a raw internal WebAssembly Text (WAT) module defining basic isolation bounds
    let wat = r#"
    (module
      (func (export "add") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.add)
      (func (export "sub") (param i32 i32) (result i32)
        local.get 0
        local.get 1
        i32.sub)
    )
    "#;
    
    // We write it as `.wasm` extension because Turn will intercept it natively
    // Wasmtime automatically detects WAT strings inside `Module::new/from_file` and parses them
    let test_dir = tempfile::tempdir()?;
    let wasm_path = test_dir.path().join("math.wasm");
    std::fs::write(&wasm_path, wat)?;
    
    let path_str = wasm_path.to_str().unwrap();

    // 2. Write the Turn script to mount the Wasm Component natively
    let local_script = format!(r#"
        let math = use_wasm("{}");
        let result = math.add(15.0, 27.0);
        return result;
    "#, path_str);

    let store = FileStore::new(PathBuf::from(".turn_test_store_wasm"));
    let mut runner = Runner::new(store, ToolRegistry::new());

    // 3. Evaluate the Native code safely into the Sandboxed Wasm Hook
    let result_val = runner.run("wasm_ffi_test", &local_script, None).await?;

    // 4. Assert isolation return values are completely coerced structurally to natively match f64
    println!("Wasm Native Mapping Return: {:?}", result_val);
    
    match result_val {
        Value::Num(n) => {
            assert_eq!(n, 42.0);
        }
        _ => anyhow::bail!("Expected numeric 42.0 from proxy FFI Wasm map, got {:?}", result_val),
    }

    Ok(())
}
