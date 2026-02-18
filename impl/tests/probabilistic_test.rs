use turn::vm::{Vm, VmResult};
use turn::value::Value;
use turn::compiler::Compiler;
use turn::parser::Parser;
use turn::lexer::Lexer;

fn run_code_with_env(source: &str, env_setup: impl FnOnce(&mut Vm)) -> Value {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexer failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser failed");
    let mut compiler = Compiler::new();
    let code = compiler.compile(&program);
    let mut vm = Vm::new(&code);
    
    env_setup(&mut vm);
    
    let result = vm.run();
    match result {
        VmResult::Complete(v) => v,
        _ => panic!("VM did not complete: {:?}", result),
    }
}

#[test]
fn test_confidence_keyword() {
    let source = r#"
    let c = confidence x;
    return c;
    "#;
    
    let res = run_code_with_env(source, |vm| {
        vm.runtime.env.insert("x".to_string(), Value::Uncertain(Box::new(Value::Num(10.0)), 0.8));
    });
    
    if let Value::Num(n) = res {
        assert_eq!(n, 0.8);
    } else {
        panic!("Expected Num, got {:?}", res);
    }
}

#[test]
fn test_confidence_of_certain_value() {
    let source = "let c = confidence 42; return c;";
    let res = run_code_with_env(source, |_| {});
    
    if let Value::Num(n) = res {
        assert_eq!(n, 1.0);
    } else {
        panic!("Expected Num(1.0), got {:?}", res);
    }
}

#[test]
fn test_uncertainty_propagation_add() {
    let source = r#"
    let z = x + y;
    return confidence z;
    "#;
    
    let res = run_code_with_env(source, |vm| {
        vm.runtime.env.insert("x".to_string(), Value::Uncertain(Box::new(Value::Num(10.0)), 0.8));
        vm.runtime.env.insert("y".to_string(), Value::Uncertain(Box::new(Value::Num(5.0)), 0.5));
    });
    
    // 0.8 * 0.5 = 0.4
    if let Value::Num(n) = res {
        assert!((n - 0.4).abs() < 1e-6);
    } else {
        panic!("Expected Num(0.4), got {:?}", res);
    }
}

#[test]
fn test_uncertainty_propagation_add_mixed() {
    let source = r#"
    let z = x + 5;
    return confidence z;
    "#;
    
    let res = run_code_with_env(source, |vm| {
        vm.runtime.env.insert("x".to_string(), Value::Uncertain(Box::new(Value::Num(10.0)), 0.8));
    });
    
    // 0.8 * 1.0 = 0.8
    if let Value::Num(n) = res {
        assert!((n - 0.8).abs() < 1e-6);
    } else {
        panic!("Expected Num(0.8), got {:?}", res);
    }
}

#[test]
fn test_uncertainty_nested_logic() {
    // (x AND y)
    // x = Uncertain(true, 0.9)
    // y = Uncertain(false, 0.5)
    // true AND false -> false
    // Probability: 0.9 * 0.5 = 0.45
    
    let source = r#"
    let res = x && y;
    return confidence res;
    "#;
    
    let res = run_code_with_env(source, |vm| {
        vm.runtime.env.insert("x".to_string(), Value::Uncertain(Box::new(Value::Bool(true)), 0.9));
        vm.runtime.env.insert("y".to_string(), Value::Uncertain(Box::new(Value::Bool(false)), 0.5));
    });
    
    if let Value::Num(n) = res {
        assert!((n - 0.45).abs() < 1e-6);
    } else {
        panic!("Expected Num(0.45), got {:?}", res);
    }
}
