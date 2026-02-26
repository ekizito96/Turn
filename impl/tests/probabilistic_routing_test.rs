use turn::run_with_tools;
use turn::tools::ToolRegistry;
use turn::value::Value;

fn get_mock_tools() -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(
        "llm_infer",
        Box::new(|_| {
            // Simulated LLM returning a low confidence value (0.5 < 0.8 threshold)
            Ok(Value::Uncertain(
                Box::new(Value::Map(std::sync::Arc::new(indexmap::IndexMap::new()))),
                0.5,
            ))
        }),
    );
    tools
}

#[tokio::test]
async fn test_probabilistic_routing_fallback() {
    let src = r#"
        let point = infer Map<Str, Num> { "Test prompt"; } ~ 0.8 else {
            return { "x": 0.0, "y": 0.0 };
        };
        return point;
    "#;

    let tools = get_mock_tools();
    let res = run_with_tools(src, &tools).unwrap();

    let point = match res {
        Value::Map(m) => m,
        _ => panic!("Expected Map result, got {:?}", res),
    };

    let px = point.get("x").unwrap();
    let py = point.get("y").unwrap();

    assert!(matches!(px, Value::Num(0.0)));
    assert!(matches!(py, Value::Num(0.0)));
}
