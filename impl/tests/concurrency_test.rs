use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::compiler::Compiler;
use turn::vm::{Vm, VmResult};
use turn::value::Value;

fn run(source: &str) -> Value {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut compiler = Compiler::new();
    let code = compiler.compile(&program);
    let mut vm = Vm::new(&code);
    match vm.run() {
        VmResult::Complete(v) => v,
        _ => panic!("Suspended?"),
    }
}

#[test]
fn test_spawn_syntax() {
    let source = r#"
    let pid = spawn turn() {
        return 42;
    };
    return pid; // Should return a PID value
    "#;
    let val = run(source);
    if let Value::Pid { node_id, local_pid } = val {
        assert_eq!(node_id, "local");
        assert!(local_pid > 1); // Root is 1, child is > 1
    } else {
        panic!("Expected PID, got {:?}", val);
    }
}

#[test]
fn test_send_receive_syntax() {
    let source = r#"
    let parent_pid = 1; // Assuming root PID is always 1?
    // Wait, Turn doesn't expose current PID yet via keyword.
    // But we can spawn a child that sends back to us if we pass our PID?
    // How to get current PID? `self`? No.
    // For this test, we can use a hardcoded assumption or just test child-to-child?
    
    // Parent spawns Child. Child sends to Parent. Parent receives.
    // But Parent doesn't know its own PID to give to Child!
    // We need `self_pid()` or similar?
    // Or just assume Root is 1 for testing.
    // But we don't have integer-to-pid cast.
    
    // Workaround: Use a shared closure/value? No, processes are isolated.
    // We definitely need `self` PID.
    
    // Let's implement `self_pid()` as a tool call? Or keyword?
    // Or `pid()`?
    
    // Actually, `spawn` returns PID.
    // Parent knows Child PID.
    // Parent can send to Child.
    // Child can receive.
    
    // Scenario: Parent spawns Child. Parent sends 100 to Child. Child receives and returns 100.
    // But how do we get Child result? Child `return`s value, but that just ends process.
    // We need Child to send back to Parent? But Child needs Parent PID.
    
    // Let's test Parent -> Child communication.
    
    // Child logic:
    // let val = receive;
    // return val;
    
    // Parent logic:
    // let child = spawn turn() { return receive; };
    // send child, 100;
    // return 0;
    
    // This doesn't prove Child got it, unless we inspect VM state.
    // But `run()` returns Parent result.
    
    // If we want to prove it works, we need two-way.
    // Since we lack `self()` pid, we can't easily do two-way unless we pass PID 1 manually?
    // But `send` requires `Pid` type. `1` is `Num`.
    // We can't cast Num to Pid in syntax yet.
    
    // Temporary Hack:
    // The test framework can inspect the Child?
    // Or we add `pid()` keyword?
    // Or we assume `spawn` enables 2-way?
    
    // Let's just test that it compiles and runs without error.
    let child = spawn turn() {
        let msg = receive;
        // Logic to verify msg?
        if msg == 100 { return true; } else { return false; }
    };
    
    let sent = send child, 100;
    return sent; // Should be true (child exists)
    "#;
    
    let val = run(source);
    assert_eq!(val, Value::Bool(true));
}

#[test]
fn test_monitor_unidirectional_signal() {
    let source = r#"
    // Spawn a child that immediately panics (throws)
    let child = spawn turn() {
        throw "I panicked";
    };
    
    // Parent monitors child
    monitor child;
    
    // Attempt to receive the DOWN signal
    // The VM scheduler will complete the child due to the unhandled throw,
    // and route a DOWN message back to the parent mailbox.
    let msg = receive;
    return msg;
    "#;
    
    let val = run(source);
    
    if let Value::Map(m) = val {
        assert_eq!(m.get("type"), Some(&Value::Str("DOWN".to_string())));
        assert_eq!(m.get("reason"), Some(&Value::Str("I panicked".to_string())));
    } else {
        panic!("Expected DOWN message Map, got {:?}", val);
    }
}

#[test]
fn test_link_bidirectional_exit_signal() {
    let source = r#"
    // Spawn a child that returns cleanly
    let child = spawn turn() {
        return "clean exit";
    };
    
    // Parent links child
    link child;
    
    // The VM scheduler will complete the child cleanly,
    // and route an EXIT message back to the parent mailbox because of the link.
    let msg = receive;
    return msg; // We should get the exit message map
    "#;
    
    let val = run(source);
    
    if let Value::Map(m) = val {
        assert_eq!(m.get("type"), Some(&Value::Str("EXIT".to_string())));
        assert_eq!(m.get("reason"), Some(&Value::Str("clean exit".to_string())));
    } else {
        panic!("Expected EXIT message Map, got {:?}", val);
    }
}

#[test]
fn test_spawn_remote_syntax() {
    let source = r#"
    let success = spawn_remote("192.168.1.5", turn() {
        return 42;
    });
    return success;
    "#;
    
    let val = run(source);
    // Since we don't have a real network switchboard attached in the test runner,
    // the generic `send_remote` proxy will fail and return false, printing a warning.
    // This correctly tests that the syntax parses, compiles, and evaluates.
    if let Value::Bool(b) = val {
        assert_eq!(b, false);
    } else {
        panic!("Expected boolean result from spawn_remote, got {:?}", val);
    }
}
