use std::sync::Arc;
use turn::run_with_tools;
use turn::tools::ToolRegistry;
use turn::value::Value;

fn get_mock_tools() -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(
        "llm_infer",
        Box::new(|arg| {
            if let Value::Map(m) = arg {
                let schema = m.get("schema").unwrap_or(&Value::Null);
                match schema {
                    Value::Str(s) if s.contains("Num") => {
                        Ok(Value::Uncertain(Box::new(Value::Num(42.0)), 0.85))
                    }
                    Value::Str(s) if s.contains("Bool") => {
                        Ok(Value::Uncertain(Box::new(Value::Bool(true)), 0.9))
                    }
                    Value::Str(s) if s.contains("Str") => Ok(Value::Uncertain(
                        Box::new(Value::Str(Arc::new("Mock Response".to_string()))),
                        0.7,
                    )),
                    _ => Ok(Value::Uncertain(Box::new(Value::Null), 0.5)),
                }
            } else {
                Err("Invalid args for llm_infer".to_string())
            }
        }),
    );
    tools
}

#[tokio::test]
async fn test_run_helper_infer_mock() {
    let source = r#"
    let x = infer Num { "What is 2+2?"; };
    return x;
    "#;

    let tools = get_mock_tools();
    let result = run_with_tools(source, &tools).expect("Run failed");

    // VM coerces Uncertain(Num(42.0)) to Num(42.0) at the assignment boundary.
    assert_eq!(result, Value::Num(42.0), "Expected Num(42.0), got {:?}", result);
}

#[tokio::test]
async fn test_infer_bool_mock() {
    let source = r#"
    let x = infer Bool { "Is water wet?"; };
    return x;
    "#;

    let tools = get_mock_tools();
    let result = run_with_tools(source, &tools).expect("Run failed");

    // VM coerces Uncertain(Bool(true)) to Bool(true) at the assignment boundary.
    assert_eq!(result, Value::Bool(true), "Expected Bool(true), got {:?}", result);
}
