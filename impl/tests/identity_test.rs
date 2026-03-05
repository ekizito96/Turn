use turn::value::Value;

#[test]
fn test_identity_capability_primitive() {
    let source = r#"
    // 1. The agent requests a secure identity capability.
    // The Turn runtime securely intercepts this via sys_grant and 
    // returns an opaque handle without exposing root secrets.
    let my_google_auth = grant identity::oauth("google_workspace");

    // 2. We can pass it to the native HTTP tools. The tools internally
    // inject the Authorization header, meaning the LLM NEVER touches the string token.
    let result = call("http_get", {
        "url": "https://httpbin.org/bearer",
        "identity": my_google_auth
    });

    return result;
    "#;

    let result = turn::run(source).expect("Run failed");

    // The mock runner injects a token named `turn_mock_token_for_<provider>`
    if let Value::Str(s) = result {
        println!("HTTP Response: {}", s);
        assert!(s.contains("turn_mock_token_for_google_workspace"));
    } else {
        panic!("Expected HTTP response string, got {:?}", result);
    }
}
