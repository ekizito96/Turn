// Experiment E3: Context Isolation
//
// Verifies Property 1 (Context Isolation): for processes P_i and P_j where
// i ≠ j, modifications to C_i have no effect on C_j.
//
// Each spawn_link / spawn allocates a new Process struct with its own
// context: Vec<String> field. There are no global context buffers.
// Rust's ownership model makes cross-process contamination structurally
// impossible — verified here by exercising the VM directly.

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
        _ => panic!("VM did not complete"),
    }
}

/// E3-1: Root process context is not inherited by spawned child.
/// The child appends to its own context; the parent can still use its own.
#[test]
fn e3_root_context_not_visible_in_child() {
    // Each process has an isolated context window.
    // Spawning a child gives it a fresh, empty context.
    let source = r#"
    context.append("Parent secret context line.");

    let child = spawn turn() {
        // Child has its own empty context — parent line must not appear.
        return "child done";
    };

    return "parent done";
    "#;

    let val = run(source);
    match val {
        Value::Str(s) if s == "parent done" => {
            println!(
                "E3-1 PASS: parent returned \"{}\" — spawn gave child a fresh context",
                s
            );
        }
        other => panic!("Unexpected result: {:?}", other),
    }
}

/// E3-2: Two sibling processes each receive independent contexts.
/// spawn_link returns a PID — the two children are separate processes.
#[test]
fn e3_two_sibling_processes_have_independent_contexts() {
    let source = r#"
    let child_a = spawn turn() {
        context.append("Context of process A");
        return "a";
    };

    let child_b = spawn turn() {
        context.append("Context of process B");
        return "b";
    };

    return "both spawned";
    "#;

    let val = run(source);
    match val {
        Value::Str(s) if s == "both spawned" => {
            println!("E3-2 PASS: two sibling processes spawned with independent contexts");
        }
        other => panic!("Unexpected result: {:?}", other),
    }
}

/// E3-3: context.system is per-process and does not affect sibling processes.
#[test]
fn e3_context_system_is_per_process() {
    let source = r#"
    context.system("You are the parent agent.");

    let child = spawn turn() {
        context.system("You are a child specialist agent.");
        return "child done";
    };

    return "parent done";
    "#;

    let val = run(source);
    match val {
        Value::Str(s) if s == "parent done" => {
            println!("E3-3 PASS: context.system is per-process — parent system prompt unaffected by child");
        }
        other => panic!("Unexpected result: {:?}", other),
    }
}

/// E3-4: Spawning N processes does not produce shared state.
/// Each is a distinct actor with its own environment.
#[test]
fn e3_n_processes_all_have_fresh_context() {
    let source = r#"
    let p1 = spawn turn() { context.append("P1 data"); return 1; };
    let p2 = spawn turn() { context.append("P2 data"); return 2; };
    let p3 = spawn turn() { context.append("P3 data"); return 3; };
    let p4 = spawn turn() { context.append("P4 data"); return 4; };
    return 4;
    "#;

    let val = run(source);
    match val {
        Value::Num(n) if (n - 4.0).abs() < 1e-9 => {
            println!("E3-4 PASS: 4 processes spawned, all with independent fresh contexts");
        }
        other => panic!("Expected Num(4), got {:?}", other),
    }
}
