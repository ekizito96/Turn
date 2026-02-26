use turn::value::Value;
use turn::store::FileStore;
use turn::tools::ToolRegistry;
use turn::runner::Runner;

#[tokio::test]
async fn test_typed_hitl_suspension_cycle() {
    let source = r#"
    let human_feedback = suspend for Str "Please review PR #42";
    return human_feedback;
    "#;

    let store = FileStore::new(".turn_test_store");
    let tools = ToolRegistry::new();
    let mut runner = Runner::new(store, tools);

    // 1. Initial run should halt completely at `suspend` and return Ok(Value::Null)
    let initial_res = runner.run("test_process", source, None).await.unwrap();
    assert_eq!(initial_res, Value::Null);

    // 2. Validate strict payload rejection from the host
    let bad_json = serde_json::json!({
        "status": "APPROVED"
    }); // This is a Map, but the AST strictly expects a Str!
    
    let resume_attempt: Result<Value, anyhow::Error> = runner.resume("test_process", bad_json).await;
    assert!(resume_attempt.is_err(), "VM must strictly reject malformed JSON");

    // 3. Inject computationally sound struct and resume
    let valid_json = serde_json::json!("APPROVED_BY_HUMAN");

    let final_res = runner.resume("test_process", valid_json).await.unwrap();
    assert_eq!(final_res, Value::Str(std::sync::Arc::new("APPROVED_BY_HUMAN".to_string())));
}
