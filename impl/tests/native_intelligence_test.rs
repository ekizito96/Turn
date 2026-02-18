use turn::vm::{Vm, VmResult};
use turn::value::Value;
use turn::compiler::Compiler;
use turn::parser::Parser;
use turn::lexer::Lexer;

fn compile_and_start(source: &str) -> Vm {
    let lexer = Lexer::new(source);
    let tokens = lexer.tokenize().expect("Lexer failed");
    let mut parser = Parser::new(tokens);
    let program = parser.parse().expect("Parser failed");
    let mut compiler = Compiler::new();
    let code = compiler.compile(&program);
    Vm::new(&code)
}

#[test]
fn test_infer_suspension_flow() {
    let source = r#"
    let res = infer Num { "Calculate 2+2"; };
    return res;
    "#;
    
    let mut vm = compile_and_start(source);
    let result = vm.run();
    
    match result {
        VmResult::Suspended { tool_name, arg, continuation } => {
            assert_eq!(tool_name, "llm_infer");
            
            if let Value::Map(m) = arg {
                assert_eq!(m.get("prompt").unwrap(), &Value::Str("Calculate 2+2".to_string()));
                // Type debug format of Num is "Num"
                assert_eq!(m.get("schema").unwrap(), &Value::Str("Num".to_string()));
            } else {
                panic!("Expected Map arg, got {:?}", arg);
            }
            
            // Resume with Mock Result
            let mock_result = Value::Uncertain(Box::new(Value::Num(4.0)), 0.99);
            let mut vm = Vm::resume_with_result(continuation, mock_result);
            let final_res = vm.run();
            
            match final_res {
                VmResult::Complete(v) => {
                    if let Value::Uncertain(inner, p) = v {
                        assert_eq!(*inner, Value::Num(4.0));
                        assert_eq!(p, 0.99);
                    } else {
                        panic!("Expected Uncertain(Num(4)), got {:?}", v);
                    }
                }
                _ => panic!("Expected completion, got {:?}", final_res),
            }
        }
        _ => panic!("Expected suspension, got {:?}", result),
    }
}

#[test]
fn test_infer_with_dynamic_prompt() {
    let source = r#"
    let q = "capital of France";
    let res = infer Str { 
        let p = "What is the " + q;
        p; 
    };
    return res;
    "#;
    
    let mut vm = compile_and_start(source);
    let result = vm.run();
    
    match result {
        VmResult::Suspended { tool_name, arg, .. } => {
            assert_eq!(tool_name, "llm_infer");
            if let Value::Map(m) = arg {
                assert_eq!(m.get("prompt").unwrap(), &Value::Str("What is the capital of France".to_string()));
            }
        }
        _ => panic!("Expected suspension, got {:?}", result),
    }
}
