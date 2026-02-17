use turn::{Vm, VmResult, compiler, parser, lexer, tools, Value};

#[test]
fn test_list_creation() {
    let source = r#"
    turn {
        let x = [1, 2, 3];
        return x;
    }
    "#;
    
    let tokens = lexer::Lexer::new(source).tokenize().unwrap();
    let program = parser::Parser::new(tokens).parse().unwrap();
    let mut compiler = compiler::Compiler::new();
    let code = compiler.compile(&program);
    let tools = tools::ToolRegistry::new();
    let mut vm = Vm::new(&code);
    
    match vm.run() {
        VmResult::Complete(val) => {
            if let Value::List(l) = val {
                assert_eq!(l.len(), 3);
                assert_eq!(l[0], Value::Num(1.0));
                assert_eq!(l[1], Value::Num(2.0));
                assert_eq!(l[2], Value::Num(3.0));
            } else {
                panic!("Expected list, got {:?}", val);
            }
        }
        _ => panic!("Expected completion"),
    }
}

#[test]
fn test_map_creation() {
    let source = r#"
    turn {
        let x = { "a": 1, "b": 2 };
        return x;
    }
    "#;
    
    let tokens = lexer::Lexer::new(source).tokenize().unwrap();
    let program = parser::Parser::new(tokens).parse().unwrap();
    let mut compiler = compiler::Compiler::new();
    let code = compiler.compile(&program);
    let tools = tools::ToolRegistry::new();
    let mut vm = Vm::new(&code);
    
    match vm.run() {
        VmResult::Complete(val) => {
            if let Value::Map(m) = val {
                assert_eq!(m.len(), 2);
                assert_eq!(m.get("a"), Some(&Value::Num(1.0)));
                assert_eq!(m.get("b"), Some(&Value::Num(2.0)));
            } else {
                panic!("Expected map, got {:?}", val);
            }
        }
        _ => panic!("Expected completion"),
    }
}

#[test]
fn test_nested_structures() {
    let source = r#"
    turn {
        let x = { "list": [1, 2], "map": { "a": 3 } };
        return x;
    }
    "#;
    
    let tokens = lexer::Lexer::new(source).tokenize().unwrap();
    let program = parser::Parser::new(tokens).parse().unwrap();
    let mut compiler = compiler::Compiler::new();
    let code = compiler.compile(&program);
    let tools = tools::ToolRegistry::new();
    let mut vm = Vm::new(&code);
    
    match vm.run() {
        VmResult::Complete(val) => {
            if let Value::Map(m) = val {
                if let Some(Value::List(l)) = m.get("list") {
                    assert_eq!(l.len(), 2);
                    assert_eq!(l[0], Value::Num(1.0));
                } else {
                    panic!("Expected list in map");
                }
                
                if let Some(Value::Map(inner)) = m.get("map") {
                    assert_eq!(inner.get("a"), Some(&Value::Num(3.0)));
                } else {
                    panic!("Expected inner map");
                }
            } else {
                panic!("Expected map, got {:?}", val);
            }
        }
        _ => panic!("Expected completion"),
    }
}
