use turn::compiler::Compiler;
use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::value::Value;
use turn::vm::{Vm, VmEvent};

// ============================================================================
// Turn Sandboxing & Inference Tests (Phase 3)
//
// These tests verify that the Turn VM strictly enforces API boundaries,
// handles confidence threshold routing deterministically, and cleanly aborts
// inferences that exceed specified budgets without panicking.
// ============================================================================

#[tokio::test]
async fn test_confidence_routing_rejects_low_score() {
    // We mock the LLM response by intercepting the VmEvent::Suspend for llm_infer
    // and injecting a Value::Uncertain with a deliberately low confidence score (0.4).
    // The `~ 0.9` syntax means the VM must route execution to the `else` block.
    let script = r#"
        struct Output {
            decision: Str
        };

        let result = infer Output {
            "Make a high-stakes decision.";
        } ~ 0.9 else {
            return "fallback";
        };

        return result.decision;
    "#;

    let tokens = Lexer::new(script).tokenize().unwrap();
    let prog = Parser::new(tokens).parse().unwrap();
    let mut comp = Compiler::new();
    let code = comp.compile(&prog);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let vm = Vm::new(&code, tx);
    vm.start().await;

    // We expect the Vm to suspend for the `infer` block (which emits llm_infer)
    if let Some(VmEvent::Suspend { tool_name, resume_tx, .. }) = rx.recv().await {
        assert_eq!(tool_name, "llm_infer");

        // Create a Turn struct to represent the LLM's response
        let mut map = indexmap::IndexMap::new();
        map.insert("decision".to_string(), Value::Str(std::sync::Arc::new("LLM says yes".to_string())));
        let struct_val = Value::Struct(
            std::sync::Arc::new("Output".to_string()),
            std::sync::Arc::new(map),
        );

        // Inject a simulated low-confidence LLM response (0.4 < 0.9 threshold)
        let _ = resume_tx.send(Value::Uncertain(Box::new(struct_val), 0.4));
    }

    // Now await the final VM completion — should be the fallback value
    let mut got_result = false;
    while let Some(event) = rx.recv().await {
        match event {
            VmEvent::Complete { result, .. } => {
                assert_eq!(result.to_string(), "fallback");
                got_result = true;
                break;
            }
            VmEvent::Error { error, .. } => {
                // If the VM errors because the fallback block returns,
                // the `return` inside the else block may cause the process
                // to complete with that value.
                panic!("VM errored unexpectedly: {}", error);
            }
            _ => continue,
        }
    }
    assert!(got_result, "VM failed to complete with fallback result");
}

#[tokio::test]
async fn test_confidence_routing_accepts_high_score() {
    // Complementary test: inject a high confidence score (0.95 > 0.9)
    // The VM should accept the result and NOT jump to the fallback.
    let script = r#"
        struct Output {
            decision: Str
        };

        let result = infer Output {
            "Make a high-stakes decision.";
        } ~ 0.9 else {
            return "fallback";
        };

        return result.decision;
    "#;

    let tokens = Lexer::new(script).tokenize().unwrap();
    let prog = Parser::new(tokens).parse().unwrap();
    let mut comp = Compiler::new();
    let code = comp.compile(&prog);

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let vm = Vm::new(&code, tx);
    vm.start().await;

    if let Some(VmEvent::Suspend { tool_name, resume_tx, .. }) = rx.recv().await {
        assert_eq!(tool_name, "llm_infer");

        let mut map = indexmap::IndexMap::new();
        map.insert("decision".to_string(), Value::Str(std::sync::Arc::new("LLM says yes".to_string())));
        let struct_val = Value::Struct(
            std::sync::Arc::new("Output".to_string()),
            std::sync::Arc::new(map),
        );

        // High confidence: 0.95 > 0.9 threshold → accept
        let _ = resume_tx.send(Value::Uncertain(Box::new(struct_val), 0.95));
    }

    let mut got_result = false;
    while let Some(event) = rx.recv().await {
        match event {
            VmEvent::Complete { result, .. } => {
                assert_eq!(result.to_string(), "LLM says yes");
                got_result = true;
                break;
            }
            VmEvent::Error { error, .. } => {
                panic!("VM errored unexpectedly: {}", error);
            }
            _ => continue,
        }
    }
    assert!(got_result, "VM failed to complete with LLM result");
}

#[tokio::test]
async fn test_budget_exhaustion_aborts_cleanly() {
    // Set a budget of 10 tokens. The generated AST should trigger a runtime
    // TokenLimitExceeded error. Since the `infer` block is inside `with budget(...)`,
    // the VM should abort cleanly rather than crash.
    let script = r#"
        struct Output { text: Str };

        with budget(tokens: 10, time: 1000) {
            #[mock(infer = "{\"text\": \"This response is much longer than ten tokens and will surely exceed the strict budget limit applied to the inference layer.\"}", tokens_used = 100)]
            let result = infer Output { "Generate a long essay." };
            return result.text;
        };
    "#;

    let result = turn::run(script);

    // We expect the result to be an Err because the tokens_used (100) exceeds budget (10)
    // and there is no fallback provided in this specific block structure.
    assert!(result.is_err(), "Budget exhaustion failed to abort the inference: {:?}", result);
}
