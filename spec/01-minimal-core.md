# Turn minimal core (v0.4 Alpha)

**Status:** Public alpha spec. This document defines the smallest set of primitives Turn must implement to support durable, long-running agentic computation. The program defines the behavior of a single **Turn process** (one instance in v0.4). That process has runtime-managed **context** and **memory**, and it executes in bounded **control cycles** with explicit **effect boundaries**.

The choice of primitives is justified: each primitive is necessary and not derivable from the others—see [00-design-mandate.md](00-design-mandate.md).

---

## 1. What is in the minimal core

| Primitive | Operations | Rationale |
|-----------|------------|-----------|
| **Turn (closure)** | `turn { ... }` ; `turn(args...) -> Type { ... }` | First-class unit of behavior for spawning and composition. |
| **Context (managed)** | `context.append(expr)` | Bounded working context managed by the runtime via a **Priority Stack Architecture**. See Section 3. |
| **Memory (managed)** | `remember(key, value)` ; `recall(key)` | Persistent process memory as a primitive store. Retrieval is explicit; higher-level structures are libraries. |
| **Effect boundary** | `call(tool_name, arg)` | External effects suspend execution and resume with a value. Enables deterministic replay given the same effect results. |
| **Native inference** | `infer Type { prompt_expr; }` | Probabilistic effect returning a typed value. Type validation is part of the runtime contract (cognitive type safety). |
| **Explicit checkpoint** | `suspend;` | Forces a durable checkpoint boundary (orthogonal persistence). |
| **Concurrency (actor)** | `spawn turn() { ... };` | Spawn concurrent processes with isolated state; communication primitives are part of the runtime model. |

**Expressions and statements:** Enough to write real agents without friction.

- **Literals:** number, string, `true`, `false`, `null`, `[ list ]`, `{ map }`.
- **Variables:** identifier (bound by `let`).
- **Operators:** `+` (concatenation/addition), `==`, `!=`, `<`, `>`, `<=`, `>=`, `and`, `or`, `!`.
- **Indexing:** `expr[index]` for lists and maps.
- **Member access:** `expr.field` for structs/maps (runtime-defined for each value kind).
- **Conditional:** `if expr block else block`.
- **Loop:** `while expr block`.
- **Block:** `{ stmt... }` with `let`, `return`, etc.
- **Let:** `let id = expr;` (local binding).
- **Structs:** `struct Name { field: Type, ... };` for cognitive type safety and schema definitions.

`recall(key)` returns `null` when key is missing.

## 2. Built-in tools (alpha)

Turn includes a default tool registry for bootstrap and testing. These are **tools**, not language primitives.

- `echo(val)`: Returns the value.
- `sleep(seconds)`: Pauses execution.
- `http_get(url)`: Performs GET request.
- `http_post({url, body})`: Performs POST request with JSON body.
- `llm_generate({messages, model})`: Calls LLM API (OpenAI compatible).
- `json_parse(str)`: Parses JSON string to Value.
- `json_stringify(val)`: Converts Value to JSON string.

---

## 3. What is deferred (not in the v1.0 core)

| Concept | Deferred to | Note |
|---------|-------------|------|
| **Supervisor trees** | Next | `link`/`monitor` and restart strategies. |
| **Networking / remote PIDs** | Future | Distributed messaging is outside minimal core. |

---

## 4. Syntax surface (conventional, not S-expr)

We choose **conventional keyword/block syntax** (like Python/JS) for readability and one obvious way:

- `turn { ... }` for a turn.
- `context.append(expr);` for context append.
- `remember(key, value);` and `recall(key)` (statement and expression).
- `call(tool_name, args);` for tool call (statement; we get result when resumed).

So: **no S-expressions** in the alpha core. The grammar (see [02-grammar.md](02-grammar.md)) is statement- and expression-based with keywords and blocks.

---

## 5. Single process (alpha)

- One **program** = the behavior of **one Turn process**. The program is a sequence of statements (including `turn` closures and spawned turns).
- The process has runtime-managed **context** and **memory** objects.
- The process has a **tool registry** (provided by the runtime; default: at least one built-in tool, e.g. `echo`).

---

## 6. Summary

**Alpha minimal core:** One process with turn closures, managed context (`context.append`), managed memory (`remember`/`recall`), explicit effect boundaries (`call`, `infer`, `suspend`), and actor-style concurrency (`spawn`). Everything else (structured context, richer policies, supervision, distribution) is layered on later.
