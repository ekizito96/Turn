# Turn minimal core (v1)

**Status:** Locked for v1. Turn is **object-oriented**: the program defines the behavior of an **agent** (one instance in v1). The agent has a **context** object (bounded buffer) and a **memory** object (key-value store), and it executes **turns** and **calls tools**. This document defines the smallest set of primitives we implement first. Everything else is deferred to v1.1 or stdlib. The choice is justified: each primitive is necessary and not derivable from the others—see [00-design-mandate.md](00-design-mandate.md).

---

## 1. What is in the minimal core

| Primitive | Operations | Rationale |
|-----------|------------|-----------|
| **Turn** | One form: `turn { body }` | The unit of execution. Body is a block of statements; at the end we have either a value or a suspension (tool call). |
| **Context** | `context.append(expr)` ; bound enforced by runtime (max size N) | The agent's context **object**; append is the only mutator in v1. Bounded size is a runtime invariant; no `rewrite` or `window` in v1. |
| **Memory** | `remember(key, value)` ; `recall(key)` | The agent's memory **object** (key-value store). No `forget` or `summarize` in v1—we can add when needed. |
| **Tool** | `call(tool_name, args)` | Invocation on the agent's tool registry. Execution suspends; runtime runs handler; execution resumes with result. |

**Expressions and statements:** Enough to write real agents without friction.

- **Literals:** number, string, `true`, `false`, `null`.
- **Variables:** identifier (bound by `let`).
- **Operators:** `+` (concatenation/addition), `==`, `!=` (equality), `and`, `or` (logical, short-circuit).
- **Conditional:** `if expr block else block`.
- **Loop:** `while expr block`.
- **Block:** `{ stmt... }` with `let`, `return`, etc.
- **Let:** `let id = expr;` (local binding).

No list/map literals in v1; add when needed. `recall(key)` returns `null` when key is missing.

---

## 2. What is deferred (not in v1)

| Concept | Deferred to | Note |
|---------|-------------|------|
| **Goal** | v1.1 or stdlib | No `goal`, `set_goal`, `replan` in grammar or runtime. |
| **Context rewrite / window** | v1.1 or stdlib | Only append + runtime-bound. No `context.rewrite`, `context.window`. |
| **Memory forget / summarize** | v1.1 or stdlib | Only remember + recall. |
| **Modules** | v1.1 | Single program only. No import/export. |
| **Types** | v1 untyped | No type annotations in syntax. Type-friendly design documented in [05-types-and-errors.md](05-types-and-errors.md). |

---

## 3. Syntax surface (conventional, not S-expr)

We choose **conventional keyword/block syntax** (like Python/JS) for readability and one obvious way:

- `turn { ... }` for a turn.
- `context.append(expr);` for context append.
- `remember(key, value);` and `recall(key)` (statement and expression).
- `call(tool_name, args);` for tool call (statement; we get result when resumed).

So: **no S-expressions** in v1. The grammar (see [02-grammar.md](02-grammar.md)) is statement- and expression-based with keywords and blocks.

---

## 4. Single agent (v1)

- One **program** = the behavior of **one agent instance**. The program is a sequence of statements (including turns); each turn is one unit of execution for that agent.
- The agent has **one context object** and **one memory object** (provided by the runtime). No modules; no multi-agent in v1.
- The agent has a **tool registry** (provided by the runtime; default: at least one built-in tool, e.g. `echo`).

---

## 5. Summary

**v1 minimal core (OOP):** One agent with turn + context object (append, bounded) + memory object (remember, recall) + call(tool, args), with expressions (literals, `+` `==` `!=` `and` `or`), `if`, `while`, `let`, `return`. No goal, no context rewrite/window, no memory forget/summarize, no modules, no types in syntax. One agent instance, one context, one memory, one tool registry.
