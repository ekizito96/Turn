# Turn minimal core (v1)

**Status:** Locked for v1. This document defines the smallest set of primitives we implement first. Everything else is deferred to v1.1 or stdlib. The choice is justified: each primitive is necessary and not derivable from the others—see the "Why this minimal set" paragraph in [00-design-mandate.md](00-design-mandate.md).

---

## 1. What is in the minimal core

| Primitive | Operations | Rationale |
|-----------|------------|-----------|
| **Turn** | One form: `turn { body }` | The unit of execution. Body is a block of statements; at the end we have either a value or a suspension (tool call). |
| **Context** | `context.append(expr)` ; bound is enforced by runtime (max size N) | Append is the only mutator. Bounded context is a runtime invariant; we don't expose `rewrite` or `window` in v1—just append and a fixed max. |
| **Memory** | `remember(key, value)` ; `recall(key)` | Key-value only. No `forget` or `summarize` in v1—we can add when needed. |
| **Tool** | `call(tool_name, args)` | Single form for invocation. Tool is looked up in the runtime tool registry. Execution suspends; runtime runs handler; execution resumes with result. |

**Expressions and statements (minimal):** We need enough to write a turn body and pass values to append/remember/call.

- **Literals:** number, string (and optionally bool).
- **Variables:** identifier (bound by `let` or by parameter).
- **Application:** `name(args...)` for function/tool call; we reserve `call(name, args)` for the built-in tool call.
- **Conditional:** `if expr then block else block`.
- **Block:** `{ stmt... }` with optional `let id = expr;` and `return expr;`.
- **Let:** `let id = expr;` (local binding in block).

No loops, no list/map literals in v1—we can add when we need them for real examples.

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

## 4. Single program, single agent

- One **program** = one **turn** (or a sequence of turns, see grammar). No modules; no multi-agent in v1.
- One **context** and one **memory** per run (runtime provides them).
- **Tool registry** is provided by the runtime (default: at least one built-in tool, e.g. `echo`, so "hello turn" can run).

---

## 5. Summary

**v1 minimal core:** turn + context.append (bounded) + remember + recall + call(tool, args), with minimal expressions (literals, variables, application, if, block, let). No goal, no context rewrite/window, no memory forget/summarize, no modules, no types in syntax. One program, one context, one memory, one tool registry.
