# Turn runtime model (v0.4 Alpha)

**Status:** Public alpha spec. Turn execution is a **process** running. The **configuration** is the full state of that process at any point—its environment, managed context, managed memory, mailbox, tool registry, turn/control-cycle state, and remaining program. This document defines that state, transition rules for executing within a turn/control cycle, and lifetimes. Implementations and tools must conform.

---

## 1. Configuration (process state)

The **configuration** is the full state of the **process** at any point. It is a single value (e.g. a record or struct) with the following components.

| Component      | Type / meaning | Lifetime |
|----------------|----------------|----------|
| **env**        | Map from identifier to value. Current lexical bindings (variables, let-bound names). | Updated on `let`; pushed/popped on block entry/exit. |
| **context**    | Managed context buffer with **Priority Stack Architecture** (System, Plan, Scratchpad, History). | Per process. Updated by `context.append`. Monitored by token budget scheduler. |
| **memory**     | Managed memory store. Key-value map plus optional semantic addressing metadata. | Per process; may be persisted across runs (implementation-defined). Updated by `remember`; read by `recall`. |
| **mailbox**    | Queue of messages for actor-style concurrency (`send`/`receive`). | Per process. Updated by `send`; consumed by `receive`. |
| **tool_registry** | Tool registry: map from tool name (string) to handler. | Set at startup. Used when evaluating `call(name, arg)` and `infer` (via an inference effect). |
| **turn_state** | Current turn id (optional); pending suspension (if any). | Updated when entering a turn and when suspending/resuming on effects (`call`, `infer`, `suspend`). |
| **program**    | Remaining program to execute (or current turn body). | For interpreter: pointer into AST or current statement. |

**Notation:** We write a configuration as a tuple or record, e.g.:

```
Config = (env, context, memory, mailbox, tool_registry, turn_state, program)
```

For **serialization** (checkpoint, replay, debug): we serialize `env`, `context`, `memory`, `mailbox`, and `turn_state`. We do **not** serialize `tool_registry` (it is runtime setup) or the full `program` (we can store program source or AST separately). So the "serializable state" is `(env, context, memory, mailbox, turn_state)`.

### Invariants (preserved by every transition)

The following must hold at **every** step (and the runtime must enforce them so that the transition relation never produces a configuration that violates them):

1. **Context bound (Token Economics / Gas):** `context` is measured in exact token counts (like Gas in EVM). A `token_budget` is set. On `context.append(expr)`, if appending would exceed the budget, the VM executes the **Entropic Expansion Policy**: semantic summarization or Priority Stack eviction. The VM does **not** allow context to silently overflow context windows.
2. **Priority Stack Preservation:** The `context` is internally a priority stack. P0 (System/Mission) is never evicted. P1 (Working Plan/Scratchpad) is summarized. P2 (Chat/History) is dropped first.
3. **Configuration well-formedness:** `env` is a finite map; `context` is a valid priority stack; `memory` is a key-value map; `tool_registry` is a map from tool names to handlers; `program` is a valid remaining program (or the empty program). No component is undefined or malformed.
3. **Serializable state:** The tuple `(env, context, memory, mailbox, turn_state)` is sufficient to restore execution (with program and tool_registry provided separately). So checkpointing does not lose information needed to resume.

Implementations must maintain these invariants. The transition relation is defined so that every step preserves them (context bound by eviction or fail on append; well-formedness by construction; serializable state unchanged by step).

---

## 2. Values (alpha)

- **Literal values:** numbers, strings, booleans (`true`, `false`), `null`.
- **Operators:** `+` (concatenation/addition), `==`, `!=` (equality), `and`, `or` (logical, short-circuit). See [02-grammar.md](02-grammar.md) §7 for semantics.
- **No first-class functions in v1** (no closures). So env maps ids to literals or to internal representations.
- **Tool call result:** When we resume from a tool call, the result is a value (e.g. string or number) that the runtime provides. That value is what the `call` statement "returns" to the program (e.g. bound to a variable by a following let, or discarded).

We leave "what is a value" minimal: numbers, strings, and possibly a dedicated "suspension" or "tool_pending" token when we are suspended.

---

## 3. One transition: execute one step

We define a **small-step** transition: one step takes the configuration to a new configuration or to a **suspension**.

**Step relation:**  
`Config → Config'`  or  `Config → Suspension(effect_name, arg, continuation)`

**Suspension** means: the program has evaluated to an **effect boundary** (`call`, `infer`, or `suspend`) and the runtime must perform an external action or durable commit. The **continuation** is the rest of the program (and env, etc.) that will run when we resume with the effect result (or a null result for `suspend`).

**Rules (informal):**

1. **Statement sequencing:** If the current program is `stmt; rest`, evaluate `stmt`:
   - If `stmt` is `let id = expr`: evaluate `expr` in env; extend env with `id → value`; continue with `rest`.
   - If `stmt` is `context.append(expr)`: evaluate `expr`; append to context (if under bound); continue with `rest`. If context is full, follow error model (e.g. fail or evict).
   - If `stmt` is `remember(k, v)`: evaluate `k` and `v`; update memory with `k → v`; continue with `rest`.
   - If `stmt` is `call(tool_name, arg)`: evaluate `tool_name` and `arg`; produce **Suspension(tool_name, arg, continuation)**.
   - If `stmt` is `infer Type { prompt; }`: evaluate `Type` and `prompt`; produce **Suspension("llm_infer", {schema, prompt, context}, continuation)** (effect name is implementation-defined; semantics are suspension + typed result).
   - If `stmt` is `suspend;`: produce **Suspension("sys_suspend", null, continuation)** to force a durable checkpoint boundary.
   - If `stmt` is `return expr`: evaluate `expr`; the turn completes with that value. (No more steps for this turn.)
   - If `stmt` is `if expr block1 else block2`: evaluate `expr`; if truthy (non-falsy), next program is `block1`; else `block2`. Then continue.
   - If `stmt` is `while expr block`: evaluate `expr`; if truthy, next program is `block` followed by the same `while` (loop); else continue with `rest`.
   - If `stmt` is `expr;`: evaluate `expr`; discard result; continue with `rest`.
   - If `stmt` is `turn block`: enter the turn—next program is the block body; turn_state updated (e.g. turn_id incremented). Continue.

2. **Resumption:** When the runtime has a **Suspension(tool_name, arg, cont)** and the tool handler has produced a result `res`, we **resume**: replace the configuration with the continuation and a synthetic "result" value, and continue from the point after the `call`. (The exact way the result is fed back—e.g. a special variable or stack slot—is implementation detail; the spec only requires that the program can use the result.)

3. **Expression evaluation:** Expressions (in let, append, remember, call, return, if condition) are evaluated in the current env to a value. No side effects during expression evaluation except that we might eventually hit a `call` in a nested statement.

**One turn:** A **turn** is the execution of a `turn { body }` from start until (a) the body runs to completion (e.g. `return` or end of block), or (b) the body suspends on an effect boundary (`call`, `infer`, `suspend`). So "one turn" is the maximal sequence of steps that starts with entering a turn and ends with either turn completion or suspension.

**Big-step (optional):** We can also define a **big-step** relation for a whole turn: `(config, turn_body) ⇓ (config', result)` or `(config, turn_body) ⇓ Suspension(...)`. The small-step relation defines the same behavior; big-step is a convenient abstraction for "run this turn to completion or suspension."

### Deterministic Semantics

Turn's core language is **deterministic**: given a configuration and a sequence of external inputs (tool results, LLM outputs), execution is **reproducible**. The transition relation `Config → Config'` is a function: same config + same inputs → same next config.

**Non-determinism is quarantined at effect boundaries:**
- **Tool calls:** `call(tool_name, arg)` suspends; the tool handler may be non-deterministic (network timing, stochastic APIs). When we resume with a result, that result becomes part of the **input sequence** for reproducibility.
- **LLM calls:** `infer` is an effect; outputs are non-deterministic but become inputs for replay.

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

For the alpha we specify a **default runtime** so that a Turn program can run without external setup:

- **Context:** In-memory buffer; max size N (e.g. 100 entries or 10_000 tokens—implementation chooses). When full, append either returns an error or evicts oldest (see [05-types-and-errors.md](05-types-and-errors.md)).
- **Memory:** In-memory key-value map. Persistence is implementation-defined.
- **Tool registry:** At least one built-in tool, e.g. `echo`, so that `call("echo", "hello")` works. Handler: return the argument as the result (or print and return ok).

So "run this Turn program" means: create one **process** with env (empty), context (empty, bounded), memory (empty), mailbox (empty), tool registry (echo + any user-provided tools), turn_state (no turn); then run the program from the first statement.

---

## 6. Summary

- **Configuration** = process state = (env, context, memory, mailbox, tool_registry, turn_state, program).
- **One step** = small-step transition or suspension. **One turn** = run a turn body to completion or suspension.
- **Serializable state** = (env, context, memory, turn_state) for checkpoint/replay of the agent.
- **Default runtime** = one agent with in-memory context object (bounded), in-memory memory object, at least `echo` tool.

This document is the single source of truth for the runtime. Implementations (interpreter, debugger, trace viewer) must conform to this model.

## 7. The Universal Loop (Implementation Pattern)

To achieve the "Universal Agent" capability (durable, pausable, resumable), implementations should follow this loop pattern:

1.  **Load:** Initialize VM with Program + State (or fresh).
2.  **Run:** Execute until `Complete` or `Suspended`.
3.  **Handle Suspension:**
    *   If `Suspended(tool, arg, continuation)`:
    *   **Persist:** Save `continuation` to durable storage (mechanism implementation-defined).
    *   **Execute:** Run the tool (async, human-in-the-loop, etc.).
    *   **Resume:** Load `continuation`, inject `result`, and goto Step 2.
4.  **Complete:** Return final value.

This loop ensures that the agent is never "blocked" on a thread, but rather "suspended" in state. This is the key to scalable, long-running agentic software.
