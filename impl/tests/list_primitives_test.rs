use turn::{value::Value, run};

fn run_turn_code(source: &str) -> Value {
    run(source).expect("Run failed")
}

#[test]
fn test_n_ary_function_arguments() {
    let source = r#"
    let add_three = turn(a: Num, b: Num, c: Num) -> Num {
        return a + b + c;
    };
    return call(add_three, 1, 2, 3);
    "#;
    
    let result = run_turn_code(source);
    assert_eq!(result, Value::Num(6.0));
}

#[test]
fn test_list_push_and_len() {
    let source = r#"
    let list = [1, 2, 3];
    let new_list = call("list_push", list, 4);
    let l = call("len", new_list);
    return l;
    "#;
    
    let result = run_turn_code(source);
    assert_eq!(result, Value::Num(4.0));
}

#[test]
fn test_list_contains() {
    let source = r#"
    let list = ["apple", "banana", "cherry"];
    let has_banana = call("list_contains", list, "banana");
    let has_grape = call("list_contains", list, "grape");
    return {"has_banana": has_banana, "has_grape": has_grape};
    "#;
    
    let result = run_turn_code(source);
    if let Value::Map(m) = result {
        assert_eq!(m.get("has_banana"), Some(&Value::Bool(true)));
        assert_eq!(m.get("has_grape"), Some(&Value::Bool(false)));
    } else {
        panic!("Expected map");
    }
}

#[test]
fn test_list_map_placeholder() {
    let source = r#"
    let list = [1, 2, 3];
    let map_fn = turn(item: Any) -> Any { return item; };
    return call("list_map", list, map_fn);
    "#;
    
    let result = run_turn_code(source);
    if let Value::List(l) = result {
        assert_eq!(l.len(), 3);
        assert_eq!(l[0], Value::Num(1.0));
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_list_filter_placeholder() {
    let source = r#"
    let list = [1, 2, 3];
    let filter_fn = turn(item: Any) -> Bool { return true; };
    return call("list_filter", list, filter_fn);
    "#;
    
    let result = run_turn_code(source);
    if let Value::List(l) = result {
        assert_eq!(l.len(), 3);
        assert_eq!(l[0], Value::Num(1.0));
    } else {
        panic!("Expected list");
    }
}
