pub mod ast;
pub mod bytecode;
pub mod compiler;
pub mod lexer;
pub mod parser;
pub mod runner;
pub mod runtime;
pub mod server;
pub mod store;
pub mod tools;
pub mod value;
pub mod vm;

pub use ast::*;
pub use bytecode::*;
pub use compiler::*;
pub use lexer::*;
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
    let mut vm = vm::Vm::new(&code);
    loop {
        match vm.run() {
            vm::VmResult::Complete(v) => return Ok(v),
            vm::VmResult::Suspended {
                tool_name,
                arg,
                continuation,
            } => {
                // Execute tool (synchronously for now, but design allows async/pause)
                let result = match tools.call(&tool_name, arg) {
                    Some(v) => v,
                    None => value::Value::Null,
                };

                // Resume execution with result
                vm = vm::Vm::resume_with_result(continuation, &code, result);
            }
        }
    }
}
