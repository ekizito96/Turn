# Hello Turn — minimal program (v1)

**Status:** Reference program for readability and spec validation. Turn is object-oriented: this program is the behavior of **one agent** using its context object, memory object, and tool call. Uses only the v1 minimal core (grammar [02-grammar.md](02-grammar.md), runtime [03-runtime-model.md](03-runtime-model.md)).

---

## 1. The program

One turn in which the agent: binds a variable, writes to its memory, appends to its context, calls a tool (and uses the result), and returns.

```turn
turn {
  let name = "Turn";
  remember("user", name);
  context.append("Hello, " + name);
  let out = call("echo", "Hello");
  return out;
}
```

---

## 2. Expected behavior

1. **Enter turn:** Runtime sets turn_state (e.g. turn_id = 1); program is the block body.
2. **let name = "Turn":** env becomes `{ name → "Turn" }`.
3. **remember("user", name):** memory becomes `{ "user" → "Turn" }`.
4. **context.append("Hello, " + name):** Evaluates `"Hello, " + name` → `"Hello, Turn"`; context buffer gets one entry, e.g. `["Hello, Turn"]`.
5. **let out = call("echo", "Hello"):** Execution suspends with `Suspension("echo", "Hello", continuation)`. Runtime runs the `echo` handler (e.g. returns `"Hello"`). Runtime resumes; continuation runs with result; `out` is bound to `"Hello"`.
6. **return out:** Turn completes with value `"Hello"`.

**Final state (conceptual):**

- env: had `name`, `out` during the turn; after turn, scope is gone (block exited).
- context: `["Hello, Turn"]`.
- memory: `{ "user" → "Turn" }`.
- Turn result: `"Hello"`.

---

## 3. Variant: use recall

To show memory in the same program, we can return what we stored:

```turn
turn {
  let name = "Turn";
  remember("user", name);
  context.append("Hello");
  call("echo", "Hello");        // statement: call, discard result
  let who = recall("user");
  return who;
}
```

Here the turn result is `"Turn"` (the value we remembered). So we have two reference programs: one returns the **tool result**, one returns a **recalled** value.

---

## 4. Trace (small-step, abbreviated)

| Step | Action | env | context | memory |
|------|--------|-----|---------|--------|
| 0 | enter turn | {} | [] | {} |
| 1 | let name = "Turn" | {name→"Turn"} | [] | {} |
| 2 | remember("user", name) | ... | [] | {"user"→"Turn"} |
| 3 | context.append("Hello, " + name) | ... | ["Hello, Turn"] | ... |
| 4 | suspend call("echo", "Hello") | ... | ... | ... |
| 5 | resume with "Hello", let out = result | {..., out→"Hello"} | ["Hello, Turn"] | ... |
| 6 | return out | — | ["Hello, Turn"] | {"user"→"Turn"} |

Turn result: `"Hello"`.

---

## 5. Purpose

- **Readability:** Minimal program that shows one agent doing one turn: using its memory, context, and tools.
- **Spec target:** Implementations and tests can use this program to validate parsing, runtime (agent state, suspension/resumption), and final state.
