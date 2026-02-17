# Types and errors (v1)

**Status:** Locked for v1. Documents type-friendly design (for a future typed surface) and the error model. v1 has **no type syntax**; this spec ensures we don't paint ourselves into a corner.

---

## 1. Type-friendly design

We design the **runtime and primitives** so that a later type system can assign sensible types without breaking changes.

| Concept | v1 (untyped) | Future typing |
|--------|---------------|----------------|
| **Context** | Bounded buffer of values. | `Context<T>` or `context: Buffer<Message>`. Append takes `T`; index/iteration (if added) yield `T`. |
| **Memory** | Key-value store; keys and values are values. | `Memory<K, V>` or `memory: Map<K, V>`. `remember(k, v)` requires `k: K`, `v: V`; `recall(k)` returns `V` or `Option<V>`. |
| **Tool call** | `call(name, arg)` → result value. | `call(name, arg): R` where the registry maps `name` to a type like `(A) => R`. Tool registry could be typed: `Map<string, (arg: A) => R>`. |
| **Turn** | `turn { body }` produces a value or suspension. | `turn { body }: T` where body's return type is `T`; suspension is an effect. |
| **Values** | Numbers, strings (and bool if we add it). | Primitives and product/sum types if we add them. |

**Optional annotations (v1.1+):** If we add types to the syntax, we want to support optional annotations, e.g. `let x: string = "hi";` and `remember("k": string, v: int);` without requiring annotations everywhere. So the grammar and AST should leave room for an optional `: Type` after identifiers or in formal parameters.

**No breaking changes:** The runtime does not depend on types. Adding types later is a static layer (checking) and possibly runtime representations (e.g. tagged values); the config and transition rules in [03-runtime-model.md](03-runtime-model.md) stay the same.

---

## 2. Error model

We classify errors and define **recoverable** vs **fatal** and what the implementation should report.

### 2.1 Context full

- **When:** `context.append(expr)` and the context buffer is already at max size N.
- **Behavior (v1):** **Fatal** (abort this turn or program) **or** implementation-defined eviction (e.g. drop oldest entry then append). Spec allows either; implementation must document which.
- **Message (example):** `"context full (max N); append failed"` or `"context full; evicted oldest entry"`.

### 2.2 Tool not found

- **When:** `call(tool_name, arg)` and `tool_name` is not in the runtime tool registry.
- **Behavior:** **Fatal.** No resumption; turn (or program) fails.
- **Message (example):** `"tool not found: <tool_name>"`.

### 2.3 Memory errors

- **recall(key)** when key is missing: return a distinguished value (e.g. `null`, `nil`, or a type-safe option) **or** fail. v1: implementation may choose; if fail, **recoverable** (caller can avoid by checking) or **fatal** (abort). Spec recommends: return a **missing** sentinel so programs can branch (recoverable).
- **remember(k, v)** when store is full or read-only: **fatal** with message, e.g. `"memory full"` or `"memory read-only"`.

### 2.4 Other

- **Undefined variable:** Reference to an identifier not in env. **Fatal.** Message: `"undefined variable: <id>"`.
- **Type mismatch (if we add types):** Static error; not a runtime error in untyped v1.
- **Invalid expression in call/append/remember:** e.g. wrong number of arguments to `call`. **Fatal** with a clear message (e.g. `"call expects (tool_name, arg)"`).

### 2.5 Summary table

| Error | Recoverable / Fatal | Message (example) |
|-------|---------------------|--------------------|
| Context full | Fatal or evict (impl-defined) | `"context full (max N)"` |
| Tool not found | Fatal | `"tool not found: <name>"` |
| recall missing key | Prefer recoverable (return sentinel) | — |
| Memory full / read-only | Fatal | `"memory full"` / `"memory read-only"` |
| Undefined variable | Fatal | `"undefined variable: <id>"` |
| Invalid call/append form | Fatal | Descriptive message |

---

## 3. Observability

Errors should be **observable**: logged or surfaced so that debugging and tooling (e.g. trace viewer) can show why a turn or program failed. The runtime model ([03-runtime-model.md](03-runtime-model.md)) does not require a specific logging API, but implementations should produce a deterministic, machine-readable representation of failures (e.g. error code + message + location) for tooling.

---

## 4. Summary

- **Type-friendly:** Context, memory, and tool call are designed so we can add types later (Context<T>, Memory<K,V>, call: (A) => R) without changing the runtime.
- **v1 untyped:** No type annotations in the grammar.
- **Errors:** Context full (fatal or evict), tool not found (fatal), recall missing (prefer sentinel), undefined variable (fatal); all with clear messages.
- **Observability:** Failures should be reportable for debugging and tooling.
