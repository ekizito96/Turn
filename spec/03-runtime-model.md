# Turn runtime model (v1)

**Status:** Locked for v1. Turn is object-oriented: execution is the **agent** running. The **configuration** is the full state of that agent at any point—its environment, its context object, its memory object, its tool registry, turn state, and remaining program. This document defines that state, one transition rule for executing one turn, and lifetimes. Implementations and tools must conform.

---

## 1. Configuration (agent state)

The **configuration** is the full state of the **agent** at any point. It is a single value (e.g. a record or struct) with the following components. Conceptually: the agent has these objects and this execution state.

| Component      | Type / meaning | Lifetime |
|----------------|----------------|----------|
| **env**        | Map from identifier to value. Current lexical bindings (variables, let-bound names). | Updated on `let`; pushed/popped on block entry/exit. |
| **context**    | The agent's **context object**: bounded buffer of values (messages or state entries). Max size N (fixed for v1). | Per agent (per run). Updated by `context.append`. When at max, append either fails or evicts (see error model). |
| **memory**     | The agent's **memory object**: key-value store. Keys and values are values (e.g. strings). | Per agent; may be persisted across runs (implementation-defined). Updated by `remember`; read by `recall`. |
| **tool_registry** | The agent's **tool registry**: map from tool name (string) to handler. | Set at startup. Used when evaluating `call(name, arg)`. |
| **turn_state** | Current turn id (optional); pending suspension (if any). | Updated when entering a turn and when suspending/resuming on `call`. |
| **program**    | Remaining program to execute (or current turn body). | For interpreter: pointer into AST or current statement. |

**Notation:** We write a configuration as a tuple or record, e.g.:

```
Config = (env, context, memory, tool_registry, turn_state, program)
```

For **serialization** (checkpoint, replay, debug): we serialize `env`, `context`, `memory`, and `turn_state`. We do **not** serialize `tool_registry` (it is runtime setup) or the full `program` (we can store program source or AST separately). So the "serializable state" is `(env, context, memory, turn_state)`.

### Invariants (preserved by every transition)

The following must hold at **every** step (and the runtime must enforce them so that the transition relation never produces a configuration that violates them):

1. **Context bound:** |context| ≤ N (or total context size ≤ M, if the runtime measures in tokens or bytes). On `context.append(expr)` when at the bound, the runtime either evicts (e.g. drop oldest) or fails (see error model); it does **not** allow context to grow beyond N.
2. **Configuration well-formedness:** `env` is a finite map; `context` is a sequence of values; `memory` is a key-value map; `tool_registry` is a map from tool names to handlers; `program` is a valid remaining program (or the empty program). No component is undefined or malformed.
3. **Serializable state:** The tuple `(env, context, memory, turn_state)` is sufficient to restore execution (with program and tool_registry provided separately). So checkpointing does not lose information needed to resume.

Implementations must maintain these invariants. The transition relation is defined so that every step preserves them (context bound by eviction or fail on append; well-formedness by construction; serializable state unchanged by step).

---

## 2. Values (v1)

- **Literal values:** numbers, strings, booleans (`true`, `false`), `null`.
- **Operators:** `+` (concatenation/addition), `==`, `!=` (equality), `and`, `or` (logical, short-circuit). See [02-grammar.md](02-grammar.md) §7 for semantics.
- **No first-class functions in v1** (no closures). So env maps ids to literals or to internal representations.
- **Tool call result:** When we resume from a tool call, the result is a value (e.g. string or number) that the runtime provides. That value is what the `call` statement "returns" to the program (e.g. bound to a variable by a following let, or discarded).

We leave "what is a value" minimal: numbers, strings, and possibly a dedicated "suspension" or "tool_pending" token when we are suspended.

---

## 3. One transition: execute one step

We define a **small-step** transition: one step takes the configuration to a new configuration or to a **suspension**.

**Step relation:**  
`Config → Config'`  or  `Config → Suspension(tool_name, arg, continuation)`

**Suspension** means: the program has evaluated to a `call(tool_name, arg)` and we need to run the tool. The **continuation** is the rest of the program (and env, etc.) that will run when we resume with the tool result.

**Rules (informal):**

1. **Statement sequencing:** If the current program is `stmt; rest`, evaluate `stmt`:
   - If `stmt` is `let id = expr`: evaluate `expr` in env; extend env with `id → value`; continue with `rest`.
   - If `stmt` is `context.append(expr)`: evaluate `expr`; append to context (if under bound); continue with `rest`. If context is full, follow error model (e.g. fail or evict).
   - If `stmt` is `remember(k, v)`: evaluate `k` and `v`; update memory with `k → v`; continue with `rest`.
   - If `stmt` is `call(tool_name, arg)`: evaluate `tool_name` and `arg`; produce **Suspension(tool_name, arg, continuation)**. The continuation holds the rest of the program and current env/context/memory/turn_state.
   - If `stmt` is `return expr`: evaluate `expr`; the turn completes with that value. (No more steps for this turn.)
   - If `stmt` is `if expr block1 else block2`: evaluate `expr`; if truthy (non-falsy), next program is `block1`; else `block2`. Then continue.
   - If `stmt` is `while expr block`: evaluate `expr`; if truthy, next program is `block` followed by the same `while` (loop); else continue with `rest`.
   - If `stmt` is `expr;`: evaluate `expr`; discard result; continue with `rest`.
   - If `stmt` is `turn block`: enter the turn—next program is the block body; turn_state updated (e.g. turn_id incremented). Continue.

2. **Resumption:** When the runtime has a **Suspension(tool_name, arg, cont)** and the tool handler has produced a result `res`, we **resume**: replace the configuration with the continuation and a synthetic "result" value, and continue from the point after the `call`. (The exact way the result is fed back—e.g. a special variable or stack slot—is implementation detail; the spec only requires that the program can use the result.)

3. **Expression evaluation:** Expressions (in let, append, remember, call, return, if condition) are evaluated in the current env to a value. No side effects during expression evaluation except that we might eventually hit a `call` in a nested statement.

**One turn:** A **turn** is the execution of a `turn { body }` from start until (a) the body runs to completion (e.g. `return` or end of block), or (b) the body suspends on `call(...)`. So "one turn" is the maximal sequence of steps that starts with entering a turn and ends with either turn completion or suspension.

**Big-step (optional):** We can also define a **big-step** relation for a whole turn: `(config, turn_body) ⇓ (config', result)` or `(config, turn_body) ⇓ Suspension(...)`. The small-step relation defines the same behavior; big-step is a convenient abstraction for "run this turn to completion or suspension."

### Deterministic Semantics

Turn's core language is **deterministic**: given a configuration and a sequence of external inputs (tool results, LLM outputs), execution is **reproducible**. The transition relation `Config → Config'` is a function: same config + same inputs → same next config.

**Non-determinism is quarantined at effect boundaries:**
- **Tool calls:** `call(tool_name, arg)` suspends; the tool handler may be non-deterministic (network timing, stochastic APIs). When we resume with a result, that result becomes part of the **input sequence** for reproducibility.
- **LLM calls:** (Future) LLM calls are also effects; their outputs are non-deterministic but become inputs for replay.

**Why this matters:**
- **Debugging:** Replay the same input sequence → reproduce the bug.
- **Audit:** Log the input sequence → reconstruct what happened.
- **Testing:** Provide deterministic inputs → test agent behavior.
- **Physics/math:** Execution is a function \(S_{t+1} = F(S_t, e_t)\) where \(e_t\) are external events. This is **well-defined** and **reproducible**.

**Implementation:** The runtime must log external inputs (tool results) as part of the trace. Replay = restore configuration + replay input sequence.

---

## 4. Lifetimes and scope

| Component | Scope / lifetime |
|-----------|-------------------|
| **env**   | Lexical. Entering a block pushes a frame; exiting pops. `let` adds to current frame. Turn body is a block. |
| **context** | One per run. Survives across turns. Bounded; when full, behavior defined by error model. |
| **memory** | One per run. Survives across turns. May survive across process restarts if runtime persists it. |
| **tool_registry** | Process/runtime lifetime. Set at startup. |
| **turn_state** | Current turn id and pending suspension. Reset or updated when a new turn starts or when we resume from suspension. |

---

## 5. Default runtime (batteries included)

For v1 we specify a **default runtime** so that a Turn program can run without external setup:

- **Context:** In-memory buffer; max size N (e.g. 100 entries or 10_000 tokens—implementation chooses). When full, append either returns an error or evicts oldest (see [05-types-and-errors.md](05-types-and-errors.md)).
- **Memory:** In-memory key-value map. No persistence by default.
- **Tool registry:** At least one built-in tool, e.g. `echo`, so that `call("echo", "hello")` works. Handler: return the argument as the result (or print and return ok).

So "run this Turn program" means: create one **agent** with env (empty), context object (empty, max N), memory object (empty), tool registry (echo + any user-provided tools), turn_state (no turn); then run the program (the agent's behavior) from the first statement.

---

## 6. Summary

- **Configuration** = agent state = (env, context, memory, tool_registry, turn_state, program).
- **One step** = small-step transition or suspension. **One turn** = run a turn body to completion or suspension.
- **Serializable state** = (env, context, memory, turn_state) for checkpoint/replay of the agent.
- **Default runtime** = one agent with in-memory context object (bounded), in-memory memory object, at least `echo` tool.

This document is the single source of truth for the runtime. Implementations (interpreter, debugger, trace viewer) must conform to this model.
