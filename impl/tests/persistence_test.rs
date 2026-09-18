use anyhow::{anyhow, Result};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use turn::{EffectRecord, Runner, Store, ToolRegistry, Value, VmState};

// In-memory store for testing
#[derive(Clone)]
struct MemoryStore {
    data: Rc<RefCell<HashMap<String, VmState>>>,
    save_count: Rc<RefCell<usize>>,
    fail_on_save: usize, // 0 means never fail
    effects: Rc<RefCell<HashMap<String, EffectRecord>>>,
    fail_after_effect_save: Rc<RefCell<bool>>,
    fail_on_load: Rc<RefCell<bool>>,
}

impl MemoryStore {
    fn new() -> Self {
        Self {
            data: Rc::new(RefCell::new(HashMap::new())),
            save_count: Rc::new(RefCell::new(0)),
            fail_on_save: 0,
            effects: Rc::new(RefCell::new(HashMap::new())),
            fail_after_effect_save: Rc::new(RefCell::new(false)),
            fail_on_load: Rc::new(RefCell::new(false)),
        }
    }
}

impl Store for MemoryStore {
    fn save(&mut self, id: &str, state: &VmState) -> Result<()> {
        let mut count = self.save_count.borrow_mut();
        *count += 1;
        if self.fail_on_save > 0 && *count == self.fail_on_save {
            return Err(anyhow!("Simulated crash on save {}", count));
        }
        self.data.borrow_mut().insert(id.to_string(), state.clone());
        Ok(())
    }

    fn load(&self, id: &str) -> Result<Option<VmState>> {
        if *self.fail_on_load.borrow() {
            return Err(anyhow!("Simulated checkpoint read failure"));
        }
        Ok(self.data.borrow().get(id).cloned())
    }

    fn delete(&mut self, id: &str) -> Result<()> {
        self.data.borrow_mut().remove(id);
        Ok(())
    }

    fn load_effect(&self, _id: &str, effect_id: &str) -> Result<Option<EffectRecord>> {
        Ok(self.effects.borrow().get(effect_id).cloned())
    }

    fn save_effect(&mut self, _id: &str, record: &EffectRecord) -> Result<()> {
        self.effects
            .borrow_mut()
            .insert(record.effect_id.clone(), record.clone());
        if *self.fail_after_effect_save.borrow() {
            *self.fail_after_effect_save.borrow_mut() = false;
            return Err(anyhow!("Simulated crash after effect journal save"));
        }
        Ok(())
    }

    fn list_effects(&self, _id: &str) -> Result<Vec<EffectRecord>> {
        Ok(self.effects.borrow().values().cloned().collect())
    }
}

#[test]
fn test_journaled_effect_is_not_executed_twice_after_crash() {
    let source = r#"
    let result = call("charge", { "order_id": "ORD-123", "amount": 99 });
    return result;
    "#;
    let store = MemoryStore::new();
    *store.fail_after_effect_save.borrow_mut() = true;
    let calls = Arc::new(AtomicUsize::new(0));

    let mut tools = ToolRegistry::new();
    let call_counter = calls.clone();
    tools.register(
        "charge",
        Box::new(move |_| {
            call_counter.fetch_add(1, Ordering::SeqCst);
            Ok((Value::Str("charged".to_string()), 0))
        }),
    );
    let mut runner = Runner::new(store.clone(), tools);
    let first = runner.run("order", source, None);
    assert_eq!(
        first.unwrap_err().to_string(),
        "Simulated crash after effect journal save"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(store.list_effects("order").unwrap().len(), 1);

    let mut tools = ToolRegistry::new();
    let call_counter = calls.clone();
    tools.register(
        "charge",
        Box::new(move |_| {
            call_counter.fetch_add(1, Ordering::SeqCst);
            Ok((Value::Str("charged-again".to_string()), 0))
        }),
    );
    let mut runner = Runner::new(store.clone(), tools);
    assert_eq!(
        runner.run("order", source, None).unwrap(),
        Value::Str("charged".to_string())
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn test_completed_rerun_does_not_reuse_prior_execution_journal() {
    let source = r#"return call("effect", null);"#;
    let store = MemoryStore::new();
    let calls = Arc::new(AtomicUsize::new(0));

    for expected in 1..=2 {
        let mut tools = ToolRegistry::new();
        let call_counter = calls.clone();
        tools.register(
            "effect",
            Box::new(move |_| {
                let value = call_counter.fetch_add(1, Ordering::SeqCst) + 1;
                Ok((Value::Num(value as f64), 0))
            }),
        );
        let mut runner = Runner::new(store.clone(), tools);
        assert_eq!(
            runner.run("same-agent", source, None).unwrap(),
            Value::Num(expected as f64)
        );
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(store.list_effects("same-agent").unwrap().len(), 2);
}

#[test]
fn test_checkpoint_read_failure_does_not_start_fresh_execution() {
    let source = r#"return call("effect", null);"#;
    let store = MemoryStore::new();
    *store.fail_on_load.borrow_mut() = true;
    let calls = Arc::new(AtomicUsize::new(0));
    let mut tools = ToolRegistry::new();
    let call_counter = calls.clone();
    tools.register(
        "effect",
        Box::new(move |_| {
            call_counter.fetch_add(1, Ordering::SeqCst);
            Ok((Value::Null, 0))
        }),
    );

    let mut runner = Runner::new(store, tools);
    let error = runner.run("agent", source, None).unwrap_err();
    assert_eq!(error.to_string(), "Simulated checkpoint read failure");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn test_persistence_crash_recovery() {
    let source = r#"
    turn {
        let a = call("echo", "step1");
        let b = call("echo", "step2");
        return a + b;
    }
    "#;

    // 1. Setup Store that fails on 2nd save (Tool 2 call)
    // Save 1: Tool 1 call (step1)
    // Save 2: Tool 2 call (step2) -> Crash
    let mut store = MemoryStore::new();
    store.fail_on_save = 2;

    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store.clone(), tools);

    // 2. Run -> Should fail
    let result = runner.run("agent1", source, None);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Simulated crash on save 2");

    // 3. Verify state was saved once (at step1)
    // The store should have the state from the first save.
    assert!(store.data.borrow().contains_key("agent1"));

    // 4. Resume with fresh runner (and non-failing store)
    let mut new_store = store.clone();
    new_store.fail_on_save = 0; // Don't fail anymore
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(new_store, tools);

    // 5. Run again -> Should replay the pending effect and complete
    let result = runner.run("agent1", source, None).unwrap();

    // 6. Verify result
    match result {
        Value::Str(s) => assert_eq!(s, "step1step2"),
        _ => panic!("Expected string, got {:?}", result),
    }
    assert!(!store.data.borrow().contains_key("agent1"));
}
