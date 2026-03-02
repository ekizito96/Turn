use turn::compiler::Compiler;
use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::value::Value;
use turn::vm::{Vm, VmResult};

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
fn test_vector_creation() {
    let source = "let v = vec[1, 2, 3]; return v;";
    let val = run(source);
    if let Value::Vec(v) = val {
        assert_eq!(v, vec![1.0, 2.0, 3.0]);
    } else {
        panic!("Expected Vec, got {:?}", val);
    }
}

#[test]
fn test_vector_addition() {
    let source = "let v1 = vec[1, 2]; let v2 = vec[3, 4]; return v1 + v2;";
    let val = run(source);
    if let Value::Vec(v) = val {
        assert_eq!(v, vec![4.0, 6.0]);
    } else {
        panic!("Expected Vec, got {:?}", val);
    }
}

#[test]
fn test_vector_scalar_mul() {
    let source = "let v = vec[1, 2]; return v * 2;";
    let val = run(source);
    if let Value::Vec(v) = val {
        assert_eq!(v, vec![2.0, 4.0]);
    } else {
        panic!("Expected Vec, got {:?}", val);
    }
}

#[test]
fn test_vector_dot_product() {
    let source = "let v1 = vec[1, 2]; let v2 = vec[3, 4]; return v1 * v2;";
    // 1*3 + 2*4 = 3 + 8 = 11
    let val = run(source);
    assert_eq!(val, Value::Num(11.0));
}

#[test]
fn test_vector_similarity() {
    let source = r#"
    let v1 = vec[1, 0];
    let v2 = vec[0, 1];
    return v1 ~> v2;
    "#;
    let val = run(source);
    assert_eq!(val, Value::Num(0.0)); // Orthogonal vectors

    let source2 = r#"
    let v1 = vec[1, 1];
    let v2 = vec[2, 2];
    return v1 ~> v2;
    "#;
    let val2 = run(source2);
    // Should be 1.0 (or very close)
    if let Value::Num(n) = val2 {
        assert!((n - 1.0).abs() < 0.00001);
    } else {
        panic!("Expected Num");
    }
}
