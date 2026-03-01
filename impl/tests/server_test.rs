use turn::server;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn test_server_run() {
    let store_path = PathBuf::from(".turn_store_test");
    if store_path.exists() {
        std::fs::remove_dir_all(&store_path).unwrap();
    }

    // Spawn server task
    let server_handle = tokio::spawn(async move {
        // Use port 3333 for testing
        if let Err(e) = server::serve(3333, store_path).await {
            eprintln!("Server error: {}", e);
        }
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 2. Make request
    let client = reqwest::Client::new();
    let resp = client.post("http://127.0.0.1:3333/run")
        .json(&serde_json::json!({
            "id": "test-agent",
            "source": "turn { return \"Hello from Test!\"; }"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"], "Hello from Test!");

    // Cleanup
    server_handle.abort();
    if std::path::Path::new(".turn_store_test").exists() {
        std::fs::remove_dir_all(".turn_store_test").unwrap();
    }
}
