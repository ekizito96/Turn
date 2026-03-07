/// Experiment E3: Structured Context Architecture
///
/// Turn implements a three-tier context model:
///   P0 (system)   -- fixed system prompts, always rendered FIRST  (primacy position)
///   P1 (working)  -- recent items in a VecDeque, bounded at MAX_CONTEXT_SIZE=100,
///                    always rendered LAST (recency position)
///   P2 (episodic) -- working items evicted when P1 overflows, rendered in the MIDDLE
///
/// The flat rendering order P0 → P2 → P1 directly exploits the primacy/recency
/// attention advantage documented by Liu et al. (2023) "Lost in the Middle".
/// Items in P0 and P1 occupy the high-recall positions; P2 holds older context
/// that must still be preserved but tolerates lower attention weight.
///
/// This experiment verifies:
///   E3-1: P0 items are always rendered first in the flat context vector
///   E3-2: P1 items are always rendered last (after P2)
///   E3-3: When P1 overflows MAX_CONTEXT_SIZE, the oldest item is evicted to P2
///          (context is demoted, not dropped)
///   E3-4: Evicted-to-P2 items appear between P0 and P1 in the flat rendering
///   E3-5: Each spawned process receives an empty context (isolation invariant)
use turn::runtime::{Runtime, StructuredContext};
use turn::value::Value;

const MAX_CONTEXT_SIZE: usize = 100;

// ─── E3-1 ────────────────────────────────────────────────────────────────────
#[test]
fn e3_1_p0_always_rendered_first() {
    let mut ctx = StructuredContext::default();

    ctx.push_system(Value::Str(
        "SYSTEM: you are a compliance analyst".to_string(),
    ));
    ctx.push_working(Value::Str("Working item A".to_string()), MAX_CONTEXT_SIZE);
    ctx.push_working(Value::Str("Working item B".to_string()), MAX_CONTEXT_SIZE);

    let flat = ctx.to_flat_vec();
    assert!(!flat.is_empty());

    // P0 system prompt is always position 0
    match &flat[0] {
        Value::Str(s) => {
            assert!(
                s.starts_with("SYSTEM:"),
                "E3-1 FAIL: first flat item should be P0 system prompt, got: {}",
                s
            );
        }
        other => panic!("E3-1 FAIL: expected Str, got {:?}", other),
    }

    println!("E3-1 PASS: P0 system prompt is always first in flat context (primacy position)");
}

// ─── E3-2 ────────────────────────────────────────────────────────────────────
#[test]
fn e3_2_p1_always_rendered_last() {
    let mut ctx = StructuredContext::default();

    ctx.push_system(Value::Str("SYSTEM: analyst".to_string()));
    ctx.push_working(Value::Str("Old working item".to_string()), MAX_CONTEXT_SIZE);
    // Deliberately push P2-like old items by filling and overflowing
    for i in 0..MAX_CONTEXT_SIZE {
        ctx.push_working(Value::Str(format!("working_{}", i)), MAX_CONTEXT_SIZE);
    }
    let last_pushed = Value::Str("MOST_RECENT".to_string());
    ctx.push_working(last_pushed.clone(), MAX_CONTEXT_SIZE);

    let flat = ctx.to_flat_vec();
    let last = flat.last().expect("E3-2 FAIL: flat context is empty");

    assert_eq!(
        last,
        &Value::Str("MOST_RECENT".to_string()),
        "E3-2 FAIL: most recent P1 item must be at the end (recency position)"
    );

    println!("E3-2 PASS: most recently appended item is last in flat context (recency position)");
}

// ─── E3-3 ────────────────────────────────────────────────────────────────────
#[test]
fn e3_3_working_overflow_evicts_to_episodic_not_dropped() {
    let mut ctx = StructuredContext::default();

    // Push exactly MAX_CONTEXT_SIZE items to fill P1
    for i in 0..MAX_CONTEXT_SIZE {
        ctx.push_working(Value::Str(format!("item_{}", i)), MAX_CONTEXT_SIZE);
    }

    // At this point P1 is full, P2 is empty
    assert_eq!(
        ctx.p1_working.len(),
        MAX_CONTEXT_SIZE,
        "E3-3 setup: P1 should be full at MAX_CONTEXT_SIZE"
    );
    assert_eq!(
        ctx.p2_episodic.len(),
        0,
        "E3-3 setup: P2 episodic should be empty before overflow"
    );

    // Push one more item -- this overflows P1 and evicts the OLDEST to P2
    ctx.push_working(Value::Str("overflow_item".to_string()), MAX_CONTEXT_SIZE);

    assert_eq!(
        ctx.p1_working.len(),
        MAX_CONTEXT_SIZE,
        "E3-3 FAIL: P1 should remain at MAX_CONTEXT_SIZE after eviction"
    );
    assert_eq!(
        ctx.p2_episodic.len(),
        1,
        "E3-3 FAIL: exactly one item should have been evicted to P2"
    );

    // Verify the evicted item is "item_0" (the oldest)
    match &ctx.p2_episodic[0] {
        Value::Str(s) => assert_eq!(
            s, "item_0",
            "E3-3 FAIL: oldest item should be evicted to episodic, got: {}",
            s
        ),
        other => panic!("E3-3 FAIL: expected Str, got {:?}", other),
    }

    println!("E3-3 PASS: P1 overflow evicts oldest item to P2 episodic (demote, not drop)");
    println!(
        "          P1 size={}, P2 size={}",
        ctx.p1_working.len(),
        ctx.p2_episodic.len()
    );
}

// ─── E3-4 ────────────────────────────────────────────────────────────────────
#[test]
fn e3_4_episodic_items_rendered_between_system_and_working() {
    let mut ctx = StructuredContext::default();

    ctx.push_system(Value::Str("P0_SYSTEM".to_string()));

    // Overflow P1 to force one item into P2
    for i in 0..=MAX_CONTEXT_SIZE {
        ctx.push_working(Value::Str(format!("item_{}", i)), MAX_CONTEXT_SIZE);
    }

    // P2 should now contain "item_0"
    assert_eq!(ctx.p2_episodic.len(), 1);

    let flat = ctx.to_flat_vec();
    // Expected structure: [P0_SYSTEM, item_0(P2), item_1..item_100(P1)]
    let p0_idx = flat
        .iter()
        .position(|v| v == &Value::Str("P0_SYSTEM".to_string()))
        .expect("E3-4 FAIL: P0 item not found in flat vec");
    let p2_idx = flat
        .iter()
        .position(|v| v == &Value::Str("item_0".to_string()))
        .expect("E3-4 FAIL: evicted P2 item not found in flat vec");
    let p1_last_idx = flat
        .iter()
        .rposition(|v| matches!(v, Value::Str(s) if s.starts_with("item_") && s != "item_0"))
        .expect("E3-4 FAIL: P1 working items not found in flat vec");

    assert!(
        p0_idx < p2_idx,
        "E3-4 FAIL: P0 must precede P2 in flat rendering"
    );
    assert!(
        p2_idx < p1_last_idx,
        "E3-4 FAIL: P2 must precede P1 in flat rendering"
    );

    println!(
        "E3-4 PASS: flat rendering order confirmed as P0({}) → P2({}) → P1({})",
        p0_idx, p2_idx, p1_last_idx
    );
    println!(
        "          Primacy zone (P0): {} item(s)",
        ctx.p0_system.len()
    );
    println!(
        "          Episodic zone (P2): {} item(s)",
        ctx.p2_episodic.len()
    );
    println!(
        "          Recency zone (P1): {} item(s)",
        ctx.p1_working.len()
    );
}

// ─── E3-5 ────────────────────────────────────────────────────────────────────
#[test]
fn e3_5_fresh_process_has_empty_context() {
    // Each process gets a fresh Runtime::new() which contains StructuredContext::default()
    let rt1 = Runtime::new();
    let mut rt2 = Runtime::new();

    rt2.append_context(Value::Str("rt2 context item".to_string()))
        .unwrap();

    // rt1 context is still empty -- no shared state
    let rt1_flat = rt1.context.to_flat_vec();
    assert!(
        rt1_flat.is_empty(),
        "E3-5 FAIL: rt1 should have empty context, got {} items",
        rt1_flat.len()
    );

    let rt2_flat = rt2.context.to_flat_vec();
    assert_eq!(
        rt2_flat.len(),
        1,
        "E3-5 FAIL: rt2 should have exactly 1 context item"
    );

    println!("E3-5 PASS: each Runtime starts with an empty context (isolation invariant)");
    println!(
        "          rt1 context size={}, rt2 context size={}",
        rt1_flat.len(),
        rt2_flat.len()
    );
}
