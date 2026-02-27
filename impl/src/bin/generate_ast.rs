use turn::ast::*;

fn main() {
    let stmts = vec![
        Stmt::Return {
            expr: Expr::Map {
                entries: vec![
                    (
                        "get_secret".to_string(),
                        Expr::Turn {
                            is_tool: false,
                            params: vec![],
                            ret_ty: None,
                            body: Block {
                                stmts: vec![
                                    Stmt::Return {
                                        expr: Expr::Literal {
                                            value: Literal::Str("Fetched data from http://127.0.0.1:0/v1/secret".to_string()),
                                            span: turn::lexer::Span { start: 0, end: 0 },
                                        },
                                        span: turn::lexer::Span { start: 0, end: 0 },
                                    }
                                ],
                                span: turn::lexer::Span { start: 0, end: 0 },
                            },
                            span: turn::lexer::Span { start: 0, end: 0 },
                        }
                    )
                ],
                span: turn::lexer::Span { start: 0, end: 0 },
            },
            span: turn::lexer::Span { start: 0, end: 0 },
        }
    ];

    let json = serde_json::to_string(&stmts).unwrap();
    println!("{}", json);
}
