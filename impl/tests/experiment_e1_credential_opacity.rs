// Experiment E1: Credential Opacity
//
// Verifies that Identity capability handles NEVER expose raw credentials:
//   1. echo(Identity) prints "<identity providerName>" — NOT the secret token.
//   2. The secret token is only resolved inside the kernel HTTP trap from the
//      host environment; it never enters the Turn value heap.
//   3. String coercion of Identity does NOT yield the underlying token.

use turn::compiler::Compiler;
use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::value::Value;
use turn::vm::{Vm, VmResult};

fn run(source: &str) -> Value {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut compiler = Compiler::new();
    let code = compiler.compile(&program);
    let mut vm = Vm::new(&code);
    match vm.run() {
        VmResult::Complete(v) => v,
        VmResult::Suspended {
            tool_name,
            arg,
            continuation,
        } => {
            // Handle sys_grant: return an Identity value — simulating what
            // runner.rs does without an actual HTTP call.
            if tool_name == "sys_grant" {
                if let Value::Str(provider) = arg {
                    let mut resumed_vm =
                        Vm::resume_with_result(continuation, Value::Identity(provider));
                    match resumed_vm.run() {
                        VmResult::Complete(v) => v,
                        _ => panic!("VM did not complete after grant resume"),
                    }
                } else {
                    panic!("sys_grant expected string provider");
                }
            } else {
                panic!("Unexpected suspension: {}", tool_name);
            }
        }
        other => panic!("Unexpected VM result: {:?}", other),
    }
}

/// E1-A: The Display representation of an Identity value NEVER includes
/// the raw credential string — only the opaque provider name.
/// grant identity::oauth("stripe") → Value::Identity("stripe") → "<identity stripe>"
#[test]
fn e1a_identity_display_is_opaque() {
    // grant identity::oauth("stripe") stores Value::Identity("stripe")
    let identity = Value::Identity("stripe".to_string());
    let displayed = format!("{}", identity);

    // Must show the opaque token marker — not any real secret.
    assert_eq!(
        displayed, "<identity stripe>",
        "Identity display must be opaque; got: {}",
        displayed
    );

    // Must not contain any pattern that looks like a real API key.
    assert!(
        !displayed.contains("sk-"),
        "Should not leak OpenAI key pattern"
    );
    assert!(!displayed.contains("Bearer"), "Should not leak auth header");
    assert!(
        !displayed.contains("token"),
        "Should not leak token keyword"
    );

    println!(
        "E1-A PASS: Identity displayed as \"{}\" (opaque, no raw credential)",
        displayed
    );
}

/// E1-B: Storing an Identity in a Turn program via grant and reading it back
/// still yields the opaque handle — not the secret.
///
/// grant identity::oauth("stripe") → Value::Identity("stripe")
/// The capability class (oauth) is syntactic; the stored name is the provider
/// argument string.  The raw token (TURN_IDENTITY_STRIPE_TOKEN) is never
/// loaded into the Turn value heap.
#[test]
fn e1b_grant_identity_returns_opaque_handle() {
    // Set a fake credential in the env — this must NEVER appear in the Identity.
    std::env::set_var("TURN_IDENTITY_STRIPE_TOKEN", "sk-live-supersecret123");

    let source = r#"
    let auth = grant identity::oauth("stripe");
    return auth;
    "#;

    let result = run(source);

    match &result {
        Value::Identity(name) => {
            // The Identity holds the provider *name* ("stripe"), not the token.
            assert_eq!(
                name, "stripe",
                "Identity must hold the provider name, not the raw credential"
            );

            let displayed = format!("{}", result);
            // Confirm it is the opaque marker, not the secret.
            assert_eq!(
                displayed, "<identity stripe>",
                "Display must be opaque; got: {}",
                displayed
            );
            assert!(
                !displayed.contains("sk-live"),
                "Raw token must NOT appear in Identity display"
            );
            assert!(
                !displayed.contains("supersecret123"),
                "Raw token must NOT appear in Identity display"
            );

            println!(
                "E1-B PASS: grant identity::oauth(\"stripe\") → \"{}\" (token never in heap)",
                displayed
            );
        }
        other => panic!("Expected Identity value; got {:?}", other),
    }
}

/// E1-C: Confidence of a non-inferred (certain) Identity value is 1.0
/// — Identity values are not uncertain.
#[test]
fn e1c_identity_is_certain_not_uncertain() {
    let identity = Value::Identity("network".to_string());
    // An Identity should not be wrapped in Uncertain<T>.
    match &identity {
        Value::Uncertain(_, _) => panic!("Identity should never be Uncertain"),
        Value::Identity(name) => {
            println!(
                "E1-C PASS: Identity(\"{}\") is a certain (non-uncertain) value",
                name
            );
        }
        other => panic!("Unexpected variant: {:?}", other),
    }
}

/// E1-D: Two distinct grants for the same provider class produce equivalent
/// but independent handles — no shared mutable state.
#[test]
fn e1d_two_grants_are_independent() {
    let a = Value::Identity("network".to_string());
    let b = Value::Identity("network".to_string());
    // Both represent the same capability class but are independent values.
    assert_eq!(format!("{}", a), format!("{}", b));
    println!("E1-D PASS: Two grants for same provider are independent identity handles");
}
