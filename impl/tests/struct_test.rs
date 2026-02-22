use turn::value::Value;

#[tokio::test]
async fn test_struct_creation() {
    let source = "struct Point { x: Num, y: Num }; let p = Point { x: 10, y: 20 }; return p;";
    let val = turn::run(source).unwrap();
    if let Value::Struct(name, _fields) = val {
        assert_eq!(name.as_str(), "Point");
    }
}
