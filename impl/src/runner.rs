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
        // 1. Check embedded Standard Library
        if let Some(source) = crate::std_lib::get_module_source(path) {
            let key = format!("std::{}", path);
            if let Some(val) = self.module_cache.get(&key) {
                return Ok(val.clone());
            }

            // Compile std lib module
            let lexer = Lexer::new(source);
            let tokens = lexer
                .tokenize()
                .map_err(|e| anyhow::anyhow!("Lexer error in std module {}: {}", path, e))?;

            let mut parser = Parser::new(tokens);
            let program = parser.parse().map_err(|e| {
                let offset = e.offset();
                let snippet = if offset < source.len() {
                    &source[offset..std::cmp::min(offset + 20, source.len())]
                } else {
                    "EOF"
                };
                anyhow::anyhow!(
                    "Parser error in std module {}: {} at offset {} near '{}'",
                    path,
                    e,
                    offset,
                    snippet
                )
            })?;

            let mut compiler = Compiler::new();
            let code = compiler.compile(&program);
            
            println!("DEBUG RUNNER CODE SIZE: {}", code.len());

            // Run in fresh VM
            let mut vm = Vm::new(&code);
            loop {
                match vm.run() {
                    VmResult::Complete(v) => {
                        self.module_cache.insert(key, v.clone());
                        return Ok(v);
                    }
                    VmResult::Suspended {
                        tool_name,
                        arg,
                        continuation,
                    } => {
                        if tool_name == "sys_import" {
                            let inner_path = match arg {
                                Value::Str(s) => s.clone(),
                                _ => "".to_string(),
                            };
                            // Std lib modules don't have a file path context
                            match self.load_module(&inner_path, None) {
                                Ok(val) => {
                                    vm = Vm::resume_with_result(continuation, val);
                                }
                                Err(e) => {
                                    vm = Vm::resume_with_error(continuation, e.to_string());
                                }
                            }
                        } else {
                            match self.tools.call(&tool_name, arg) {
                                Ok((val, cost)) => {
                                    let mut state = continuation;
                                    if state.gas_remaining >= cost {
                                        state.gas_remaining =
                                            state.gas_remaining.saturating_sub(cost);
                                        vm = Vm::resume_with_result(state, val);
                                    } else {
                                        vm = Vm::resume_with_error(
                                            state,
                                            "Token budget exhausted".to_string(),
                                        );
                                    }
                                }
                                Err(e) => {
                                    vm = Vm::resume_with_error(continuation, e);
                                }
                            }
                        }
                    }
                    VmResult::Yielded => unreachable!("VM should handle yields internally"),
                }
            }
        }

        // Resolve path relative to current file if provided
        let mut resolved_path = if let Some(base) = current_file {
            let parent = base.parent().unwrap_or(std::path::Path::new("."));
            parent.join(path)
        } else {
            PathBuf::from(path)
        };

        // If file doesn't exist, and path looks like a package import (no path separators),
        // try looking in .turn_modules by walking up the directory tree
        if !resolved_path.exists() {
            let is_package_import =
                !path.contains('/') && !path.contains('\\') && !path.starts_with('.');
            if is_package_import {
                // Search up from current_file (or CWD if None)
                let mut search_dir = if let Some(base) = current_file {
                    // Start from file's directory
                    base.parent()
                        .unwrap_or(std::path::Path::new("."))
                        .to_path_buf()
                } else {
                    std::env::current_dir().unwrap_or(PathBuf::from("."))
                };

                // Limit loop to avoid infinite loop on weird filesystems
                for _ in 0..20 {
                    // Try .tn first, then .turn
                    let pkg_path_tn = search_dir
                        .join(".turn_modules")
                        .join(format!("{}.tn", path));
                    if pkg_path_tn.exists() {
                        resolved_path = pkg_path_tn;
                        break;
                    }

                    let pkg_path_turn = search_dir
                        .join(".turn_modules")
                        .join(format!("{}.turn", path));
                    if pkg_path_turn.exists() {
                        resolved_path = pkg_path_turn;
                        break;
                    }

                    if !search_dir.pop() {
                        break;
                    }
                }
            }
        }

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
        let tokens = lexer
            .tokenize()
            .map_err(|e| anyhow::anyhow!("Lexer error in module {}: {}", key, e))?;

        let mut parser = Parser::new(tokens);
        let program = parser
            .parse()
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
                VmResult::Suspended {
                    tool_name,
                    arg,
                    continuation,
                } => {
                    // Recurse for imports inside modules!
                    if tool_name == "sys_import" {
                        let inner_path = match arg {
                            Value::Str(s) => s.clone(),
                            _ => "".to_string(),
                        };
                        match self.load_module(&inner_path, Some(&abs_path)) {
                            Ok(val) => {
                                vm = Vm::resume_with_result(continuation, val);
                            }
                            Err(e) => {
                                vm = Vm::resume_with_error(continuation, e.to_string());
                            }
                        }
                    } else {
                        // Normal tool call
                        match self.tools.call(&tool_name, arg) {
                            Ok((val, cost)) => {
                                let mut state = continuation;
                                state.gas_remaining = state.gas_remaining.saturating_sub(cost);
                                vm = Vm::resume_with_result(state, val);
                            }
                            Err(e) => {
                                vm = Vm::resume_with_error(continuation, e);
                            }
                        }
                    }
                }
                VmResult::Yielded => unreachable!("VM should handle yields internally"),
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
            let tokens = lexer
                .tokenize()
                .map_err(|e| anyhow::anyhow!("Lexer error: {}", e))?;

            let mut parser = Parser::new(tokens);
            let program = parser
                .parse()
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
                VmResult::Suspended {
                    tool_name,
                    arg,
                    continuation,
                } => {
                    // Handle Suspend
                    if tool_name == "sys_suspend" {
                        self.store.save(id, &continuation)?;
                        return Ok(Value::Null);
                    }

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
                            }
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
                        Ok((val, cost)) => {
                            let mut state = continuation;
                            state.gas_remaining = state.gas_remaining.saturating_sub(cost);
                            vm = Vm::resume_with_result(state, val);
                        }
                        Err(e) => {
                            vm = Vm::resume_with_error(continuation, e);
                        }
                    }
                }
                VmResult::Yielded => unreachable!("VM should handle yields internally"),
            }
        }
    }
}
