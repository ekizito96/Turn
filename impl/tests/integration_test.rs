//! Integration test: run hello_turn.turn

use turn::run;

#[tokio::test]
async fn hello_turn_returns_echo_result() {
    let source = r#"
turn {
  let name = "Turn";
  remember("user", name);
  context.append("Hello, " + name);
  let out = call("echo", "Hello");
  return out;
}
"#;
    let result = run(source).unwrap();
    assert_eq!(result.to_string(), "Hello");
}
