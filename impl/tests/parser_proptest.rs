use proptest::prelude::*;
use turn::lexer::Lexer;
use turn::parser::{Parser, ParseError};

// We want to ensure that NO sequence of characters ever causes the lexer or parser
// to panic (unwind). They should always return a generic Ok(AST) or an Err. 
// This is the core property of a robust parser in Rust.

proptest! {
    // Proptest will generate 10,000 random strings of varying lengths
    #[test]
    fn parser_never_panics_on_garbage(ref input in "\\PC*") {
        // Step 1: Lex the input. If it panics, the test fails.
        let mut lexer = Lexer::new(&input);
        match lexer.tokenize() {
            Ok(tokens) => {
                // Step 2: If lexing succeeds, attempt to parse. If it panics, the test fails.
                let mut parser = Parser::new(tokens);
                let _ = parser.parse(); 
                
                // Result does not matter for this property test. Outputting an AST or returning
                // ParseError::UnexpectedToken is perfectly acceptable. A `panic!` is the only failure.
            }
            Err(_) => {
                // Lexer correctly identified invalid tokens and gracefully errored. Success.
            }
        }
    }

    #[test]
    fn parser_never_panics_on_malformed_keywords(
        ref ws in r"[ \t\n\r]*",
        ref keyword in "(turn|tool|struct|infer|spawn|harvest|suspend|send|receive|persist|context|remember|recall|match|with|budget)",
        ref garbage in "\\PC*"
    ) {
        let input = format!("{}{}{}", ws, keyword, garbage);
        let mut lexer = Lexer::new(&input);
        if let Ok(tokens) = lexer.tokenize() {
            let mut parser = Parser::new(tokens);
            let _ = parser.parse(); 
        }
    }
}
