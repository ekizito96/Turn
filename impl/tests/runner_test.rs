use std::sync::Mutex;
use tempfile::tempdir;
use turn::value::Value;
use turn::{FileStore, Runner, ToolRegistry};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_run_helper_infer_mock() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("TURN_LLM_PROVIDER", "mock");
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
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("TURN_LLM_PROVIDER", "mock");
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

#[test]
fn test_run_helper_decide_mock() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("TURN_DECISION_PROVIDER", "mock");
    let source = r#"
    let result = decide("A customer needs help", {
        "department": {
            "type": "choice",
            "instructions": "Who should handle this?",
            "criteria": {
                "support": "Existing customer help",
                "sales": "New customer questions"
            }
        }
    });
    return result["answers"]["department"]["choice"];
    "#;

    let result = turn::run(source).expect("Run failed");
    assert_eq!(result, Value::Str("sales".to_string()));
}

#[test]
fn test_typesafe_decide_requires_api_key() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("TURN_DECISION_PROVIDER", "typesafe");
    std::env::remove_var("TYPESAFE_API_KEY");
    let source = r#"
    return decide("A customer needs help", {
        "urgent": {
            "type": "noul",
            "instructions": "Is this urgent?"
        }
    });
    "#;

    let error = turn::run(source).expect_err("Missing API key should fail");
    assert!(error.to_string().contains("TYPESAFE_API_KEY is required"));
}

#[test]
fn test_playground_registry_allows_decisions() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    std::env::set_var("TURN_DECISION_PROVIDER", "mock");
    let directory = tempdir().unwrap();
    let store = FileStore::new(directory.path());
    let mut runner = Runner::new(store, ToolRegistry::playground()).sandboxed();
    let source = r#"
    let result = decide("A customer needs help", {
        "urgent": { "type": "noul", "instructions": "Is this urgent?" }
    });
    return result["answers"]["urgent"]["noul"];
    "#;

    assert_eq!(
        runner.run("decision", source, None).unwrap(),
        Value::Num(0.5)
    );
}

#[test]
fn test_playground_registry_denies_host_access() {
    let directory = tempdir().unwrap();
    let store = FileStore::new(directory.path());
    let mut runner = Runner::new(store, ToolRegistry::playground()).sandboxed();

    let error = runner
        .run("import", "let fs = use \"std/fs\"; return fs;", None)
        .expect_err("Sandbox should deny imports");
    assert!(error
        .to_string()
        .contains("Sandbox denied effect: sys_import"));
}
