pub mod analysis;
pub mod ast;
pub mod bytecode;
pub mod compiler;
pub mod lexer;
pub mod llm_tools;
pub mod lsp;
pub mod parser;
pub mod runner;
pub mod runtime;
pub mod server;
pub mod std_lib;
pub mod store;
pub mod tools;
pub mod value;
pub mod vm;
pub mod wasm_host;
pub mod macro_engine;

pub use analysis::*;
pub use ast::*;
pub use bytecode::*;
pub use compiler::*;
pub use lexer::*;
pub use llm_tools::*;
pub use lsp::*;
pub use parser::*;
pub use runner::*;
pub use runtime::*;
pub use server::*;
pub use store::*;
pub use tools::*;
pub use value::*;
pub use vm::*;
pub use macro_engine::*;

/// Converts a byte offset into source to (line, column) for error messages.
pub fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in source.char_indices() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Converts (line, column) to byte offset.
pub fn line_col_to_offset(source: &str, line: usize, col: usize) -> Option<usize> {
    let mut current_line = 1;
    let mut current_col = 1;
    let mut offset = 0;

    for c in source.chars() {
        if current_line == line && current_col == col {
            return Some(offset);
        }

        offset += c.len_utf8();

        if c == '\n' {
            current_line += 1;
            current_col = 1;
        } else {
            current_col += 1;
        }
    }

    if current_line == line && current_col == col {
        return Some(offset);
    }

    None
}

pub fn run(source: &str) -> Result<value::Value, Box<dyn std::error::Error>> {
    run_with_tools(source, &tools::ToolRegistry::new())
}

pub fn run_with_tools(
    source: &str,
    tools: &tools::ToolRegistry,
) -> Result<value::Value, Box<dyn std::error::Error>> {
    let source_str = source.to_string();
    let tools_clone = tools.clone();

    let fut = async move {
        let tokens = lexer::Lexer::new(&source_str).tokenize()
            .map_err(|e| anyhow::anyhow!("Lexer error: {}", e))?;
        let mut program = parser::Parser::new(tokens).parse()
            .map_err(anyhow::Error::from)?;
        
        // Execute Wasm macros (Schema Adapters) before compilation
        crate::macro_engine::MacroEngine::expand(&mut program).await?;

        let mut compiler = compiler::Compiler::new();
        let code = compiler.compile(&program);

        let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();
        let vm = vm::Vm::new(&code, host_tx);
        let registry = vm.registry.clone(); // NEW Phase 5: Hook into VM state for telemetry dispatch
        vm.start().await;

        loop {
            if let Some(event) = host_rx.recv().await {
                match event {
                    vm::VmEvent::Complete { pid, result } => {
                        if pid == 1 {
                            return Ok(result);
                        }
                    }
                    vm::VmEvent::Error { pid, error } => {
                        if pid == 1 {
                            return Err(anyhow::anyhow!("VM Error: {}", error));
                        }
                    }
                    vm::VmEvent::Suspend {
                        pid,
                        tool_name,
                        arg,
                        resume_tx,
                        ..
                    } => {
                        let result = tokio::task::block_in_place(|| {
                            tools_clone.call(&tool_name, arg).unwrap_or_else(|e| {
                                value::Value::Str(std::sync::Arc::new(e.to_string()))
                            })
                        });
                        
                        // NEW Phase 5 (Pillar 2): Natively route execution trace events to attached processes
                        let tracers = registry.get_tracers(pid);
                        if !tracers.is_empty() {
                            let mut map = indexmap::IndexMap::new();
                            map.insert("type".to_string(), value::Value::Str(std::sync::Arc::new("TraceEvent".to_string())));
                            map.insert("tool".to_string(), value::Value::Str(std::sync::Arc::new(tool_name.clone())));
                            
                            match &result {
                                value::Value::ToolCallRequest(func, _args) => {
                                    map.insert("action".to_string(), value::Value::Str(std::sync::Arc::new("ToolCall".to_string())));
                                    map.insert("func".to_string(), value::Value::Str(std::sync::Arc::new(func.clone())));
                                }
                                value::Value::Struct(name, _fields) => {
                                    map.insert("action".to_string(), value::Value::Str(std::sync::Arc::new("Thought".to_string())));
                                    map.insert("result_type".to_string(), value::Value::Str(name.clone()));
                                }
                                _ => {
                                    map.insert("action".to_string(), value::Value::Str(std::sync::Arc::new("Thought".to_string())));
                                }
                            }
                            
                            let trace_val = value::Value::Struct(std::sync::Arc::new("TraceEvent".to_string()), std::sync::Arc::new(map));
                            for t in tracers {
                                registry.send(t, trace_val.clone());
                            }
                        }

                        let _ = resume_tx.send(result);
                    }
                }
            } else {
                return Err(anyhow::anyhow!("VM unexpectedly halted"));
            }
        }
    };

    let result = std::thread::scope(|s| {
        s.spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(fut)
        })
        .join()
        .unwrap()
    });

    result.map_err(|e| e.into())
}
