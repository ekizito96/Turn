use turn::compiler::Compiler;
use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::vm::{Vm, VmResult};
use turn::value::Value;

fn run(source: &str) -> Value {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut compiler = Compiler::new();
    let code = compiler.compile(&program);
    let mut vm = Vm::new(&code);
    match vm.run() {
        VmResult::Complete(v) => v,
        _ => panic!("Suspended?"),
    }
}

#[test]
fn test_runtime_let_type_check() {
    let source = r#"
    let x: Num = "hello";
    "#;
    let result = run(source);
    // Should return the error string
    match result {
        Value::Str(s) => assert!(s.contains("Runtime Type Error")),
        _ => panic!("Expected type error string, got {:?}", result),
    }
}

#[test]
fn test_runtime_param_type_check() {
    let source = r#"
    let f = turn(x: Num) {
        return x;
    };
    call(f, { "x": "not a number" });
    "#;
    let result = run(source);
    match result {
        Value::Str(s) => assert!(s.contains("Runtime Type Error")),
        v => panic!("Expected type error string, got {:?}", v),
    }
}

#[test]
fn test_runtime_param_type_check_pass() {
    let source = r#"
    let f = turn(x: Num) {
        return x + 1;
    };
    return call(f, { "x": 10 });
    "#;
    let result = run(source);
    match result {
        Value::Num(n) => assert_eq!(n, 11.0),
        v => panic!("Expected number, got {:?}", v),
    }
}

#[test]
fn test_runtime_any_pass() {
    let source = r#"
    let x: Any = "hello";
    let y: Any = 123;
    "#;
    let result = run(source);
    assert!(matches!(result, Value::Null));
}
