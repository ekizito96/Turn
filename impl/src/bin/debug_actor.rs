use turn::lexer::Lexer;

fn main() {
    let source = r#"
        let worker = turn() -> Null {
            let msg = receive;
            print("Worker received: " + msg);
            return null;
        };
        let pid = spawn worker;
        print("Spawned worker with PID: " + pid);
        let success = send pid, "Hello!";
    "#;
    let mut lexer = Lexer::new(source);
    while let Ok(tok) = lexer.next_token() {
        println!("{:?}", tok);
    }
}
