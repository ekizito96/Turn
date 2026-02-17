use crate::compiler::Compiler;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::store::Store;
use crate::tools::ToolRegistry;
use crate::value::Value;
use crate::vm::{Vm, VmResult};
use anyhow::Result;

pub struct Runner<S: Store> {
    store: S,
    tools: ToolRegistry,
}

impl<S: Store> Runner<S> {
    pub fn new(store: S, tools: ToolRegistry) -> Self {
        Self { store, tools }
    }

    pub fn run(&mut self, id: &str, source: &str) -> Result<Value> {
        // 1. Load or Init
        let loaded_state = self.store.load(id)?;
        
        // We need 'code' to live for the duration of VM execution
        let code = if let Some(ref state) = loaded_state {
            state.code.clone()
        } else {
            let tokens = Lexer::new(source).tokenize()?;
            let program = Parser::new(tokens).parse()?;
            let mut compiler = Compiler::new();
            compiler.compile(&program)
        };

        let mut vm = if let Some(state) = loaded_state {
            // Resuming from suspension.
            // In a real durable system, we'd check if we have a saved result for the tool call.
            // For v1, we assume restart means "tool failed or we lost result", so we resume with Null.
            // Or we could re-execute the tool? 
            // Let's just resume with Null for now to keep it simple and safe.
            Vm::resume_with_result(state, &code, &self.tools, Value::Null)
        } else {
            Vm::new(&code, &self.tools)
        };

        // 2. Loop
        loop {
            match vm.run() {
                VmResult::Complete(v) => {
                    // Completion
                    return Ok(v);
                }
                VmResult::Suspended { tool_name, arg, continuation } => {
                    // 3. Save state (checkpoint)
                    self.store.save(id, &continuation)?;
                    
                    // 4. Execute tool
                    let result = match self.tools.call(&tool_name, arg) {
                        Some(v) => v,
                        None => Value::Null,
                    };
                    
                    // 5. Resume
                    // We need to construct a new VM with the result
                    vm = Vm::resume_with_result(continuation, &code, &self.tools, result);
                }
            }
        }
    }
}
