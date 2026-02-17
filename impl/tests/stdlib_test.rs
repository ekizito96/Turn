use turn::{Runner, Store, ToolRegistry, Value, VmState};
use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;

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
fn test_fs_read_write() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    let path_str = file_path.to_string_lossy().to_string();

    // On Windows, paths have backslashes which need escaping in Turn string literals.
    let escaped_path = path_str.replace("\\", "\\\\");
    let source = format!(r#"
    turn {{
        let content = "Hello, Turn FS!";
        call("fs_write", {{ "path": "{}", "content": content }});
        
        let read_back = call("fs_read", "{}");
        return read_back;
    }}
    "#, escaped_path, escaped_path);

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);
    
    let result = runner.run("test_fs", &source, None).unwrap();
    assert_eq!(result, Value::Str("Hello, Turn FS!".to_string()));
}

#[test]
fn test_fs_error_handling() {
    let source = r#"
    turn {
        try {
            call("fs_read", "non_existent_file.txt");
        } catch (e) {
            return "caught: " + e; // e should be the error message
        }
        return "failed to catch";
    }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);
    
    let result = runner.run("test_fs_err", source, None).unwrap();
    
    if let Value::Str(s) = result {
        assert!(s.starts_with("caught: Failed to read file"));
    } else {
        panic!("Expected string result, got {:?}", result);
    }
}

#[test]
fn test_env_vars() {
    let source = r#"
    turn {
        call("env_set", { "key": "TURN_TEST_VAR", "value": "turn_is_cool" });
        let v = call("env_get", "TURN_TEST_VAR");
        return v;
    }
    "#;

    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);
    
    let result = runner.run("test_env", source, None).unwrap();
    assert_eq!(result, Value::Str("turn_is_cool".to_string()));
}
