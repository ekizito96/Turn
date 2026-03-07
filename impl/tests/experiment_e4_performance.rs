// Experiment E4: VM Performance Microbenchmarks
//
// Measures the overhead of Turn's core safety mechanisms on Apple M4
// (10-core CPU, 10-core GPU, 16 GB unified memory, macOS 15).
// Each measurement is averaged over N iterations using std::time::Instant.
//
// The dominant cost in production is the LLM network round-trip (100ms–5s);
// these benchmarks demonstrate that the VM's structural overhead is negligible.

use std::time::Instant;
use turn::compiler::Compiler;
use turn::lexer::Lexer;
use turn::parser::Parser;
use turn::value::Value;
use turn::vm::{Vm, VmResult};

fn compile_and_run(source: &str) -> Value {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut compiler = Compiler::new();
    let code = compiler.compile(&program);
    let mut vm = Vm::new(&code);
    match vm.run() {
        VmResult::Complete(v) => v,
        _ => Value::Null,
    }
}

fn compile_bytecode(source: &str) -> Vec<turn::bytecode::Instr> {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let program = Parser::new(tokens).parse().unwrap();
    let mut compiler = Compiler::new();
    compiler.compile(&program)
}

fn run_bytecode(code: &[turn::bytecode::Instr]) -> Value {
    let mut vm = Vm::new(code);
    match vm.run() {
        VmResult::Complete(v) => v,
        _ => Value::Null,
    }
}

const ITERS: u64 = 10_000;

/// E4-1: Bytecode execution throughput — simple arithmetic loop.
/// Measures raw opcode dispatch speed without I/O or inference.
#[test]
fn e4_bytecode_execution_throughput() {
    let source = r#"
    let x = 1 + 2;
    let y = x * 3;
    let z = y - 1;
    return z;
    "#;
    let code = compile_bytecode(source);

    // Warmup
    for _ in 0..100 {
        run_bytecode(&code);
    }

    let start = Instant::now();
    for _ in 0..ITERS {
        let v = run_bytecode(&code);
        assert_eq!(v, Value::Num(8.0));
    }
    let elapsed = start.elapsed();
    let per_iter_us = elapsed.as_micros() as f64 / ITERS as f64;

    println!(
        "E4-1: Bytecode execution (3-op arithmetic) | {} iters | {:.2} µs/iter | {:.0} ops/sec",
        ITERS,
        per_iter_us,
        1_000_000.0 / per_iter_us
    );
    // Should be well under 100µs per iteration on release build.
    // (Sanity threshold — generous for debug build in CI)
    assert!(
        per_iter_us < 5000.0,
        "Unexpectedly slow: {:.2} µs/iter",
        per_iter_us
    );
}

/// E4-2: Process spawn latency.
/// Measures time to allocate a fresh (E, C, M, B, pc) process tuple.
#[test]
fn e4_process_spawn_latency() {
    let source = r#"
    let pid = spawn turn() { return 1; };
    return pid;
    "#;
    let code = compile_bytecode(source);

    // Warmup
    for _ in 0..100 {
        run_bytecode(&code);
    }

    let start = Instant::now();
    for _ in 0..ITERS {
        let v = run_bytecode(&code);
        assert!(matches!(v, Value::Pid(_)));
    }
    let elapsed = start.elapsed();
    let per_iter_us = elapsed.as_micros() as f64 / ITERS as f64;

    println!(
        "E4-2: Process spawn latency | {} iters | {:.2} µs/iter",
        ITERS, per_iter_us
    );
    assert!(
        per_iter_us < 10_000.0,
        "Process spawn too slow: {:.2} µs/iter",
        per_iter_us
    );
}

/// E4-3: Message send latency.
/// Measures time to enqueue a value into a process's mailbox.
#[test]
fn e4_message_send_latency() {
    let source = r#"
    let child = spawn turn() { return receive; };
    send child, 42;
    return true;
    "#;
    let code = compile_bytecode(source);

    for _ in 0..100 {
        run_bytecode(&code);
    }

    let start = Instant::now();
    for _ in 0..ITERS {
        run_bytecode(&code);
    }
    let elapsed = start.elapsed();
    let per_iter_us = elapsed.as_micros() as f64 / ITERS as f64;

    println!(
        "E4-3: Message send latency | {} iters | {:.2} µs/iter",
        ITERS, per_iter_us
    );
    assert!(
        per_iter_us < 10_000.0,
        "Message send too slow: {:.2} µs/iter",
        per_iter_us
    );
}

/// E4-4: Schema validation overhead (in-process JSON Schema check).
/// Validates a pre-constructed JSON object against a simple 3-field schema.
/// This measures the validation cost without an LLM round-trip.
#[test]
fn e4_schema_validation_latency() {
    use serde_json::json;

    let schema = json!({
        "type": "object",
        "properties": {
            "ticker":   { "type": "string" },
            "price":    { "type": "number" },
            "pe_ratio": { "type": "number" }
        },
        "required": ["ticker", "price", "pe_ratio"]
    });

    let valid_response = json!({
        "ticker":   "NVDA",
        "price":    135.42,
        "pe_ratio": 67.3
    });

    // Warmup
    for _ in 0..1000 {
        let compiled = jsonschema::JSONSchema::compile(&schema).unwrap();
        let _ = compiled.validate(&valid_response);
    }

    let n: u64 = 100_000;
    let compiled_schema = jsonschema::JSONSchema::compile(&schema).unwrap();

    let start = Instant::now();
    let mut all_valid = true;
    for _ in 0..n {
        if compiled_schema.validate(&valid_response).is_err() {
            all_valid = false;
        }
    }
    let elapsed = start.elapsed();
    assert!(
        all_valid,
        "All valid responses should pass schema validation"
    );

    let per_iter_ns = elapsed.as_nanos() as f64 / n as f64;
    let per_iter_us = per_iter_ns / 1000.0;

    println!(
        "E4-4: JSON Schema validation (3-field struct) | {} iters | {:.3} µs/iter | {:.0} validations/sec",
        n,
        per_iter_us,
        1_000_000.0 / per_iter_us
    );
    assert!(
        per_iter_us < 1000.0,
        "Schema validation too slow: {:.3} µs/iter",
        per_iter_us
    );
}

/// E4-5: Confidence extraction latency.
/// Measures the cost of reading the confidence scalar from an Uncertain<T>.
#[test]
fn e4_confidence_extraction_latency() {
    use turn::vm::Vm;

    let source = "let c = confidence x; return c;";
    let code = compile_bytecode(source);

    let n: u64 = 100_000;
    let mut total_ns = 0u128;

    for _ in 0..n {
        let mut vm = Vm::new(&code);
        if let Some(p) = vm.scheduler.front_mut() {
            p.runtime.env.insert(
                "x".to_string(),
                Value::Uncertain(Box::new(Value::Num(42.0)), 0.87),
            );
        }
        let start = Instant::now();
        let result = vm.run();
        total_ns += start.elapsed().as_nanos();
        assert!(matches!(result, VmResult::Complete(Value::Num(_))));
    }

    let per_iter_ns = total_ns as f64 / n as f64;
    println!(
        "E4-5: Confidence extraction | {} iters | {:.0} ns/iter",
        n, per_iter_ns
    );
    assert!(
        per_iter_ns < 500_000.0,
        "Confidence extraction too slow: {:.0} ns/iter",
        per_iter_ns
    );
}
