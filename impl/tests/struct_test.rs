use turn::analysis::Analysis;
use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::compiler::Compiler;
use turn::vm::{Vm, VmResult};
use turn::value::Value;

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
fn test_struct_runtime_creation() {
    let source = r#"
    struct Point { x: Num, y: Num };
    let p = Point { x: 10, y: 20 };
    return p;
    "#;
    let val = run(source);
    if let Value::Struct(name, fields) = val {
        assert_eq!(name.as_str(), "Point");
        assert_eq!(fields.get("x"), Some(&Value::Num(10.0)));
        assert_eq!(fields.get("y"), Some(&Value::Num(20.0)));
    } else {
        panic!("Expected struct, got {:?}", val);
    }
}

#[test]
fn test_struct_method_runtime() {
    let source = r#"
    struct Point { x: Num, y: Num };
    impl Point {
        let sum = turn() -> Num {
            return x + y; // Implicit self access via context injection
        };
    };
    let p = Point { x: 10, y: 20 };
    return p.sum(); 
    "#;
    
    let val = run(source);
    assert_eq!(val, Value::Num(30.0));
}

#[test]
fn test_type_alias_usage() {
    let source = r#"
    type ID = Num;
    let x: ID = 123;
    "#;
    let analysis = analyze(source);
    assert!(analysis.diagnostics.is_empty());
}

#[test]
fn test_impl_def() {
    let source = r#"
    struct Point { x: Num, y: Num };
    impl Point {
        let dist = turn(p: Point) -> Num {
            return 0; // Placeholder
        };
    };
    "#;
    let analysis = analyze(source);
    assert!(analysis.diagnostics.is_empty());
}

#[test]
fn test_method_call_check() {
    let source = r#"
    struct Point { x: Num, y: Num };
    impl Point {
        let dist = turn(other: Point) -> Num {
            return 0;
        };
    };
    let p1 = Point { x: 1, y: 2 };
    let p2 = Point { x: 3, y: 4 };
    let d: Num = p1.dist(p2);
    "#;
    let analysis = analyze(source);
    assert!(analysis.diagnostics.is_empty());
}

#[test]
fn test_method_call_unknown() {
    let source = r#"
    struct Point { x: Num, y: Num };
    let p = Point { x: 1, y: 2 };
    let d = p.area();
    "#;
    let analysis = analyze(source);
    assert!(!analysis.diagnostics.is_empty());
    let (_, msg) = &analysis.diagnostics[0];
    assert!(msg.contains("Unknown method 'area'"));
}
