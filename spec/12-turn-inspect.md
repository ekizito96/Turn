# 12. `turn inspect` — VM Observability

**Status:** Implemented in v1.0.0. Part of the core CLI.

---

## 1. Motivation

Turn agents are long-running, non-deterministic processes. Standard debuggers are not suited to them. `turn inspect` provides structured, zero-instrumentation observability into the full internal state of any suspended agent by reading its serialized `VmState` from the store.

---

## 2. CLI Usage

```bash
turn inspect <agent-id> [--store <path>]
```

| Argument | Default | Description |
|----------|---------|-------------|
| `<agent-id>` | required | The ID used with `turn run --id` |
| `--store <path>` | `.turn_store` | Path to the agent state store directory |

---

## 3. Requirements

The agent must have called `suspend` at least once, or have been suspended mid-execution waiting for a tool result. The command reads the most recent checkpoint for the given ID.

---

## 4. Output Sections

`turn inspect` prints five sections derived from the `VmState` struct:

### [1] Tripartite Context (`runtime.context`)

Displays P0 (system/primacy), P1 (working/recency), and P2 (episodic/middle) tiers in order. P0 is locked and never evicted. P1 evicts its oldest entries to P2 when capacity is reached. The rendered prompt order is P0 then P2 then P1.

### [2] Durable Memory (`runtime.memory`)

All key-value pairs from `remember`. Printed as `key => value`. This store is fully isolated per process.

### [3] Actor Mailbox (`mailbox`)

All messages currently queued in the process mailbox. These are values waiting for `receive` or `gather` to consume them.

### [4] Cognitive Belief State (`runtime.last_confidence`)

The `f64` confidence score of the most recent `infer` call, stored in `Runtime.last_confidence`. Color thresholds:

| Range | Color | Meaning |
|-------|-------|---------|
| 0.80 and above | Green | High confidence |
| 0.50 to 0.79 | Yellow | Moderate confidence |
| Below 0.50 | Red | Low confidence, likely caused retry or escalation |

### [5] Supervisor Tree (`scheduler` + `parent_pid`)

All processes in the scheduler at the time of the last checkpoint. Shows each PID, its relationship to the inspected process, and its status (Suspended, Running, or Completed).

---

## 5. OODA Loop Pattern

`turn inspect` is designed for the Observe-Orient-Decide-Act debugging cycle:

1. **Observe:** Run `turn inspect <id>` to read exact state at suspension.
2. **Orient:** Check Cognitive Belief State for low confidence explaining unexpected behavior.
3. **Decide:** Revise the system prompt (P0), input data, or confidence threshold.
4. **Act:** Run `turn run` again. The VM resumes from the exact checkpoint.

---

## 6. Implementation

- `Commands::Inspect` subcommand in `impl/src/main.rs`
- Reads `VmState` via `FileStore::load` from `impl/src/store.rs`
- Confidence telemetry stored in `Runtime.last_confidence` in `impl/src/runtime.rs`
- Confidence captured from `Value::Uncertain` on every tool result return in `impl/src/runner.rs`

---

## 7. State Storage Format

Each agent checkpoint is stored as a single JSON file at `<store>/<agent-id>.json`. The file contains the full `VmState` including frames, stack, runtime (env, context, memory, structs, last_confidence), mailbox, scheduler, and next_pid. This file can be version-controlled, inspected directly with any JSON tool, or copied between machines.
