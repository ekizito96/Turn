use turn::{Runner, Store, ToolRegistry, Value, VmState};
use anyhow::{Result, anyhow};
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;

// In-memory store for testing
#[derive(Clone)]
struct MemoryStore {
    data: Rc<RefCell<HashMap<String, VmState>>>,
    save_count: Rc<RefCell<usize>>,
    fail_on_save: usize, // 0 means never fail
}

impl MemoryStore {
    fn new() -> Self {
        Self {
            data: Rc::new(RefCell::new(HashMap::new())),
            save_count: Rc::new(RefCell::new(0)),
            fail_on_save: 0,
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
        Ok(self.data.borrow().get(id).cloned())
    }
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

    // 5. Run again -> Should resume and complete
    let result = runner.run("agent1", source, None).unwrap();
    
    // 6. Verify result
    // "step1" + "step2" = "step1step2"
    // BUT: Since we crashed at Save 2 (before persisting "step1"), we revert to Save 1 state.
    // Save 1 state is "suspended at Tool 1".
    // When we resume, we inject Null (simulating tool failure/loss).
    // So 'a' becomes Null.
    // Then we proceed to Tool 2. 'b' becomes "step2".
    // Result is Null + "step2" = "nullstep2".
    match result {
        Value::Str(s) => assert_eq!(s.as_str(), "nullstep2"),
        _ => panic!("Expected string, got {:?}", result),
    }
}

#[test]
fn test_jvm_snapshot_recovery() {
    use turn::compiler::Compiler;
    use turn::lexer::Lexer;
    use turn::parser::Parser;
    use turn::vm::{Vm, VmResult};
    use std::fs;

    let source = r#"
    let count = 10;
    suspend;
    let next = count + 5;
    return next * 2;
    "#;
    
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let program = parser.parse().unwrap();
    let mut compiler = Compiler::new();
    let code = compiler.compile(&program);
    let mut vm = Vm::new(&code);
    
    let heap_path = ".turn_heap.json";
    let _ = fs::remove_file(heap_path);

    // Initial run up to suspend
    let result = vm.run();
    
    // Expect suspension
    let state = match result {
        VmResult::Suspended { continuation, .. } => continuation,
        _ => panic!("Expected VM to suspend, got {:?}", result),
    };
    
    // Verify file was written by Orthogonal Persistence framework
    assert!(fs::metadata(heap_path).is_ok(), "Heap file should exist after suspend");
    
    // Simulate server crash! Drop the VM
    drop(vm);
    
    // Boot a new VM directly from the orthogonal persistence file
    let mut recovered_vm = Vm::resume_from_disk(heap_path).expect("Failed to resume from disk");
    
    // Force a mock stack push that the suspend syscall would usually return
    if let Some(process) = recovered_vm.scheduler.front_mut() {
        process.stack.push(Value::Null); // suspend returns null natively
    }
    
    // Continue execution from exact Instruction Pointer where it died
    let final_result = recovered_vm.run();
    
    match final_result {
        VmResult::Complete(Value::Num(n)) => assert_eq!(n, 30.0),
        _ => panic!("Expected execution to resume and return Complete(30.0), but got {:?}", final_result),
    }
    
    // Cleanup
    let _ = fs::remove_file(heap_path);
}
