use turn::lexer::Lexer;

fn main() {
    let source = "let search_docs = tool(query: Str) -> Str { };";
    let mut lexer = Lexer::new(source);
    while let Ok(t) = lexer.next_token() {
        if t.token == turn::lexer::Token::Eof {
            break;
        }
        println!("{:?} {:?}", t.token, t.span);
    }
}
