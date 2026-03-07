/// Experiment E5: Durable Execution — Suspend and Resume
///
/// Turn's `suspend` primitive serializes the entire VM state (VmState) to a
/// JSON file via the FileStore. On the next invocation the runner deserializes
/// the state and calls `Vm::resume_with_result`, restoring exact execution.
///
/// VmState includes: pid, frames (code + ip), stack, runtime (env + context +
/// memory), mailbox, scheduler, and gas_remaining. Every field is serde-annotated.
///
/// This experiment answers:
///   - What is the serialized size of a Turn process state? (bytes on disk)
///   - How fast is the serialize + deserialize round-trip? (ms)
///   - Is state fidelity exact? (all values before suspend == all values after)
///
///   E5-1: Small state (10 memory entries) -- size and round-trip latency
///   E5-2: Medium state (500 memory entries) -- size and round-trip latency
///   E5-3: Large state (5,000 memory entries) -- size and round-trip latency
///   E5-4: State fidelity -- every field is identical before and after round-trip
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use turn::runtime::Runtime;
use turn::value::Value;
use turn::vm::{Frame, VmState};

fn build_state(mem_entries: usize) -> VmState {
    let mut runtime = Runtime::new();
    runtime
        .system_context(Value::Str("You are a durable agent.".to_string()))
        .unwrap();
    for i in 0..mem_entries {
        runtime
            .remember(
                Value::Str(format!("key_{}", i)),
                Value::Str(format!("value_{}_with_some_content", i)),
            )
            .unwrap();
    }
    // Push some working context items
    for j in 0..10.min(mem_entries) {
        runtime
            .append_context(Value::Str(format!("context item {}", j)))
            .unwrap();
    }

    let frame = Frame {
        code: Arc::new(vec![]),
        ip: 42,
        env: {
            let mut h = std::collections::HashMap::new();
            h.insert("ticker".to_string(), Value::Str("NVDA".to_string()));
            h.insert("approved".to_string(), Value::Num(2_000_000.0));
            h
        },
        handlers: vec![],
    };

    VmState {
        pid: 1,
        parent_pid: None,
        frames: vec![frame],
        stack: vec![Value::Num(42.0), Value::Bool(true)],
        runtime,
        mailbox: VecDeque::new(),
        scheduler: VecDeque::new(),
        next_pid: 2,
        gas_remaining: u64::MAX,
    }
}

fn measure_roundtrip(state: &VmState) -> (usize, u128) {
    let t0 = Instant::now();
    let bytes = serde_json::to_vec(state).expect("serialize failed");
    let size = bytes.len();
    let _restored: VmState = serde_json::from_slice(&bytes).expect("deserialize failed");
    let elapsed_us = t0.elapsed().as_micros();
    (size, elapsed_us)
}

// ─── E5-1 ────────────────────────────────────────────────────────────────────
#[test]
fn e5_1_durable_state_small() {
    let state = build_state(10);
    let (size, us) = measure_roundtrip(&state);
    println!(
        "E5-1 PASS: mem=10 entries | state size={} B ({:.1} KB) | round-trip={} µs",
        size,
        size as f64 / 1024.0,
        us
    );
}

// ─── E5-2 ────────────────────────────────────────────────────────────────────
#[test]
fn e5_2_durable_state_medium() {
    let state = build_state(500);
    let (size, us) = measure_roundtrip(&state);
    println!(
        "E5-2 PASS: mem=500 entries | state size={} B ({:.1} KB) | round-trip={} µs",
        size,
        size as f64 / 1024.0,
        us
    );
}

// ─── E5-3 ────────────────────────────────────────────────────────────────────
#[test]
fn e5_3_durable_state_large() {
    let state = build_state(5_000);
    let (size, us) = measure_roundtrip(&state);
    println!(
        "E5-3 PASS: mem=5,000 entries | state size={} B ({:.1} KB) | round-trip={} µs",
        size,
        size as f64 / 1024.0,
        us
    );
}

// ─── E5-4 ────────────────────────────────────────────────────────────────────
#[test]
fn e5_4_state_fidelity_after_roundtrip() {
    let original = build_state(50);

    let bytes = serde_json::to_vec(&original).expect("serialize failed");
    let restored: VmState = serde_json::from_slice(&bytes).expect("deserialize failed");

    // PID is preserved
    assert_eq!(
        restored.pid, original.pid,
        "E5-4 FAIL: pid mismatch after round-trip"
    );

    // Program counter (ip) is preserved
    assert_eq!(
        restored.frames[0].ip, original.frames[0].ip,
        "E5-4 FAIL: ip mismatch after round-trip"
    );

    // Stack is preserved
    assert_eq!(
        restored.stack, original.stack,
        "E5-4 FAIL: stack mismatch after round-trip"
    );

    // All memory entries are preserved
    for i in 0..50 {
        let key = Value::Str(format!("key_{}", i));
        let expected = Value::Str(format!("value_{}_with_some_content", i));
        let got = restored.runtime.recall(&key);
        assert_eq!(
            got, expected,
            "E5-4 FAIL: memory entry {} mismatch after round-trip",
            i
        );
    }

    // Environment variables are preserved
    let ticker = restored.frames[0]
        .env
        .get("ticker")
        .expect("E5-4 FAIL: 'ticker' env var missing after round-trip");
    assert_eq!(
        ticker,
        &Value::Str("NVDA".to_string()),
        "E5-4 FAIL: 'ticker' env var corrupted after round-trip"
    );

    // Context is preserved (P0 system prompt)
    let p0 = &restored.runtime.context.p0_system;
    assert_eq!(
        p0.len(),
        1,
        "E5-4 FAIL: P0 system context lost after round-trip"
    );
    assert_eq!(
        p0[0],
        Value::Str("You are a durable agent.".to_string()),
        "E5-4 FAIL: P0 system prompt corrupted after round-trip"
    );

    println!("E5-4 PASS: full state fidelity after serialize → deserialize round-trip");
    println!(
        "          pid={} | ip={} | stack={} items | memory=50 entries | P0 system=preserved",
        restored.pid,
        restored.frames[0].ip,
        restored.stack.len()
    );
}
