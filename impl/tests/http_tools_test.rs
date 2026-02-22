use turn::{run_with_tools, tools, Value};

#[tokio::test]
async fn test_http_get() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("GET", "/hello")
        .with_status(200)
        .with_header("content-type", "text/plain")
        .with_body("world")
        .create();

    let url = format!("{}/hello", server.url());

    let source = format!(
        r#"
    turn {{
        let url = "{}";
        let res = call("http_get", url);
        return res;
    }}
    "#,
        url
    );

    let tools = tools::ToolRegistry::new();
    let result = run_with_tools(&source, &tools).unwrap();

    match result {
        Value::Str(s) => assert_eq!(s.as_str(), "world"),
        _ => panic!("Expected string 'world', got {:?}", result),
    }
}

#[tokio::test]
async fn test_http_post() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("POST", "/echo")
        .match_body(mockito::Matcher::JsonString(
            "{\"msg\":\"hello\"}".to_string(),
        ))
        .with_status(200)
        .with_body("ok")
        .create();

    let url = format!("{}/echo", server.url());

    let source = format!(
        r#"
    turn {{
        let payload = {{ "url": "{}", "body": {{ "msg": "hello" }} }};
        let res = call("http_post", payload);
        return res;
    }}
    "#,
        url
    );

    let tools = tools::ToolRegistry::new();
    let result = run_with_tools(&source, &tools).unwrap();

    match result {
        Value::Str(s) => assert_eq!(s.as_str(), "ok"),
        _ => panic!("Expected string 'ok', got {:?}", result),
    }
}

#[tokio::test]
async fn test_http_get_error() {
    let server = mockito::Server::new();
    // No mock setup, request should fail (404 on server or connection error if server dropped)
    // We keep server alive to get 501 or 404
    let url = format!("{}/missing", server.url());

    let source = format!(
        r#"
    turn {{
        let url = "{}";
        let res = call("http_get", url);
        return res;
    }}
    "#,
        url
    );

    let tools = tools::ToolRegistry::new();
    let result = run_with_tools(&source, &tools).unwrap();

    // Should return error Struct on error
    match result {
        Value::Struct(name, fields) => {
            assert_eq!(name.as_str(), "Result");
            if let Some(Value::Str(s)) = fields.get("err") {
                assert!(
                    s.contains("HTTP request failed")
                        || s.contains("HTTP request error")
                        || s.contains("501")
                );
            } else {
                panic!("Expected 'err' field with string");
            }
        }
        _ => panic!("Expected Result struct, got {:?}", result),
    }
}
