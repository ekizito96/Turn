use crate::compiler::Compiler;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::store::Store;
use crate::tools::ToolRegistry;
use crate::value::Value;
use crate::vm::{Vm, VmResult};
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Runner<S: Store> {
    store: S,
    tools: ToolRegistry,
    module_cache: HashMap<String, Value>,
}

impl<S: Store> Runner<S> {
    pub fn new(store: S, tools: ToolRegistry) -> Self {
        Self { 
            store, 
            tools,
            module_cache: HashMap::new(),
        }
    }

    fn load_module(&mut self, path: &str, current_file: Option<&PathBuf>) -> Result<Value> {
        // Resolve path relative to current file if provided
        let resolved_path = if let Some(base) = current_file {
            let parent = base.parent().unwrap_or(std::path::Path::new("."));
            parent.join(path).canonicalize().unwrap_or_else(|_| PathBuf::from(path))
        } else {
            PathBuf::from(path)
        };
        
        let key = resolved_path.to_string_lossy().to_string();
        
        // Check cache
        if let Some(val) = self.module_cache.get(&key) {
            return Ok(val.clone());
        }
        
        // Read file
        let source = std::fs::read_to_string(&resolved_path)
            .map_err(|e| anyhow::anyhow!("failed to read module {}: {}", key, e))?;
            
        // Compile
        let tokens = Lexer::new(&source).tokenize()?;
        let program = Parser::new(tokens).parse()?;
        let mut compiler = Compiler::new();
        let code = compiler.compile(&program);
        
        // Run module in a fresh VM (recursive)
        let mut vm = Vm::new(&code);
        // let module_id = format!("module:{}", key); // Virtual ID for module execution
        
        loop {
            match vm.run() {
                VmResult::Complete(v) => {
                    self.module_cache.insert(key, v.clone());
                    return Ok(v);
                }
                VmResult::Suspended { tool_name, arg, continuation } => {
                    // Recurse for imports inside modules!
                    if tool_name == "sys_import" {
                        let inner_path = match arg {
                            Value::Str(s) => s,
                            _ => "".to_string(),
                        };
                        let val = self.load_module(&inner_path, Some(&resolved_path))?;
                        vm = Vm::resume_with_result(continuation, &[], val);
                    } else {
                        // Normal tool call
                        // Checkpointing inside module loading?
                        // If we crash during module load, we probably just restart load.
                        // So we might skip saving state for modules for now to avoid complexity.
                        let result = match self.tools.call(&tool_name, arg) {
                            Some(v) => v,
                            None => Value::Null,
                        };
                        vm = Vm::resume_with_result(continuation, &[], result);
                    }
                }
            }
        }
    }

    pub fn run(&mut self, id: &str, source: &str, path: Option<PathBuf>) -> Result<Value> {
        // 1. Load or Init
        let loaded_state = self.store.load(id)?;
        
        // If we are starting fresh, we need to compile.
        // If resuming, the code is in the frames.
        // However, Vm::new needs a slice.
        // And Vm::resume needs state.
        
        let mut vm = if let Some(state) = loaded_state {
            Vm::resume_with_result(state, &[], Value::Null)
        } else {
            let tokens = Lexer::new(source).tokenize()?;
            let program = Parser::new(tokens).parse()?;
            let mut compiler = Compiler::new();
            let code = compiler.compile(&program);
            Vm::new(&code)
        };

        // 2. Loop
        loop {
            match vm.run() {
                VmResult::Complete(v) => {
                    return Ok(v);
                }
                VmResult::Suspended { tool_name, arg, continuation } => {
                    // Handle Import
                    if tool_name == "sys_import" {
                        // 3a. Save state (checkpoint)
                        self.store.save(id, &continuation)?;
                        
                        // 3b. Load Module
                        let import_path = match arg {
                            Value::Str(s) => s,
                            _ => "".to_string(),
                        };
                        
                        // Use the provided path as base, or CWD
                        let base_path = path.as_ref();
                        
                        let result = match self.load_module(&import_path, base_path) {
                            Ok(v) => v,
                            Err(e) => {
                                eprintln!("Error loading module {}: {}", import_path, e);
                                Value::Null
                            }
                        };
                        
                        vm = Vm::resume_with_result(continuation, &[], result);
                        continue;
                    }

                    // 3. Save state (checkpoint)
                    self.store.save(id, &continuation)?;
                    
                    // 4. Execute tool
                    let result = match self.tools.call(&tool_name, arg) {
                        Some(v) => v,
                        None => Value::Null,
                    };
                    
                    // 5. Resume
                    vm = Vm::resume_with_result(continuation, &[], result);
                }
            }
        }
    }
}
