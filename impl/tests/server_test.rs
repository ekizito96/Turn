use std::path::PathBuf;
use turn::runner::Runner;
use turn::store::FileStore;
use turn::tools::ToolRegistry;

#[tokio::test]
async fn test_server_run() {
    let store_path = PathBuf::from(".turn_store_server_test");
    if store_path.exists() {
        std::fs::remove_dir_all(&store_path).unwrap();
    }

    let store = FileStore::new(store_path.clone());
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner
        .run("test-agent", "turn { return \"Hello from Test!\"; }", None)
        .await;

    assert!(result.is_ok(), "Runner should succeed: {:?}", result.err());
    // Convert result value to string for assertion
    let val = result.unwrap();
    let val_str = format!("{:?}", val);
    assert!(
        val_str.contains("Hello from Test!"),
        "Unexpected output: {}",
        val_str
    );

    // Cleanup
    if std::path::Path::new(".turn_store_server_test").exists() {
        std::fs::remove_dir_all(".turn_store_server_test").unwrap();
    }
}
