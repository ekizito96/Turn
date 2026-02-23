use turn::value::Value;
use turn::tools::ToolRegistry;
use turn::run_with_tools;
use std::sync::Arc;

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
                    Value::Str(s) if s.contains("Str") => {
                        Ok(Value::Uncertain(Box::new(Value::Str(Arc::new("Mock Response".to_string()))), 0.7))
                    }
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

    if let Value::Uncertain(inner, p) = result {
        assert_eq!(*inner, Value::Num(42.0));
        assert!(p > 0.8);
    } else {
        panic!("Expected Uncertain(Num(42.0)), got {:?}", result);
    }
}

#[tokio::test]
async fn test_infer_bool_mock() {
    let source = r#"
    let x = infer Bool { "Is water wet?"; };
    return x;
    "#;

    let tools = get_mock_tools();
    let result = run_with_tools(source, &tools).expect("Run failed");

    if let Value::Uncertain(inner, p) = result {
        assert_eq!(*inner, Value::Bool(true));
        assert!(p > 0.8);
    } else {
        panic!("Expected Uncertain(Bool(true)), got {:?}", result);
    }
}
