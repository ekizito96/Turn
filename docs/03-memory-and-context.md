# Memory and Context

Every Turn process has two forms of working knowledge: **context** (per-turn scratchpad) and **memory** (permanent semantic store).

## Memory: `remember` and `recall`

```turn
// Write to persistent semantic memory
remember("user_name", "Alice");
remember("last_topic", "pricing strategy for enterprise tier");

// Read back — returns null if key not found
let name = recall("user_name");
```

Memory persists across turns, across `suspend` boundaries, and across process restarts. It is part of the durable agent state serialized by the VM.

### Semantic Auto-Recall

Before every `infer` call, the VM performs an HNSW nearest-neighbor search over all memories and injects the most relevant results into the context payload automatically. No retrieval code needed.

### Ebbinghaus Decay

Memories are de-weighted over time if not accessed. This prevents stale memories from polluting every inference call indefinitely — the agent's memory stays focused on what's actually relevant.

## Context Window

```turn
context.append("You are a senior financial analyst.");
context.append("Client has conservative risk profile.");

// Both lines are included in the next infer call
let result = infer Portfolio { "Recommend an allocation."; };
```

The context window is **token-budgeted** — the VM tracks how many tokens have been consumed against a configured limit.

### Priority Stack

Context is organized in four priority layers:

| Priority | Name | Eviction Policy |
|---|---|---|
| P0 | System / Mission | **Never evicted** |
| P1 | Plan / Scratchpad | Summarized when budget approached |
| P2 | History | Dropped oldest-first |
| P3 | User Input | Protected for current turn |

When `context.append()` would exceed the budget, the VM applies the **Entropic Expansion Policy**: evict P2 first, summarize P1 if needed. P0 is never touched.

## Memory vs. Context

| | Memory | Context |
|---|---|---|
| Lifetime | Permanent | Per-turn |
| Capacity | Unlimited | Token-budgeted |
| Access | Semantic similarity search | Ordered stack |
| Primitive | `remember` / `recall` | `context.append()` |

→ Next: [Actors](04-actors.md)
