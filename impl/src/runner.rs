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
            parent.join(path) // Don't canonicalize yet to keep relative paths simple?
            // Actually canonicalize is good for unique keys.
        } else {
            PathBuf::from(path)
        };
        
        // Canonicalize to absolute path for cache key
        let abs_path = match std::fs::canonicalize(&resolved_path) {
            Ok(p) => p,
            Err(_) => resolved_path.clone(), // Fallback if file doesn't exist (will fail read later)
        };
        
        let key = abs_path.to_string_lossy().to_string();
        
        // Check cache
        if let Some(val) = self.module_cache.get(&key) {
            return Ok(val.clone());
        }
        
        // Read file
        let source = std::fs::read_to_string(&abs_path)
            .map_err(|e| anyhow::anyhow!("Failed to read module {}: {}", key, e))?;
            
        // Compile
        let lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()
            .map_err(|e| anyhow::anyhow!("Lexer error in module {}: {}", key, e))?;
            
        let mut parser = Parser::new(tokens);
        let program = parser.parse()
            .map_err(|e| anyhow::anyhow!("Parser error in module {}: {}", key, e))?;
            
        let mut compiler = Compiler::new();
        let code = compiler.compile(&program);
        
        // Run module in a fresh VM (recursive)
        let mut vm = Vm::new(&code);
        
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
                            Value::Str(s) => s.clone(),
                            _ => "".to_string(),
                        };
                        match self.load_module(&inner_path, Some(&abs_path)) {
                            Ok(val) => {
                                vm = Vm::resume_with_result(continuation, val);
                            },
                            Err(e) => {
                                vm = Vm::resume_with_error(continuation, e.to_string());
                            }
                        }
                    } else {
                        // Normal tool call
                        match self.tools.call(&tool_name, arg) {
                            Ok(val) => {
                                vm = Vm::resume_with_result(continuation, val);
                            },
                            Err(e) => {
                                vm = Vm::resume_with_error(continuation, e);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn run(&mut self, id: &str, source: &str, path: Option<PathBuf>) -> Result<Value> {
        // 1. Load or Init
        // If resuming, we load from store.
        let mut vm = if let Ok(Some(state)) = self.store.load(id) {
            // Check if state is valid?
            // Resume with Null as "last result" - technically incorrect if we crashed mid-tool-return?
            // But good enough for now.
            Vm::resume_with_result(state, Value::Null)
        } else {
            let lexer = Lexer::new(source);
            let tokens = lexer.tokenize()
                 .map_err(|e| anyhow::anyhow!("Lexer error: {}", e))?;
                 
            let mut parser = Parser::new(tokens);
            let program = parser.parse()
                 .map_err(|e| anyhow::anyhow!("Parser error: {}", e))?;
                 
            let mut compiler = Compiler::new();
            let code = compiler.compile(&program);
            Vm::new(&code)
        };

        // 2. Loop
        loop {
            match vm.run() {
                VmResult::Complete(v) => {
                    // Clear store on successful completion?
                    // self.store.delete(id)?; 
                    // Keeping it allows inspecting final state or re-running?
                    return Ok(v);
                }
                VmResult::Suspended { tool_name, arg, continuation } => {
                    // Handle Import
                    if tool_name == "sys_import" {
                        // 3a. Save state (checkpoint)
                        self.store.save(id, &continuation)?;
                        
                        // 3b. Load Module
                        let import_path = match arg {
                            Value::Str(s) => s.clone(),
                            _ => "".to_string(),
                        };
                        
                        // Use the provided path as base, or CWD
                        let base_path = path.as_ref();
                        
                        match self.load_module(&import_path, base_path) {
                            Ok(val) => {
                                vm = Vm::resume_with_result(continuation, val);
                            },
                            Err(e) => {
                                vm = Vm::resume_with_error(continuation, e.to_string());
                            }
                        }
                        continue;
                    }

                    // 3. Save state (checkpoint)
                    self.store.save(id, &continuation)?;
                    
                    // 4. Execute tool
                    match self.tools.call(&tool_name, arg) {
                        Ok(val) => {
                             vm = Vm::resume_with_result(continuation, val);
                        },
                        Err(e) => {
                             vm = Vm::resume_with_error(continuation, e);
                        }
                    }
                }
            }
        }
    }
}
