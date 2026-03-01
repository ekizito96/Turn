use turn::vm::{Vm, VmResult};
use turn::value::Value;
use turn::compiler::Compiler;
use turn::parser::Parser;
use turn::lexer::Lexer;

fn run_code_with_env(source: &str, env_setup: impl FnOnce(&mut Vm)) -> Value {
    let lexer = Lexer::new(source);
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
fn test_uncertainty_not() {
    // !Uncertain(false, 0.8) -> Uncertain(true, 0.8)
    let source = r#"
    let x_val = !x;
    return confidence x_val;
    "#;
    
    let res = run_code_with_env(source, |vm| {
        if let Some(p) = vm.scheduler.front_mut() {
            p.runtime.env.insert("x".to_string(), Value::Uncertain(Box::new(Value::Bool(false)), 0.8));
        }
    });
    
    if let Value::Num(n) = res {
        assert!((n - 0.8).abs() < 1e-6);
    } else {
        panic!("Expected Num(0.8), got {:?}", res);
    }
}

#[test]
fn test_uncertainty_eq() {
    // Uncertain(5, 0.9) == Uncertain(5, 0.8) -> Uncertain(true, 0.72)
    let source = r#"
    let res = x == y;
    return confidence res;
    "#;
    
    let res = run_code_with_env(source, |vm| {
        if let Some(p) = vm.scheduler.front_mut() {
            p.runtime.env.insert("x".to_string(), Value::Uncertain(Box::new(Value::Num(5.0)), 0.9));
            p.runtime.env.insert("y".to_string(), Value::Uncertain(Box::new(Value::Num(5.0)), 0.8));
        }
    });
    
    if let Value::Num(n) = res {
        // We changed logic to use Zadeh's T-norms (min/max) instead of multiplication!
        // min(0.9, 0.8) = 0.8
        assert!((n - 0.8).abs() < 1e-6);
    } else {
        panic!("Expected Num(0.8), got {:?}", res);
    }
}

#[test]
fn test_uncertainty_ne() {
    // Uncertain(5, 0.9) != Uncertain(10, 0.8) -> Uncertain(true, 0.8)
    // 5 != 10 is true.
    let source = r#"
    let res = x != y;
    return confidence res;
    "#;
    
    let res = run_code_with_env(source, |vm| {
        if let Some(p) = vm.scheduler.front_mut() {
            p.runtime.env.insert("x".to_string(), Value::Uncertain(Box::new(Value::Num(5.0)), 0.9));
            p.runtime.env.insert("y".to_string(), Value::Uncertain(Box::new(Value::Num(10.0)), 0.8));
        }
    });
    
    if let Value::Num(n) = res {
        assert!((n - 0.8).abs() < 1e-6);
    } else {
        panic!("Expected Num(0.8), got {:?}", res);
    }
}
