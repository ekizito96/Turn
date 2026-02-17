pub mod ast;
pub mod bytecode;
pub mod compiler;
pub mod lexer;
pub mod parser;
pub mod runtime;
pub mod tools;
pub mod value;
pub mod vm;

pub use ast::*;
pub use bytecode::*;
pub use compiler::*;
pub use lexer::*;
pub use parser::*;
pub use runtime::*;
pub use tools::*;
pub use value::*;
pub use vm::*;

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
    let mut vm = vm::Vm::new(&code, tools);
    match vm.run() {
        vm::VmResult::Complete(v) => Ok(v),
        vm::VmResult::Suspended { .. } => {
            Err("Suspension not yet implemented (tool call)".into())
        }
    }
}
