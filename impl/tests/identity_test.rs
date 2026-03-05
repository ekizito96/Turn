use turn::value::Value;

#[test]
fn test_identity_capability_primitive() {
    // Configure the real token via the env var convention.
    // TURN_IDENTITY_<PROVIDER_UPPERCASED>_TOKEN
    std::env::set_var(
        "TURN_IDENTITY_GOOGLE_WORKSPACE_TOKEN",
        "test_bearer_token_12345",
    );

    let source = r#"
    // The agent requests a secure identity capability.
    // The raw token never touches Turn memory.
    let my_google_auth = grant identity::oauth("google_workspace");

    // Pass the Identity capability into the HTTP tool.
    // The Turn VM intercepts this, looks up TURN_IDENTITY_GOOGLE_WORKSPACE_TOKEN 
    // from the host environment, and injects it as a Bearer header.
    let result = call("http_get", {
        "url": "https://httpbin.org/bearer",
        "identity": my_google_auth
    });

    return result;
    "#;

    let result = turn::run(source).expect("Run failed");

    // httpbin.org/bearer returns {"authenticated": true, "token": "..."}
    // when a valid Bearer token is sent.
    if let Value::Str(s) = result {
        println!("HTTP Response: {}", s);
        assert!(s.contains("authenticated"));
    } else {
        panic!("Expected HTTP response string, got {:?}", result);
    }
}
