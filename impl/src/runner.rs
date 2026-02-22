use crate::compiler::Compiler;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::store::Store;
use crate::tools::ToolRegistry;
use crate::value::Value;
use crate::vm::{Vm, VmEvent};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Runner<S: Store> {
    store: S,
    tools: ToolRegistry,
    module_cache: HashMap<String, Value>,
    injected_caps: HashMap<String, String>,
}

impl<S: Store + std::marker::Send> Runner<S> {
    pub fn new(store: S, tools: ToolRegistry) -> Self {
        Self {
            store,
            tools,
            module_cache: HashMap::new(),
            injected_caps: HashMap::new(),
        }
    }

    pub fn inject_capability(&mut self, name: &str, secret: &str) {
        self.injected_caps
            .insert(name.to_string(), secret.to_string());
    }

    #[allow(clippy::needless_lifetimes)]
    fn load_module<'a>(
        &'a mut self,
        path: &'a str,
        current_file: Option<&'a PathBuf>,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<crate::value::Value>> + Send + 'a>,
    > {
        Box::pin(async move {
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

                let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();
                let vm = Vm::new(&code, host_tx);
                vm.start().await;

                loop {
                    if let Some(event) = host_rx.recv().await {
                        match event {
                            VmEvent::Complete { pid, result } => {
                                if pid == 1 {
                                    self.module_cache.insert(key, result.clone());
                                    return Ok(result);
                                }
                            }
                            VmEvent::Error { pid, error } => {
                                if pid == 1 {
                                    return Err(anyhow::anyhow!("VM Error: {}", error));
                                }
                            }
                            VmEvent::Suspend {
                                pid: _,
                                tool_name,
                                arg,
                                resume_tx,
                                continuation: _,
                            } => {
                                if tool_name == "sys_import" {
                                    let inner_path = match arg {
                                        Value::Str(s) => s.to_string(),
                                        _ => "".to_string(),
                                    };
                                    match self.load_module(&inner_path, None).await {
                                        Ok(val) => {
                                            let _ = resume_tx.send(val);
                                        }
                                        Err(e) => {
                                            let _ = resume_tx.send(Value::Str(
                                                std::sync::Arc::new(e.to_string()),
                                            ));
                                        }
                                    }
                                } else {
                                    let result = tokio::task::block_in_place(|| {
                                        self.tools.call(&tool_name, arg)
                                    });
                                    match result {
                                        Ok(val) => {
                                            let _ = resume_tx.send(val);
                                        }
                                        Err(e) => {
                                            let _ =
                                                resume_tx.send(Value::Str(std::sync::Arc::new(e)));
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        return Err(anyhow::anyhow!("VM unexpectedly halted"));
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
            let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();
            let vm = Vm::new(&code, host_tx);
            vm.start().await;

            loop {
                if let Some(event) = host_rx.recv().await {
                    match event {
                        VmEvent::Complete { pid, result } => {
                            if pid == 1 {
                                self.module_cache.insert(key, result.clone());
                                return Ok(result);
                            }
                        }
                        VmEvent::Error { pid, error } => {
                            if pid == 1 {
                                return Err(anyhow::anyhow!("VM Error: {}", error));
                            }
                        }
                        VmEvent::Suspend {
                            pid: _,
                            tool_name,
                            arg,
                            resume_tx,
                            continuation: _,
                        } => {
                            if tool_name == "sys_import" {
                                let inner_path = match arg {
                                    Value::Str(s) => s.to_string(),
                                    _ => "".to_string(),
                                };
                                match self.load_module(&inner_path, Some(&abs_path)).await {
                                    Ok(val) => {
                                        let _ = resume_tx.send(val);
                                    }
                                    Err(e) => {
                                        let _ = resume_tx
                                            .send(Value::Str(std::sync::Arc::new(e.to_string())));
                                    }
                                }
                            } else {
                                let result = tokio::task::block_in_place(|| {
                                    self.tools.call(&tool_name, arg)
                                });
                                match result {
                                    Ok(val) => {
                                        let _ = resume_tx.send(val);
                                    }
                                    Err(e) => {
                                        let _ = resume_tx.send(Value::Str(std::sync::Arc::new(e)));
                                    }
                                }
                            }
                        }
                    }
                } else {
                    return Err(anyhow::anyhow!("VM unexpectedly halted"));
                }
            }
        })
    }

    pub async fn run(
        &mut self,
        id: &str,
        source: &str,
        path: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Value> {
        let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();

        let vm = if let Ok(Some(state)) = self.store.load(id) {
            // Write to disk temporarily and use resume_from_disk
            let temp_path = format!(".turn_tmp_{}.json", id);
            let data = serde_json::to_string(&state)?;
            std::fs::write(&temp_path, data)?;
            let vm = Vm::resume_from_disk(&temp_path, host_tx)?;
            let _ = std::fs::remove_file(&temp_path);
            vm
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

            Vm::new(&code, host_tx)
        };

        vm.start().await;

        loop {
            if let Some(event) = host_rx.recv().await {
                match event {
                    VmEvent::Complete { pid, result } => {
                        if pid == 1 {
                            return Ok(result);
                        }
                    }
                    VmEvent::Error { pid, error } => {
                        if pid == 1 {
                            return Err(anyhow::anyhow!("VM Error: {}", error));
                        }
                    }
                    VmEvent::Suspend {
                        pid: _,
                        tool_name,
                        arg,
                        resume_tx,
                        continuation,
                    } => {
                        if tool_name == "sys_suspend" {
                            if let Some(c) = *continuation {
                                self.store.save(id, &c)?;
                            }
                            return Ok(Value::Null);
                        }

                        if tool_name == "sys_import" {
                            if let Some(c) = &*continuation {
                                self.store.save(id, c)?;
                            }

                            let import_path = match arg {
                                Value::Str(s) => s.to_string(),
                                _ => "".to_string(),
                            };

                            let base_path = path.as_ref();
                            match self.load_module(&import_path, base_path).await {
                                Ok(val) => {
                                    let _ = resume_tx.send(val);
                                }
                                Err(e) => {
                                    let _ = resume_tx
                                        .send(Value::Str(std::sync::Arc::new(e.to_string())));
                                }
                            }
                            continue;
                        }

                        if let Some(c) = &*continuation {
                            self.store.save(id, c)?;
                        }

                        match self.tools.call(&tool_name, arg) {
                            Ok(val) => {
                                let _ = resume_tx.send(val);
                            }
                            Err(e) => {
                                let _ = resume_tx.send(Value::Str(std::sync::Arc::new(e)));
                            }
                        }
                    }
                }
            } else {
                return Err(anyhow::anyhow!("VM unexpectedly halted"));
            }
        }
    }
}
