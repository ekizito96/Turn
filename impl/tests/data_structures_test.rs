use turn::value::Value;

#[tokio::test]
async fn test_map_creation() {
    let source = "let m = #{ \"a\" => 1, \"b\" => 2 }; return m;";
    let val = turn::run(source).unwrap();
    if let Value::Map(m) = val {
        assert_eq!(m.get("a"), Some(&Value::Num(1.0)));
    }
}
