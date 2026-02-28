use anyhow::Result;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use turn::{Runner, Store, ToolRegistry, Value, VmState};

// In-memory store
#[derive(Clone)]
struct MemoryStore {
    data: Arc<RwLock<HashMap<String, VmState>>>,
}

impl MemoryStore {
    fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Store for MemoryStore {
    fn save(&mut self, id: &str, state: &VmState) -> Result<()> {
        self.data
            .write()
            .unwrap()
            .insert(id.to_string(), state.clone());
        Ok(())
    }

    fn load(&self, id: &str) -> Result<Option<VmState>> {
        Ok(self.data.read().unwrap().get(id).cloned())
    }
}

#[tokio::test]
async fn test_try_catch_basic() {
    let source = r#"
        let result = err("oops");
        match result {
            ok(v) -> { return "uncaught"; }
            err(e) -> { return "caught: " + e; }
        }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner.run("test_error", source, None).await.unwrap();
    assert_eq!(
        result,
        Value::Str(std::sync::Arc::new("caught: oops".to_string()))
    );
}

#[tokio::test]
async fn test_try_catch_no_error() {
    let source = r#"
        let result = ok(1);
        match result {
            ok(v) -> { return "success"; }
            err(e) -> { return "caught: " + e; }
        }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner.run("test_error", source, None).await.unwrap();
    assert_eq!(
        result,
        Value::Str(std::sync::Arc::new("success".to_string()))
    );
}

#[tokio::test]
async fn test_nested_try_catch() {
    let source = r#"
        let res1 = err("inner");
        
        match res1 {
            ok(v) -> { return "unexpected"; }
            err(e) -> { 
                let res2 = err("outer: " + e);
                match res2 {
                    ok(v2) -> { return "success"; }
                    err(e2) -> { return "caught: " + e2; }
                }
            }
        }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner.run("test_error", source, None).await.unwrap();
    assert_eq!(
        result,
        Value::Str(std::sync::Arc::new("caught: outer: inner".to_string()))
    );
}

#[tokio::test]
async fn test_cross_function_throw() {
    let source = r#"
        let fail = turn() {
            return err("fail");
        };
        
        let result = call(fail, {});
        match result {
            ok(v) -> { return "success"; }
            err(e) -> { return "caught: " + e; }
        }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner.run("test_error", source, None).await.unwrap();
    assert_eq!(
        result,
        Value::Str(std::sync::Arc::new("caught: fail".to_string()))
    );
}
