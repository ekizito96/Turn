use turn::run;

// ============================================================================
// Turn End-to-End Tests (Phase 4)
//
// These tests exercise Turn's core language features in integration,
// verifying the full pipeline from lexer → parser → compiler → VM → result.
// ============================================================================

// ---------------------------------------------------------------------------
// 1. Struct definition + field access
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_struct_field_access() {
    let script = r#"
        struct User {
            name: Str,
            age: Num
        };
        let u = User { name: "Alice", age: 30 };
        return u.name;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "Alice");
}

// ---------------------------------------------------------------------------
// 2. Closure (tool) definition and invocation via call()
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_closure_call() {
    let script = r#"
        let double = turn(x: Num) -> Num {
            return x * 2;
        };
        let result = call(double, 21);
        return result;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "42");
}

// ---------------------------------------------------------------------------
// 3. List operations
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_list_operations() {
    let script = r#"
        let items = [10, 20, 30];
        let first = items[0];
        let last = items[2];
        return first + last;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "40");
}

// ---------------------------------------------------------------------------
// 4. Map creation and indexing
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_map_operations() {
    let script = r#"
        struct Config {
            host: Str,
            port: Num
        };
        let config = Config { host: "localhost", port: 8080 };
        return config.host;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "localhost");
}

// ---------------------------------------------------------------------------
// 5. If/else control flow
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_if_else_control_flow() {
    let script = r#"
        let x = 10;
        if x > 5 {
            return "big";
        } else {
            return "small";
        }
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "big");
}

// ---------------------------------------------------------------------------
// 6. While loop
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_while_loop() {
    let script = r#"
        let sum = 0;
        let i = 1;
        while i <= 10 {
            let sum = sum + i;
            let i = i + 1;
        }
        return sum;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "55");
}

// ---------------------------------------------------------------------------
// 7. String concatenation
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_string_concatenation() {
    let script = r#"
        let greeting = "Hello" + ", " + "World!";
        return greeting;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "Hello, World!");
}

// ---------------------------------------------------------------------------
// 8. Nested struct construction
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_nested_struct() {
    let script = r#"
        struct Address {
            city: Str
        };
        struct Person {
            name: Str,
            addr: Address
        };
        let a = Address { city: "Nairobi" };
        let p = Person { name: "Muyu", addr: a };
        return p.addr.city;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "Nairobi");
}

// ---------------------------------------------------------------------------
// 9. Boolean logic
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_boolean_logic() {
    let script = r#"
        let a = true;
        let b = false;
        if a and !b {
            return "correct";
        }
        return "wrong";
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "correct");
}

// ---------------------------------------------------------------------------
// 10. Error handling: try/catch
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_error_result_matching() {
    let script = r#"
        let result = err("expected error");
        match result {
            ok(v) -> { return "uncaught"; }
            err(e) -> { return "caught: " + e; }
        }
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "caught: expected error");
}

// ---------------------------------------------------------------------------
// 11. Remember / Recall (stateful memory)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_remember_recall() {
    let script = r#"
        remember("key", "value123");
        let v = recall("key");
        return v;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "value123");
}

// ---------------------------------------------------------------------------
// 12. Higher-order functions (closures as arguments)
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_higher_order_functions() {
    let script = r#"
        let square = turn(n: Num) -> Num {
            return n * n;
        };
        let result = call(square, 7);
        return result;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "49");
}

// ---------------------------------------------------------------------------
// 13. Arithmetic operations
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_arithmetic() {
    let script = r#"
        let a = 10 + 5;
        let b = a * 2;
        let c = b - 3;
        let d = c / 9;
        return d;
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "3");
}

// ---------------------------------------------------------------------------
// 14. Comparison operators
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_comparisons() {
    let script = r#"
        if 10 >= 10 and 5 < 6 and 3 != 4 and 7 == 7 {
            return "all_pass";
        }
        return "fail";
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "all_pass");
}

// ---------------------------------------------------------------------------
// 15. Context append
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_context_append() {
    let script = r#"
        context.append("System message 1");
        context.append("System message 2");
        return "ok";
    "#;
    let result = run(script).unwrap();
    assert_eq!(result.to_string(), "ok");
}
