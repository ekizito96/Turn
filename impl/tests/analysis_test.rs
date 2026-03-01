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

#[test]
fn test_type_check_let() {
    let source = r#"
    let x: Num = "hello";
    "#;
    let analysis = analyze(source);
    
    assert_eq!(analysis.diagnostics.len(), 1);
    let (_, msg) = &analysis.diagnostics[0];
    assert!(msg.contains("Type mismatch"));
    assert!(msg.contains("expected Num"));
    assert!(msg.contains("got Str"));
}

#[test]
fn test_type_check_return() {
    let source = r#"
    let f = turn() -> Num {
        return "not a number";
    };
    "#;
    let analysis = analyze(source);
    
    assert_eq!(analysis.diagnostics.len(), 1);
    let (_, msg) = &analysis.diagnostics[0];
    assert!(msg.contains("Type mismatch"));
    assert!(msg.contains("expected Num"));
    assert!(msg.contains("got Str"));
}

#[test]
fn test_type_inference_propagation() {
    let source = r#"
    let x = 10;
    let y: Str = x;
    "#;
    let analysis = analyze(source);
    
    assert_eq!(analysis.diagnostics.len(), 1);
    let (_, msg) = &analysis.diagnostics[0];
    assert!(msg.contains("Type mismatch"));
    assert!(msg.contains("expected Str"));
    assert!(msg.contains("got Num"));
}
