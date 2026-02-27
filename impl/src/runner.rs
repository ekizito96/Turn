use crate::compiler::Compiler;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::store::Store;
use crate::tools::ToolRegistry;
use crate::value::Value;
use crate::vm::{Vm, VmEvent};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use wasmtime::*;

pub struct Runner<S: Store> {
    store: S,
    tools: ToolRegistry,
    module_cache: HashMap<String, Value>,
    injected_caps: HashMap<String, String>,
    wasm_engine: wasmtime::Engine,
    wasm_store: wasmtime::Store<()>,
    wasm_instances: std::collections::HashMap<String, wasmtime::Instance>,
}

impl<S: Store + std::marker::Send> Runner<S> {
    pub fn new(store: S, tools: ToolRegistry) -> Self {
        let engine = wasmtime::Engine::default();
        let wasm_store = wasmtime::Store::new(&engine, ());
        
        Self {
            store,
            tools,
            module_cache: HashMap::new(),
            injected_caps: HashMap::new(),
            wasm_engine: engine,
            wasm_store,
            wasm_instances: std::collections::HashMap::new(),
        }
    }

    pub fn inject_capability(&mut self, name: &str, secret: &str) {
        self.injected_caps
            .insert(name.to_string(), secret.to_string());
    }

    #[allow(clippy::needless_lifetimes)]
    pub fn load_module<'a>(
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
                let mut program = parser.parse().map_err(|e| {
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

                crate::macro_engine::MacroEngine::expand(&mut program).await?;

                let mut compiler = Compiler::new();
                let code = compiler.compile(&program);

                let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();
                let vm = Vm::new(&code, host_tx);
                let registry = vm.registry.clone();
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
                                pid,
                                tool_name,
                                arg,
                                resume_tx,
                                continuation: _,
                                expected_type: _,
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
                                            self.dispatch_trace_event(&registry, pid, &tool_name, &val);
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

            // 2. Check if URL module (HTTPS fetches)
            if path.starts_with("https://") || path.starts_with("http://") {
                let path_str = path.to_string();
                let cache_dir = std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".turn_cache")
                    .join("ast");
                if !cache_dir.exists() {
                    let _ = std::fs::create_dir_all(&cache_dir);
                }

                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(path_str.as_bytes());
                let hash_str = format!("{:x}", hasher.finalize());
                let cache_file = cache_dir.join(format!("{}.tn", hash_str));

                let source = if cache_file.exists() {
                    std::fs::read_to_string(&cache_file)
                        .map_err(|e| anyhow::anyhow!("Cache read error for {}: {}", path_str, e))?
                } else {
                    let resp = reqwest::get(&path_str).await
                        .map_err(|e| anyhow::anyhow!("HTTP request failed for {}: {}", path_str, e))?;
                    if !resp.status().is_success() {
                        anyhow::bail!("Failed to fetch URL {}: HTTP {}", path_str, resp.status());
                    }
                    let text = resp.text().await
                        .map_err(|e| anyhow::anyhow!("HTTP body error for {}: {}", path_str, e))?;
                    let _ = std::fs::write(&cache_file, &text);
                    text
                };

                let key = path_str.clone();
                if let Some(val) = self.module_cache.get(&key) {
                    return Ok(val.clone());
                }

                let lexer = Lexer::new(&source);
                let tokens = lexer.tokenize()
                    .map_err(|e| anyhow::anyhow!("Lexer error in URL module {}: {}", path_str, e))?;

                let mut parser = Parser::new(tokens);
                let mut program = parser.parse()
                    .map_err(|e| anyhow::anyhow!("Parser error in URL module {}: {}", path_str, e))?;

                crate::macro_engine::MacroEngine::expand(&mut program).await?;

                let mut compiler = Compiler::new();
                let code = compiler.compile(&program);

                let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();
                let vm = Vm::new(&code, host_tx);
                let registry = vm.registry.clone();
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
                                    return Err(anyhow::anyhow!("VM Error in URL module {}: {}", path_str, error));
                                }
                            }
                            VmEvent::Suspend { pid, tool_name, arg, resume_tx, continuation: _, expected_type: _ } => {
                                if tool_name == "sys_import" {
                                    let inner_path = match arg { Value::Str(s) => s.to_string(), _ => "".to_string() };
                                    match self.load_module(&inner_path, None).await {
                                        Ok(val) => { let _ = resume_tx.send(val); }
                                        Err(e) => { let _ = resume_tx.send(Value::Str(std::sync::Arc::new(e.to_string()))); }
                                    }

                                } else {
                                    let result = tokio::task::block_in_place(|| self.tools.call(&tool_name, arg));
                                    match result {
                                        Ok(val) => {
                                            self.dispatch_trace_event(&registry, pid, &tool_name, &val);
                                            let _ = resume_tx.send(val);
                                        }
                                        Err(e) => { let _ = resume_tx.send(Value::Str(std::sync::Arc::new(e))); }
                                    }
                                }
                            }
                        }
                    } else {
                        return Err(anyhow::anyhow!("VM unexpectedly halted in URL module {}", path_str));
                    }
                }
            }

            // 3. Resolve path relative to current file if provided
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
                        let pkg_path_tn = search_dir
                            .join(".turn_modules")
                            .join(format!("{}.tn", path));
                        if pkg_path_tn.exists() {
                            resolved_path = pkg_path_tn;
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
            let mut program = parser
                .parse()
                .map_err(|e| anyhow::anyhow!("Parser error in module {}: {}", key, e))?;

            crate::macro_engine::MacroEngine::expand(&mut program).await?;

            let mut compiler = Compiler::new();
            let code = compiler.compile(&program);

            // Run module in a fresh VM (recursive)
            let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();
            let vm = Vm::new(&code, host_tx);
            let registry = vm.registry.clone();
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
                            pid,
                            tool_name,
                            arg,
                            resume_tx,
                            continuation: _,
                            expected_type: _,
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
                                        self.dispatch_trace_event(&registry, pid, &tool_name, &val);
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

        // Turn enforces strict boundaries: `run` starts new jobs; `resume` wakes suspended ones.
        // We let the previous state be overwritten implicitly on next suspend.
        
        let lexer = Lexer::new(source);
        let tokens = lexer
            .tokenize()
            .map_err(|e| anyhow::anyhow!("Lexer error: {}", e))?;

        let mut parser = Parser::new(tokens);
        let mut program = parser
            .parse()
            .map_err(|e| anyhow::anyhow!("Parser error: {}", e))?;

        crate::macro_engine::MacroEngine::expand(&mut program).await?;

        let mut compiler = Compiler::new();
        let code = compiler.compile(&program);

        let vm = Vm::new(&code, host_tx);
        let registry = vm.registry.clone();
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
                        pid,
                        tool_name,
                        arg,
                        resume_tx,
                        continuation,
                        expected_type: _,
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


                        if tool_name == "sys_wasm_adapter" {
                            match self.install_wasm_component(arg) {
                                Ok(val) => { let _ = resume_tx.send(val); }
                                Err(e) => { let _ = resume_tx.send(Value::Str(std::sync::Arc::new(e))); }
                            }
                            continue;
                        }

                        if tool_name == "sys_wasm_call" {
                            match self.invoke_wasm_component(arg) {
                                Ok(val) => { let _ = resume_tx.send(val); }
                                Err(e) => { let _ = resume_tx.send(Value::Str(std::sync::Arc::new(e))); }
                            }
                            continue;
                        }

                        if let Some(c) = &*continuation {
                            self.store.save(id, c)?;
                        }

                        match self.tools.call(&tool_name, arg) {
                            Ok(val) => {
                                self.dispatch_trace_event(&registry, pid, &tool_name, &val);
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

    pub async fn resume(
        &mut self,
        id: &str,
        input: serde_json::Value,
    ) -> anyhow::Result<Value> {
        let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();

        let vm = if let Ok(Some(mut state)) = self.store.load(id) {
            // Cognitive Type Check: The Host violently rejects structurally mismatched data
            if let Some(expected_type) = &state.pending_suspend_type {
                match crate::llm_tools::json_value_to_turn_value(expected_type, &input) {
                    Ok(validated_value) => {
                        state.stack.push(validated_value);
                    }
                    Err(_) => {
                        return Err(anyhow::anyhow!(
                            "Cognitive Schema mismatch: injected data {:?} failed to coerce to {:?}",
                            input, expected_type
                        ));
                    }
                }
            } else {
                state.stack.push(
                    crate::llm_tools::json_value_to_turn_value(&crate::ast::Type::Any, &input)
                        .unwrap_or(Value::Null),
                );
            }
            state.pending_suspend_type = None;

            let temp_path = format!(".turn_tmp_{}.json", id);
            let data = serde_json::to_string(&state)?;
            std::fs::write(&temp_path, data)?;
            let vm = Vm::resume_from_disk(&temp_path, host_tx)?;
            let _ = std::fs::remove_file(&temp_path);
            vm
        } else {
            return Err(anyhow::anyhow!("No suspended state found on disk for {}", id));
        };

        let registry = vm.registry.clone();
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
                        pid,
                        tool_name,
                        arg,
                        resume_tx,
                        continuation,
                        expected_type: _,
                    } => {
                        if tool_name == "sys_suspend" {
                            if let Some(c) = *continuation {
                                self.store.save(id, &c)?;
                            }
                            return Ok(Value::Null); // Halts completely
                        }

                        if tool_name == "sys_import" {
                            if let Some(c) = &*continuation {
                                self.store.save(id, c)?;
                            }

                            let import_path = match arg {
                                Value::Str(s) => s.to_string(),
                                _ => "".to_string(),
                            };
                            match self.load_module(&import_path, None).await {
                                Ok(val) => {
                                    let _ = resume_tx.send(val);
                                }
                                Err(e) => {
                                    let _ = resume_tx.send(Value::Str(std::sync::Arc::new(
                                        e.to_string(),
                                    )));
                                }
                            }


                        } else if tool_name == "sys_wasm_adapter" {
                            match self.install_wasm_component(arg) {
                                Ok(val) => { let _ = resume_tx.send(val); }
                                Err(e) => { let _ = resume_tx.send(Value::Str(std::sync::Arc::new(e))); }
                            }
                        } else if tool_name == "sys_wasm_call" {
                            match self.invoke_wasm_component(arg) {
                                Ok(val) => { let _ = resume_tx.send(val); }
                                Err(e) => { let _ = resume_tx.send(Value::Str(std::sync::Arc::new(e))); }
                            }
                        } else {
                            let result = tokio::task::block_in_place(|| {
                                self.tools.call(&tool_name, arg)
                            });
                            match result {
                                Ok(val) => {
                                    self.dispatch_trace_event(&registry, pid, &tool_name, &val);
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
    }

    // NEW Phase 5: Helper mapping telemetry to registered tracing loops
    fn dispatch_trace_event(&self, registry: &crate::vm::Registry, pid: u64, tool_name: &str, result: &Value) {
        let tracers = registry.get_tracers(pid);
        if !tracers.is_empty() {
            let mut map = indexmap::IndexMap::new();
            map.insert("type".to_string(), Value::Str(std::sync::Arc::new("TraceEvent".to_string())));
            map.insert("tool".to_string(), Value::Str(std::sync::Arc::new(tool_name.to_string())));
            
            match result {
                Value::ToolCallRequest(func, _) => {
                    map.insert("action".to_string(), Value::Str(std::sync::Arc::new("ToolCall".to_string())));
                    map.insert("func".to_string(), Value::Str(std::sync::Arc::new(func.clone())));
                }
                Value::Struct(name, _) => {
                    map.insert("action".to_string(), Value::Str(std::sync::Arc::new("Thought".to_string())));
                    map.insert("result_type".to_string(), Value::Str(name.clone()));
                }
                _ => {
                    map.insert("action".to_string(), Value::Str(std::sync::Arc::new("Thought".to_string())));
                }
            }
            
            let trace_val = Value::Struct(std::sync::Arc::new("TraceEvent".to_string()), std::sync::Arc::new(map));
            for t in tracers {
                registry.send(t, trace_val.clone());
            }
        }
    }

    fn install_wasm_component(&mut self, arg: Value) -> Result<Value, String> {
        if let Value::Map(m) = arg {
            if let Some(Value::Str(url)) = m.get("url") {
                let url_str = url.to_string();
                
                let module = wasmtime::Module::from_file(&self.wasm_engine, &url_str)
                    .map_err(|e| format!("Wasm fetch failed: {}", e))?;
                
                let instance = wasmtime::Instance::new(&mut self.wasm_store, &module, &[])
                    .map_err(|e| format!("Wasm instantiation failed: {}", e))?;
                    
                self.wasm_instances.insert(url_str.clone(), instance);
                
                let mut proxy_methods = indexmap::IndexMap::new();
                for export in module.exports() {
                    if let wasmtime::ExternType::Func(f_ty) = export.ty() {
                        let fn_name = export.name().to_string();
                        let param_count = f_ty.params().len();
                        
                        let mut params = vec![];
                        let mut code = vec![];
                        
                        for i in 0..param_count {
                            let p_name = format!("p{}", i);
                            params.push((p_name.clone(), None, false));
                        }
                        
                        // Stack bottom -> top
                        code.push(crate::bytecode::Instr::PushStr("sys_wasm_call".to_string()));
                        
                        // MAP PAIR 1: "args" -> [p0, p1...]
                        code.push(crate::bytecode::Instr::PushStr("args".to_string()));
                        for i in 0..param_count {
                            let p_name = format!("p{}", i);
                            code.push(crate::bytecode::Instr::Load(p_name));
                        }
                        code.push(crate::bytecode::Instr::MakeList(param_count));
                        
                        // MAP PAIR 2: "url" -> "<url>"
                        code.push(crate::bytecode::Instr::PushStr("url".to_string()));
                        code.push(crate::bytecode::Instr::PushStr(url_str.clone()));
                        
                        // MAP PAIR 3: "func" -> "<fn_name>"
                        code.push(crate::bytecode::Instr::PushStr("func".to_string()));
                        code.push(crate::bytecode::Instr::PushStr(fn_name.clone()));
                        
                        // Build the map
                        code.push(crate::bytecode::Instr::MakeMap(3));
                        
                        // Call the tool ("sys_wasm_call" is below the Map)
                        code.push(crate::bytecode::Instr::CallTool);
                        code.push(crate::bytecode::Instr::Return);
                        
                        let closure = Value::Closure {
                            is_tool: false,
                            code: std::sync::Arc::new(code),
                            ip: 0,
                            env: std::collections::HashMap::new(),
                            params,
                        };
                        
                        proxy_methods.insert(fn_name, closure);
                    }
                }
                return Ok(Value::Map(std::sync::Arc::new(proxy_methods)));
            }
        }
        Err("Invalid sys_wasm_adapter map".to_string())
    }

    fn invoke_wasm_component(&mut self, arg: Value) -> Result<Value, String> {
        if let Value::Map(m) = arg {
            if let (Some(Value::Str(url)), Some(Value::Str(fname)), Some(Value::List(args_val))) = (m.get("url"), m.get("func"), m.get("args")) {
                let url_str = url.to_string();
                let fn_name = fname.to_string();
                
                if let Some(instance) = self.wasm_instances.get(&url_str) {
                    let func = instance.get_func(&mut self.wasm_store, &fn_name)
                        .ok_or_else(|| format!("Function {} not found in Wasm module", fn_name))?;
                    
                    let mut wasm_args = vec![];
                    for a in args_val.iter() {
                        if let Value::Num(n) = a {
                            wasm_args.push(wasmtime::Val::I32(*n as i32));
                        } else {
                            return Err("Only numeric arguments are supported in MVP Wasm bindings".to_string());
                        }
                    }
                    
                    let mut results = vec![wasmtime::Val::I32(0)];
                    func.call(&mut self.wasm_store, &wasm_args, &mut results)
                        .map_err(|e| format!("Wasm call panicked: {}", e))?;
                        
                    if let Some(res) = results.first() {
                        if let wasmtime::Val::I32(i) = res {
                            return Ok(Value::Num(*i as f64));
                        }
                    }
                    return Ok(Value::Null);
                }
                return Err(format!("Wasm instance {} not loaded", url_str));
            }
        }
        Err("Invalid sys_wasm_call map arguments".to_string())
    }
}
