use anyhow::Result;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use turn::{Runner, Store, ToolRegistry, Value, VmState};

// In-memory store
#[derive(Clone)]
struct MemoryStore {
    data: Rc<RefCell<HashMap<String, VmState>>>,
}

impl MemoryStore {
    fn new() -> Self {
        Self {
            data: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}

impl Store for MemoryStore {
    fn save(&mut self, id: &str, state: &VmState) -> Result<()> {
        self.data.borrow_mut().insert(id.to_string(), state.clone());
        Ok(())
    }

    fn load(&self, id: &str) -> Result<Option<VmState>> {
        Ok(self.data.borrow().get(id).cloned())
    }
}

#[test]
fn test_try_catch_basic() {
    let source = r#"
    turn {
        try {
            throw "oops";
        } catch (e) {
            return "caught: " + e;
        }
        return "uncaught";
    }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner.run("test_error", source, None).unwrap();
    assert_eq!(result, Value::Str("caught: oops".to_string()));
}

#[test]
fn test_try_catch_no_error() {
    let source = r#"
    turn {
        try {
            let x = 1;
        } catch (e) {
            return "caught: " + e;
        }
        return "success";
    }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner.run("test_error", source, None).unwrap();
    assert_eq!(result, Value::Str("success".to_string()));
}

#[test]
fn test_nested_try_catch() {
    let source = r#"
    turn {
        try {
            try {
                throw "inner";
            } catch (e) {
                throw "outer: " + e;
            }
        } catch (e) {
            return "caught: " + e;
        }
    }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner.run("test_error", source, None).unwrap();
    assert_eq!(result, Value::Str("caught: outer: inner".to_string()));
}

#[test]
fn test_cross_function_throw() {
    let source = r#"
    turn {
        let fail = turn {
            throw "fail";
        };
        
        try {
            call(fail, {});
        } catch (e) {
            return "caught: " + e;
        }
    }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    let result = runner.run("test_error", source, None).unwrap();
    assert_eq!(result, Value::Str("caught: fail".to_string()));
}
