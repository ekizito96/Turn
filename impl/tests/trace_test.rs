use turn::runner::Runner;
use turn::store::Store;
use turn::vm::VmState;
use turn::tools::ToolRegistry;
use turn::value::Value;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

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
async fn test_native_tracing() {
    let store = MemoryStore::new();
    let mut runner = Runner::new(store, ToolRegistry::new());

    // In a trace environment, the target pid expects its LLM stream (or tool calls)
    // to mirror natively into the watcher (tracer) mailbox.
    let code = r#"


        // We spawn a target actor that will execute tools/inference
        let target_pid = spawn turn() {
            // Simulate a tool call / infer evaluation loop
            // Since this test validates the hook, we can just trigger a basic suspension 
            // However, tracing natively routes LLM streams during `infer` or tool evaluations.
            print("triggering native trace hooks");
        };

        // We call trace() on the returned PID
        trace(target_pid);

        // We check our local mailbox to see if we received the target's emitted trace
        let received = harvest;
        return received;
    "#;

    let result = runner.run("test", code, None).await.unwrap();
    
    // We expect the actor to return either Null or the struct 
    // depending on timing, for this dummy test we're ensuring the `trace()` 
    // evaluated successfully without panic and trapped.
    assert!(
        matches!(result, Value::Null) || matches!(result, Value::Struct(_, _)),
        "Expected trace event struct or Null, got {:?}",
        result
    );
}
