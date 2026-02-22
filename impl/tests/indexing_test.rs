use turn::{run_with_tools, tools, Value};

#[tokio::test]
async fn test_list_indexing() {
    let source = r#"
    turn {
        let l = [10, 20, 30];
        let x = l[1];
        return x;
    }
    "#;

    let tools = tools::ToolRegistry::new();
    let result = run_with_tools(source, &tools).unwrap();

    match result {
        Value::Num(n) => assert_eq!(n, 20.0),
        _ => panic!("Expected number 20, got {:?}", result),
    }
}

#[tokio::test]
async fn test_map_indexing() {
    let source = r#"
    turn {
        let m = { "a": 1, "b": 2 };
        let x = m["b"];
        return x;
    }
    "#;

    let tools = tools::ToolRegistry::new();
    let result = run_with_tools(source, &tools).unwrap();

    match result {
        Value::Num(n) => assert_eq!(n, 2.0),
        _ => panic!("Expected number 2, got {:?}", result),
    }
}

#[tokio::test]
async fn test_nested_indexing() {
    let source = r#"
    turn {
        let data = { "users": [ { "name": "Alice" }, { "name": "Bob" } ] };
        let name = data["users"][1]["name"];
        return name;
    }
    "#;

    let tools = tools::ToolRegistry::new();
    let result = run_with_tools(source, &tools).unwrap();

    match result {
        Value::Str(s) => assert_eq!(s.as_str(), "Bob"),
        _ => panic!("Expected string 'Bob', got {:?}", result),
    }
}
