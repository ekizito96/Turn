use turn::{Expr, Lexer, Parser, Stmt, Type};

#[tokio::test]
async fn test_parse_typed_let() {
    let src = "let x: Num = 10;";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();

    match &program.stmts[0] {
        Stmt::Let { name, ty, .. } => {
            assert_eq!(name, "x");
            assert_eq!(ty, &Some(Type::Num));
        }
        _ => panic!("Expected Let"),
    }
}

#[tokio::test]
async fn test_parse_typed_function() {
    let src = "let add = turn(a: Num, b: Num) -> Num { return a + b; };";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();

    match &program.stmts[0] {
        Stmt::Let { init, .. } => match init {
            Expr::Turn { params, ret_ty, .. } => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].0, "a");
                assert_eq!(params[0].2, Some(Type::Num));
                assert_eq!(params[1].0, "b");
                assert_eq!(params[1].2, Some(Type::Num));
                assert_eq!(ret_ty, &Some(Type::Num));
            }
            _ => panic!("Expected Turn expr"),
        },
        _ => panic!("Expected Let"),
    }
}
