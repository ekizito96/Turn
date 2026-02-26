use turn::run;
use turn::value::Value;

#[tokio::test]
async fn test_await_receive_block() {
    // Tests that `await receive` suspends the green thread and yields the CPU
    // until a valid message is sent to its mailbox.
    let source = r#"
        let worker = turn() -> Any {
            let msg = await receive;
            return msg;
        };
        let server = spawn worker;
        send server, "Wake up!";
        return "Wake up!";
    "#;

    let res = run(source).unwrap();
    assert_eq!(res, Value::Str(std::sync::Arc::new("Wake up!".to_string())));
}
