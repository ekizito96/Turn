use turn::{value::Value, run};

fn run_turn_code(source: &str) -> Value {
    run(source).expect("Run failed")
}

#[test]
fn test_struct_spread_syntax() {
    let source = r#"
    struct User { name: Str, age: Num, city: Str };
    let base = User { name: "Alice", age: 30, city: "London" };
    let updated = User { age: 31, city: "Paris", ..base };
    return updated;
    "#;
    
    let result = run_turn_code(source);
    if let Value::Struct(name, fields) = result {
        assert_eq!(name, "User");
        assert_eq!(fields.get("name"), Some(&Value::Str("Alice".to_string())));
        assert_eq!(fields.get("age"), Some(&Value::Num(31.0)));
        assert_eq!(fields.get("city"), Some(&Value::Str("Paris".to_string())));
    } else {
        panic!("Expected Struct, got {:?}", result);
    }
}

#[test]
fn test_context_inject_primitive() {
    let source = r#"
    context.append("This is important");
    return 1;
    "#;
    
    let result = run_turn_code(source);
    assert_eq!(result, Value::Num(1.0));
}
