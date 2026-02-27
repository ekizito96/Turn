use turn::compiler::Compiler;
use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::value::Value;
use turn::vm::{Vm, VmEvent};

#[tokio::test]
async fn test_stochastic_mock() {
    let src = r#"
struct TargetStruct { status: Str, score: Num };

#[mock(infer TargetStruct = TargetStruct { status: "MOCKED", score: 99.9 })]
test mock_bypass {
    let out = infer TargetStruct { "This is a prompt"; };
    return out;
}
"#;

    let tokens = Lexer::new(src).tokenize().unwrap();
    let prog = Parser::new(tokens).parse().unwrap();
    let mut comp = Compiler::new();
    let code = comp.compile(&prog);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let vm = Vm::new(&code, tx);
    vm.start().await;

    // We expect the VmEvent::Complete to yield the struct.
    // If the mock bypass didn't work, we would get VmEvent::Suspend asking for an LLM response.
    if let Some(event) = rx.recv().await {
        match event {
            VmEvent::Complete { result, .. } => {
                if let Value::Struct(name, fields) = result {
                    assert_eq!(name.as_str(), "TargetStruct");
                    assert_eq!(
                        fields.get("status").unwrap(),
                        &Value::Str(std::sync::Arc::new("MOCKED".to_string()))
                    );
                    assert_eq!(fields.get("score").unwrap(), &Value::Num(99.9));
                } else {
                    panic!("Expected Struct result, got {:?}", result);
                }
            }
            _ => panic!("Expected Complete event, got {:?}", event),
        }
    } else {
        panic!("VM finished without emitting event");
    }
}
