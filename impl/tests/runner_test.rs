use turn::value::Value;

#[test]
fn test_run_helper_infer_mock() {
    let source = r#"
    let x = infer Num { "What is 2+2?"; };
    return x;
    "#;
    
    // turn::run uses default ToolRegistry which includes llm_infer mock
    let result = turn::run(source).expect("Run failed");
    
    if let Value::Uncertain(inner, p) = result {
        // Mock returns 42.0 for "Num" schema with 0.85 conf
        assert_eq!(*inner, Value::Num(42.0));
        assert!(p > 0.8);
    } else {
        panic!("Expected Uncertain(Num(42.0)), got {:?}", result);
    }
}

#[test]
fn test_infer_bool_mock() {
    let source = r#"
    let x = infer Bool { "Is water wet?"; };
    return x;
    "#;
    
    let result = turn::run(source).expect("Run failed");
    
    if let Value::Uncertain(inner, p) = result {
        // Mock returns true for "Bool"
        assert_eq!(*inner, Value::Bool(true));
        assert!(p > 0.8);
    } else {
        panic!("Expected Uncertain(Bool(true)), got {:?}", result);
    }
}
