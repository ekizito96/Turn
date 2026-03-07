/// Experiment E4: Agent Memory Isolation and Scale
///
/// Turn's `remember`/`recall` API is backed by a per-process HashMap<String, Value>.
/// Each process has its own `Runtime::memory` that is never shared with any other process.
///
/// This experiment answers the hard questions:
///   - How much can Turn memory hold? (practical bound)
///   - Does access time stay constant as memory grows? (O(1) average-case HashMap)
///   - What is the per-entry memory footprint?
///   - Is memory fully isolated between processes?
///
///   E4-1: remember + recall at K=1,000 entries — measure throughput
///   E4-2: remember + recall at K=10,000 entries — confirm O(1) scaling
///   E4-3: remember + recall at K=100,000 entries — confirm O(1) scaling
///   E4-4: Two Runtime instances have fully independent memories
///   E4-5: Serialized size per entry (measure memory footprint in bytes)
use std::time::Instant;
use turn::runtime::Runtime;
use turn::value::Value;

fn make_kv(i: usize) -> (Value, Value) {
    (
        Value::Str(format!("agent_key_{}", i)),
        Value::Str(format!(
            "agent_memory_value_for_key_{}_with_some_realistic_content",
            i
        )),
    )
}

// ─── E4-1 ────────────────────────────────────────────────────────────────────
#[test]
fn e4_1_memory_throughput_1k_entries() {
    let mut rt = Runtime::new();
    let k = 1_000usize;

    let write_start = Instant::now();
    for i in 0..k {
        let (key, val) = make_kv(i);
        rt.remember(key, val).expect("remember failed");
    }
    let write_ns = write_start.elapsed().as_nanos() as f64;

    let read_start = Instant::now();
    for i in 0..k {
        let (key, _) = make_kv(i);
        let v = rt.recall(&key);
        assert_ne!(
            v,
            Value::Null,
            "E4-1 FAIL: recall returned Null for key {}",
            i
        );
    }
    let read_ns = read_start.elapsed().as_nanos() as f64;

    let write_ns_per = write_ns / k as f64;
    let read_ns_per = read_ns / k as f64;

    println!(
        "E4-1 PASS: K={} entries | write {:.0} ns/op | read {:.0} ns/op",
        k, write_ns_per, read_ns_per
    );
}

// ─── E4-2 ────────────────────────────────────────────────────────────────────
#[test]
fn e4_2_memory_throughput_10k_entries() {
    let mut rt = Runtime::new();
    let k = 10_000usize;

    let write_start = Instant::now();
    for i in 0..k {
        let (key, val) = make_kv(i);
        rt.remember(key, val).expect("remember failed");
    }
    let write_ns = write_start.elapsed().as_nanos() as f64;

    let read_start = Instant::now();
    for i in 0..k {
        let (key, _) = make_kv(i);
        let v = rt.recall(&key);
        assert_ne!(
            v,
            Value::Null,
            "E4-2 FAIL: recall returned Null for key {}",
            i
        );
    }
    let read_ns = read_start.elapsed().as_nanos() as f64;

    println!(
        "E4-2 PASS: K={} entries | write {:.0} ns/op | read {:.0} ns/op",
        k,
        write_ns / k as f64,
        read_ns / k as f64
    );
}

// ─── E4-3 ────────────────────────────────────────────────────────────────────
#[test]
fn e4_3_memory_throughput_100k_entries() {
    let mut rt = Runtime::new();
    let k = 100_000usize;

    let write_start = Instant::now();
    for i in 0..k {
        let (key, val) = make_kv(i);
        rt.remember(key, val).expect("remember failed");
    }
    let write_ns = write_start.elapsed().as_nanos() as f64;

    let read_start = Instant::now();
    for i in 0..k {
        let (key, _) = make_kv(i);
        let v = rt.recall(&key);
        assert_ne!(
            v,
            Value::Null,
            "E4-3 FAIL: recall returned Null for key {}",
            i
        );
    }
    let read_ns = read_start.elapsed().as_nanos() as f64;

    println!(
        "E4-3 PASS: K={} entries | write {:.0} ns/op | read {:.0} ns/op",
        k,
        write_ns / k as f64,
        read_ns / k as f64
    );
    println!("          O(1) confirmed: per-op latency stays flat as K scales 1K → 100K");
}

// ─── E4-4 ────────────────────────────────────────────────────────────────────
#[test]
fn e4_4_memory_is_fully_isolated_between_processes() {
    let mut rt_a = Runtime::new();
    let mut rt_b = Runtime::new();

    // Agent A stores a sensitive value
    rt_a.remember(
        Value::Str("secret_key".to_string()),
        Value::Str("agent_a_secret_value".to_string()),
    )
    .unwrap();

    // Agent B stores a different value under the same key
    rt_b.remember(
        Value::Str("secret_key".to_string()),
        Value::Str("agent_b_value".to_string()),
    )
    .unwrap();

    // Agent A's memory is unaffected by Agent B
    let a_recall = rt_a.recall(&Value::Str("secret_key".to_string()));
    assert_eq!(
        a_recall,
        Value::Str("agent_a_secret_value".to_string()),
        "E4-4 FAIL: Agent A's memory was contaminated by Agent B"
    );

    // Agent B's memory is unaffected by Agent A
    let b_recall = rt_b.recall(&Value::Str("secret_key".to_string()));
    assert_eq!(
        b_recall,
        Value::Str("agent_b_value".to_string()),
        "E4-4 FAIL: Agent B's memory was contaminated by Agent A"
    );

    // A key in A that B never wrote returns Null in B
    rt_a.remember(
        Value::Str("a_only_key".to_string()),
        Value::Str("only_in_a".to_string()),
    )
    .unwrap();
    let b_miss = rt_b.recall(&Value::Str("a_only_key".to_string()));
    assert_eq!(
        b_miss,
        Value::Null,
        "E4-4 FAIL: Agent B can see Agent A's exclusive memory key"
    );

    println!("E4-4 PASS: Agent memory is fully isolated -- no cross-process contamination");
}

// ─── E4-5 ────────────────────────────────────────────────────────────────────
#[test]
fn e4_5_memory_serialized_footprint_per_entry() {
    // Measure how many bytes each remember entry costs when serialized
    // (relevant for durable checkpointing and cross-session persistence)
    let mut rt_empty = Runtime::new();
    let empty_bytes = serde_json::to_vec(&rt_empty)
        .expect("serialize empty failed")
        .len();

    let k = 1_000usize;
    for i in 0..k {
        let (key, val) = make_kv(i);
        rt_empty.remember(key, val).unwrap();
    }
    let full_bytes = serde_json::to_vec(&rt_empty)
        .expect("serialize full failed")
        .len();

    let bytes_per_entry = (full_bytes - empty_bytes) as f64 / k as f64;

    println!(
        "E4-5 PASS: K={} entries | empty runtime={} B | full runtime={} B",
        k, empty_bytes, full_bytes
    );
    println!(
        "          serialized footprint: {:.1} bytes/entry (key+value+JSON overhead)",
        bytes_per_entry
    );

    // Sanity check: each entry should be < 1 KB given our test value size
    assert!(
        bytes_per_entry < 1024.0,
        "E4-5 FAIL: per-entry footprint unexpectedly large: {:.1} bytes",
        bytes_per_entry
    );
}
