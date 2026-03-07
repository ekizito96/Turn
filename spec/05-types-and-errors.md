# Types and errors (v0.4 Alpha)

**Status:** Locked for v1. Documents type-friendly design (for a future typed surface) and the error model. v1 has **no type syntax**; this spec ensures we don't paint ourselves into a corner.

---

## 1. Type-friendly design

We design the **runtime and primitives** so that a later type system can assign sensible types without breaking changes.

Turn is process-centric; the type system should reflect that. In the minimal core we have one process with runtime-managed context and memory. The goal is to keep operational semantics stable while strengthening static guarantees.

| Concept | v1 (untyped) | Future typing |
|--------|---------------|----------------|
| **Process** | One instance; state = (env, context, memory, turn_state, program). | `Process` type with managed resources (context, memory) and effect capabilities (call, infer). |
| **Context** | A bounded buffer managed by the runtime. | `Context<T>` with policies for retention/promotion; append takes `T`. |
| **Memory** | A key-value store managed by the runtime. | `Memory<K, V>`; `remember(k, v)` requires `k: K`, `v: V`; `recall(k)` returns `V` or `Option<V>`. |
| **Tool call** | `call(name, arg)` → result value. | `call(name, arg): R` where the registry maps `name` to a type like `(A) => R`. |
| **Turn** | `turn { body }` produces a value or suspension. | `turn { body }: T` where body's return type is `T`; suspension is an effect. |
| **Values** | Numbers, strings, booleans (`true`, `false`), `null`. | Primitives and product/sum types if we add them. |

**Optional annotations (v1.1+):** If we add types to the syntax, we want to support optional annotations, e.g. `let x: string = "hi";` and `remember("k": string, v: int);` without requiring annotations everywhere. So the grammar and AST should leave room for an optional `: Type` after identifiers or in formal parameters.

**No breaking changes:** The runtime does not depend on types. Adding types later is a static layer (checking) and possibly runtime representations (e.g. tagged values); the config and transition rules in [03-runtime-model.md](03-runtime-model.md) stay the same.

---

## 2. Error model

We classify errors and define **recoverable** vs **fatal** and what the implementation should report.

### 2.1 Context full

- **When:** `context.append(expr)` and the context buffer is already at max size N.
- **Behavior (alpha):** **Fatal** (abort this turn or program) **or** implementation-defined eviction (e.g. drop oldest entry then append). Spec allows either; implementation must document which.
- **Message (example):** `"context full (max N); append failed"` or `"context full; evicted oldest entry"`.

### 2.2 Tool not found

- **When:** `call(tool_name, arg)` and `tool_name` is not in the runtime tool registry.
- **Behavior:** **Fatal.** No resumption; turn (or program) fails.
- **Message (example):** `"tool not found: <tool_name>"`.

### 2.3 Memory errors

- **recall(key)** when key is missing: return `null`. This is **recoverable**—programs can check `if x == null` or `if x` (null is falsy) before using the value.
- **remember(k, v)** when store is full or read-only: **fatal** with message, e.g. `"memory full"` or `"memory read-only"`.

### 2.4 Type mismatch (compile-time)

Since v1.0.0, Turn runs a semantic analysis pass before execution. If a function parameter has a declared type and the caller passes a value of a different type, the program is rejected at compile time with a structured error.

- **When:** `call(fn, arg)` where `fn` has parameter type `T` and `arg` has type `S`, and `S` is not compatible with `T`.
- **Behavior:** **Fatal at compile time.** The program never executes.
- **Message (example):** `"[10:1] Type mismatch: expected Struct("StripePayload", ...), got Struct("WrongPayload", ...)"`

Turn uses gradual typing. Functions with unannotated parameters or parameters typed as `Any` skip this check. Full enforcement requires explicit type annotations.

Coverage:
- Named struct mismatches (field-level layout compared)
- Primitive mismatches (`Num`, `Str`, `Bool`)
- Generic container mismatches (`List<Num>` vs `List<Str>`)
- `Void` parameter accepts any argument (Turn calling convention: `call(fn, {})` for no-arg closures)

### 2.5 Other

- **Undefined variable:** Reference to an identifier not in env. **Fatal.** Message: `"undefined variable: <id>"`.
- **Invalid expression in call/append/remember:** e.g. wrong number of arguments to `call`. **Fatal** with a clear message (e.g. `"call expects (tool_name, arg)"`).

### 2.6 Summary table

| Error | Recoverable / Fatal | Message (example) |
|-------|---------------------|-------------------|
| Context full | Fatal or evict (impl-defined) | `"context full (max N)"` |
| Tool not found | Fatal | `"tool not found: <name>"` |
| recall missing key | Recoverable (returns `null`) | none |
| Memory full / read-only | Fatal | `"memory full"` / `"memory read-only"` |
| Undefined variable | Fatal | `"undefined variable: <id>"` |
| Invalid call/append form | Fatal | Descriptive message |
| Type mismatch (annotated) | Fatal at compile time | `"Type mismatch: expected T, got S"` |

---

## 3. Observability

Errors should be **observable**: logged or surfaced so that debugging and tooling (e.g. trace viewer) can show why a turn or program failed. The runtime model ([03-runtime-model.md](03-runtime-model.md)) does not require a specific logging API, but implementations should produce a deterministic, machine-readable representation of failures (e.g. error code + message + location) for tooling.

---

## 4. Summary

- **Type-friendly:** Context, memory, and tool call are designed so we can add types later (Context<T>, Memory<K,V>, call: (A) => R) without changing the runtime.
- **v1 untyped:** No type annotations in the grammar.
- **Errors:** Context full (fatal or evict), tool not found (fatal), recall missing (prefer sentinel), undefined variable (fatal); all with clear messages.
- **Observability:** Failures should be reportable for debugging and tooling.
