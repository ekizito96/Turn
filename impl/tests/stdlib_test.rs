use turn::analysis::Analysis;
use turn::lexer::Lexer;
use turn::parser::Parser;

fn analyze(source: &str) -> Analysis {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut analysis = Analysis::new();
    analysis.analyze(&program);
    analysis
}

#[tokio::test]
async fn test_stdlib_return_types() {
    let source = r#"
    let content: Str = call("fs_read", "file.txt");
    let json: Any = call("json_parse", content);
    "#;
    let analysis = analyze(source);
    assert!(analysis.diagnostics.is_empty());
}

#[tokio::test]
async fn test_stdlib_type_mismatch() {
    let source = r#"
    let content: Num = call("fs_read", "file.txt");
    "#;
    let analysis = analyze(source);
    assert!(!analysis.diagnostics.is_empty());
    let (_, msg) = &analysis.diagnostics[0];
    assert!(msg.contains("Type mismatch"));
    assert!(msg.contains("expected Num"));
    assert!(msg.contains("got Str"));
}

#[tokio::test]
async fn test_single_param_function_inference() {
    let source = r#"
    let identity = turn(x: Num) -> Num {
        return x;
    };
    // 1 param, inferred arg type is Num. Ret is Num.
    // Function(Num, Num)
    let res: Num = call(identity, 10);
    "#;
    let analysis = analyze(source);
    assert!(analysis.diagnostics.is_empty());
}

#[tokio::test]
async fn test_single_param_function_mismatch() {
    let source = r#"
    let identity = turn(x: Num) -> Num {
        return x;
    };
    let res: Str = call(identity, 10);
    "#;
    let analysis = analyze(source);
    assert!(!analysis.diagnostics.is_empty());
    let (_, msg) = &analysis.diagnostics[0];
    assert!(msg.contains("Type mismatch"));
    assert!(msg.contains("expected Str"));
    assert!(msg.contains("got Num"));
}
