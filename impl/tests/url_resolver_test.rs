use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use axum::{routing::get, Router};
use turn::{compiler::Compiler, lexer::Lexer, parser::Parser, vm::Vm, FileStore, Runner, ToolRegistry};

#[tokio::test]
async fn test_url_native_ast_caching() -> anyhow::Result<()> {
    // 1. Start a local Axum HTTP server to serve the mock remote .tn file
    let remote_tn_code = r#"
        let get_secret = turn {
            return 42;
        };
        return {
            "get_secret": get_secret
        };
    "#;

    let app = Router::new().route("/remote.tn", get(move || async move { remote_tn_code }));

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // 2. Clear out the cache directory if it exists to ensure purely fresh download
    let cache_dir = std::env::current_dir()?.join(".turn_cache").join("ast");
    if cache_dir.exists() {
        std::fs::remove_dir_all(&cache_dir)?;
    }

    let local_script = format!(r#"
        let remote = use "http://127.0.0.1:{}/remote.tn";
        return remote.get_secret();
    "#, port);

    let store = FileStore::new(PathBuf::from(".turn_test_store_url"));
    let mut runner = Runner::new(store, ToolRegistry::new());

    // Evaluate the code using the standard Turn runner
    let result_val = runner.run("url_test", &local_script, None).await?;

    assert_eq!(result_val, turn::value::Value::Num(42.0));

    // Verify cache was created
    assert!(cache_dir.exists(), "Cache directory was not created");
    
    let mut stored_files = 0;
    for entry in std::fs::read_dir(&cache_dir)? {
        let entry = entry?;
        if entry.path().extension().and_then(|e| e.to_str()) == Some("tn") {
            stored_files += 1;
        }
    }
    assert_eq!(stored_files, 1, "Expected exactly 1 cached .tn file");

    Ok(())
}
