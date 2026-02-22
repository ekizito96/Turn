use turn::value::Value;

#[tokio::test]
async fn test_manual_suspension_resume_cycle() {
    let source = r#"
    tool fn my_tool(ping: Str) -> Str {
        return "pong";
    }
    turn {
        let x = my_tool("ping");
        return x;
    }
    "#;
    let res = turn::run(source).unwrap();
    assert_eq!(res, Value::Str(std::sync::Arc::new("pong".to_string())));
}
