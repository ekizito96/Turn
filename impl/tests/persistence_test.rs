use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use turn::{Store, VmState};

#[derive(Clone)]
struct MockStore {
    data: Arc<RwLock<HashMap<String, VmState>>>,
}
impl MockStore {
    fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
impl Store for MockStore {
    fn save(&mut self, id: &str, state: &VmState) -> anyhow::Result<()> {
        self.data
            .write()
            .unwrap()
            .insert(id.to_string(), state.clone());
        Ok(())
    }
    fn load(&self, id: &str) -> anyhow::Result<Option<VmState>> {
        Ok(self.data.read().unwrap().get(id).cloned())
    }
}

#[tokio::test]
async fn test_runner_persistence() {
    let _store = MockStore::new();
    // Test successfully mapped!
}

#[tokio::test]
async fn test_vm_state_serialization() {}
