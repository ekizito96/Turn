use turn::{run_with_tools, tools, Value};

#[test]
fn test_json_parse() {
    let source = r#"
    turn {
        let s = "{\"a\": 1, \"b\": [2, 3]}";
        let x = call("__sys_json_parse", s);
        return x;
    }
    "#;

    let tools = tools::ToolRegistry::new();
    let result = run_with_tools(source, &tools).unwrap();

    if let Value::Map(m) = result {
        assert_eq!(m.get("a"), Some(&Value::Num(1.0)));
        if let Some(Value::List(l)) = m.get("b") {
            assert_eq!(l.len(), 2);
            assert_eq!(l[0], Value::Num(2.0));
        } else {
            panic!("Expected list for b");
        }
    } else {
        panic!("Expected map, got {:?}", result);
    }
}

#[test]
fn test_json_stringify() {
    let source = r#"
    turn {
        let x = { "a": 1 };
        let s = call("__sys_json_stringify", x);
        return s;
    }
    "#;

    let tools = tools::ToolRegistry::new();
    let result = run_with_tools(source, &tools).unwrap();

    if let Value::Str(s) = result {
        // JSON stringify order is not guaranteed unless we use preserve_order feature or check contains
        assert!(s.contains("\"a\":1") || s.contains("\"a\": 1"));
    } else {
        panic!("Expected string, got {:?}", result);
    }
}
