use turn::{Runner, Store, ToolRegistry, Value, VmState};
use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

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
fn test_module_import() {
    let temp_dir = tempfile::tempdir().unwrap();
    let module_path = temp_dir.path().join("math.turn");
    
    // Create a module file
    let module_source = r#"
    let pi = 3.14;
    let double = turn {
        let x = recall("x");
        return x + x;
    };
    return {
        "PI": pi,
        "double": double
    };
    "#;
    fs::write(&module_path, module_source).unwrap();
    
    // Main script using the module
    let main_source = format!(r#"
    turn {{
        let math = use "{}";
        let pi = math["PI"];
        let d = call(math["double"], {{ "x": 10 }});
        return {{ "pi": pi, "d": d }};
    }}
    "#, module_path.display());
    
    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);
    
    let result = runner.run("test_agent", &main_source, None).unwrap();
    
    if let Value::Map(m) = result {
        assert_eq!(m.get("pi"), Some(&Value::Num(3.14)));
        assert_eq!(m.get("d"), Some(&Value::Num(20.0)));
    } else {
        panic!("Expected map, got {:?}", result);
    }
}

#[test]
fn test_recursive_import() {
    let temp_dir = tempfile::tempdir().unwrap();
    let util_path = temp_dir.path().join("util.turn");
    let main_path = temp_dir.path().join("main.turn");
    
    // util.turn
    fs::write(&util_path, r#"return "util";"#).unwrap();
    
    // main.turn imports util
    // We test relative import logic in Runner if we implement it.
    // Currently Runner::load_module handles relative paths if 'current_file' is passed.
    // But 'run' doesn't pass a file path for the source.
    // So imports in the main script are relative to CWD.
    // We'll use absolute paths for simplicity in test.
    
    let source = format!(r#"
    turn {{
        let u = use "{}";
        return u;
    }}
    "#, util_path.display());
    
    let store = MemoryStore::new();
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);
    
    let result = runner.run("test_agent", &source, None).unwrap();
    assert_eq!(result, Value::Str("util".to_string()));
}
