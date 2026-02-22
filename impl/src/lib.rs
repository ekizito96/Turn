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
    let tokens = lexer::Lexer::new(source).tokenize()?;
    let program = parser::Parser::new(tokens).parse()?;
    let mut compiler = compiler::Compiler::new();
    let code = compiler.compile(&program);
    Ok(tokio::runtime::Runtime::new().unwrap().block_on(async {
        let (host_tx, mut host_rx) = tokio::sync::mpsc::unbounded_channel();
        let vm = vm::Vm::new(&code, host_tx);
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
                            return Err(error.to_string());
                        }
                    }
                    vm::VmEvent::Suspend {
                        tool_name,
                        arg,
                        resume_tx,
                        ..
                    } => {
                        let result = tools.call(&tool_name, arg).unwrap_or_else(|e| {
                            value::Value::Str(std::sync::Arc::new(e.to_string()))
                        });
                        let _ = resume_tx.send(result);
                    }
                }
            } else {
                return Err("VM unexpectedly halted".to_string());
            }
        }
    })?)
}
