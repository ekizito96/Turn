use turn::{compiler, lexer, parser, tools, Value, Vm, VmResult};

#[test]
fn test_manual_suspension_resume_cycle() {
    let source = r#"
    turn {
        let x = call("my_tool", "ping");
        return x;
    }
    "#;

    // 1. Compile
    let tokens = lexer::Lexer::new(source).tokenize().unwrap();
    let program = parser::Parser::new(tokens).parse().unwrap();
    let mut compiler = compiler::Compiler::new();
    let code = compiler.compile(&program);

    // 2. Setup Tools
    let tools = tools::ToolRegistry::new(); // empty registry is fine as we handle call manually

    // 3. Start VM
    let mut vm = Vm::new(&code);

    // 4. Run -> Suspend
    let continuation = match vm.run() {
        VmResult::Suspended {
            tool_name,
            arg,
            continuation,
        } => {
            assert_eq!(tool_name, "my_tool");
            assert_eq!(arg, Value::Str("ping".to_string()));
            println!("Suspended on tool call. Saving state...");
            continuation
        }
        _ => panic!("Expected suspension, got completion"),
    };

    // 5. Simulate "Long Pause" (e.g. human approval or slow API)
    // The 'continuation' struct holds the entire frozen state of the agent.

    // 6. Resume with Result "pong"
    println!("Resuming with result 'pong'...");
    let mut resumed_vm =
        Vm::resume_with_result(continuation, &code, Value::Str("pong".to_string()));

    // 7. Run -> Complete
    match resumed_vm.run() {
        VmResult::Complete(val) => {
            assert_eq!(val, Value::Str("pong".to_string()));
            println!("Completed successfully.");
        }
        _ => panic!("Expected completion, got suspension"),
    }
}
