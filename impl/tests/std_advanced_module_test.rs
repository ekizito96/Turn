use turn::runner::Runner;
use turn::store::FileStore;
use turn::tools::ToolRegistry;
use turn::value::Value;

fn make_runner(suffix: &str) -> Runner<FileStore> {
    let store_dir = std::env::temp_dir().join(format!("turn_test_{}", suffix));
    let _ = std::fs::remove_dir_all(&store_dir);
    let store = FileStore::new(store_dir);
    let tools = ToolRegistry::new();
    Runner::new(store, tools)
}

#[tokio::test]
async fn test_std_json_module_parse_and_stringify() {
    let mut runner = make_runner("std_json");
    let source = r#"
    let json = use "std/json";
    let obj = json.parse("{\"name\":\"turn\"}");
    let name = obj["name"];
    let text = json.stringify({ "ok": true });
    return [name, text];
    "#;

    let result = runner
        .run("test_agent", source, None)
        .await
        .expect("Run failed");
    if let Value::List(items) = result {
        assert_eq!(
            items[0],
            Value::Str(std::sync::Arc::new("turn".to_string()))
        );
        match &items[1] {
            Value::Str(s) => {
                let parsed: serde_json::Value =
                    serde_json::from_str(s).expect("json.stringify should return valid JSON");
                assert_eq!(parsed.get("ok"), Some(&serde_json::Value::Bool(true)));
            }
            _ => panic!("Expected JSON string, got {:?}", items[1]),
        }
    } else {
        panic!("Expected list, got {:?}", result);
    }
}

#[tokio::test]
async fn test_std_time_module_now() {
    let mut runner = make_runner("std_time");
    let source = r#"
    let time = use "std/time";
    return time.now();
    "#;

    let result = runner
        .run("test_agent", source, None)
        .await
        .expect("Run failed");
    match result {
        Value::Num(n) => assert!(n > 0.0),
        _ => panic!("Expected Num timestamp, got {:?}", result),
    }
}

#[tokio::test]
async fn test_std_regex_module() {
    let mut runner = make_runner("std_regex");
    let source = r#"
    let re = use "std/regex";
    let is_match = re.matches("^turn$", "turn");
    let out = re.replace("turn", "hello turn world", "TURN");
    return [is_match, out];
    "#;

    let result = runner
        .run("test_agent", source, None)
        .await
        .expect("Run failed");
    if let Value::List(items) = result {
        assert_eq!(items[0], Value::Bool(true));
        assert_eq!(
            items[1],
            Value::Str(std::sync::Arc::new("hello TURN world".to_string()))
        );
    } else {
        panic!("Expected list, got {:?}", result);
    }
}
