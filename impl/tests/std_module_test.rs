use turn::runner::Runner;
use turn::store::FileStore;
use turn::tools::ToolRegistry;
use turn::value::Value;

#[tokio::test]
async fn test_std_math_module() {
    let store_dir = std::env::temp_dir().join("turn_test_std_math");
    let _ = std::fs::remove_dir_all(&store_dir);
    let store = FileStore::new(store_dir);
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let source = r#"
    let math = use "std/math";
    let a = math.max(10, 20);
    let b = math.min(10, 20);
    let c = math.abs(-5);
    return [a, b, c];
    "#;

    let result = runner.run("test_agent", source, None).await.expect("Run failed");

    if let Value::List(items) = result {
        assert_eq!(items[0], Value::Num(20.0));
        assert_eq!(items[1], Value::Num(10.0));
        assert_eq!(items[2], Value::Num(5.0));
    } else {
        panic!("Expected list, got {:?}", result);
    }
}

#[tokio::test]
async fn test_std_fs_module() {
    // Note: We can't easily test actual FS write/read without mocking fs_read/write tools.
    // But we can check if the module loads and functions exist.
    let store_dir = std::env::temp_dir().join("turn_test_std_fs");
    let _ = std::fs::remove_dir_all(&store_dir);
    let store = FileStore::new(store_dir);
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let source = r#"
    let fs = use "std/fs";
    // Just verify we got a map with "read" and "write"
    return fs;
    "#;

    let result = runner.run("test_agent", source, None).await.expect("Run failed");

    if let Value::Map(m) = result {
        assert!(m.contains_key("read"));
        assert!(m.contains_key("write"));
    } else {
        panic!("Expected map, got {:?}", result);
    }
}
