use turn::analysis::Analysis;
use turn::compiler::Compiler;
use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::value::Value;
use turn::vm::{Vm, VmResult};

fn analyze(source: &str) -> Analysis {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut analysis = Analysis::new();
    analysis.analyze(&program);
    analysis
}

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
fn test_parse_generics() {
    let source = r#"
    let list: List<Num> = [1, 2, 3];
    let map: Map<Str> = { "key": "value" };
    "#;
    let analysis = analyze(source);
    assert!(analysis.diagnostics.is_empty());
}

#[test]
fn test_analysis_nested_generics_mismatch() {
    let source = r#"
    let list: List<Num> = ["string", "oops"];
    "#;
    let analysis = analyze(source);
    assert!(!analysis.diagnostics.is_empty());
    let (_, msg) = &analysis.diagnostics[0];
    assert!(msg.contains("Type mismatch"));
}

#[test]
fn test_runtime_generic_list_check() {
    let source = r#"
    let l: List<Num> = [1, 2, 3];
    return l;
    "#;
    let result = run(source);
    match result {
        Value::List(items) => assert_eq!(items.len(), 3),
        _ => panic!("Expected list"),
    }
}

#[test]
fn test_runtime_generic_list_fail() {
    let source = r#"
    let l: List<Num> = [1, "two", 3];
    "#;
    let result = run(source);
    match result {
        Value::Str(s) => assert!(s.contains("Runtime Type Error")),
        _ => panic!("Expected runtime error"),
    }
}

#[test]
fn test_runtime_generic_map_check() {
    let source = r#"
    let m: Map<Num> = { "a": 1, "b": 2 };
    return m;
    "#;
    let result = run(source);
    match result {
        Value::Map(m) => assert_eq!(m.len(), 2),
        _ => panic!("Expected map, got {:?}", result),
    }
}

#[test]
fn test_nested_generics() {
    let source = r#"
    let l: List<List<Num>> = [[1, 2], [3, 4]];
    return l;
    "#;
    let result = run(source);
    match result {
        Value::List(outer) => {
            assert_eq!(outer.len(), 2);
            match &outer[0] {
                Value::List(inner) => assert_eq!(inner.len(), 2),
                _ => panic!("Expected inner list"),
            }
        }
        _ => panic!("Expected outer list"),
    }
}

#[test]
fn test_runtime_generic_map_fail() {
    let source = r#"
    let m: Map<Num> = { "a": 1, "b": "two" };
    "#;
    let result = run(source);
    match result {
        Value::Str(s) => assert!(s.contains("Runtime Type Error")),
        _ => panic!("Expected runtime error"),
    }
}
