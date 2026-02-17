# Research: Turn-specific primitives

How Turn’s **agentic primitives** (turn, memory, context, goal, tool) map onto **syntax**, **structure**, and **semantics**. This doc collects open questions and design options; it will be updated as we lock decisions.

---

## Open questions

- [ ] Exact syntax for “one turn” (keyword? block? form?)
- [ ] Is context a value or an implicit stack? How is it scoped?
- [ ] Memory: key-value only, or also semantic/vector recall? Schema in the language?
- [ ] Tool: built-in form vs standard library? How do we represent “tool call in progress” (suspension)?
- [ ] Goal: first-class type? How does “replan” interact with the grammar?

---

## 1. Turn (the unit of execution)

- **Idea:** One “turn” = perceive (inputs, tool results, context) → decide → act (message, tool call, memory op).
- **Syntax options:** `turn { ... }`, `step { ... }`, or a form like `(turn body)`. Could have explicit “inputs” and “outputs” or leave them implicit from context.
- **Structure:** Turn body is a scope? Can it contain multiple statements and one “result” or “action”?
- **Semantics:** Operational rule: run body with current context and memory; body may read context, call memory ops, call tools (and suspend); at end of body we get new context and possibly a tool-call suspension. Need to write this as a transition rule.

**References:** Design doc “turn as unit of execution”; research [01-syntax](01-syntax.md), [03-algorithms](03-algorithms.md), [04-foundations](04-foundations.md).

---

## 2. Memory

- **Idea:** Long-term and short-term; operations: remember, recall, forget, maybe summarize.
- **Syntax options:** `remember(key, value)`, `recall(query)`, `forget(selector)`, `summarize(scope)`. Or methods on a `memory` object if we have OO-style.
- **Structure:** Is memory a global singleton or passed explicitly? Per-agent or shared? Typed (e.g. “memory of type Facts”) or untyped?
- **Semantics:** At least: `remember` updates store; `recall` returns value(s) or error; `forget` removes; `summarize` returns a summary (implementation may use an LLM or heuristic). Specify ordering (e.g. same-turn reads see same-turn writes).

**References:** Design doc “memory as first-class”; agentic memory literature (MEM1, semantic anchoring).

---

## 3. Context

- **Idea:** Bounded, evolving buffer (e.g. conversation, recent state). Can be appended, rewritten (summarized), or windowed.
- **Syntax options:** `context.append(x)`, `context.rewrite(strategy)`, `context.window(n)`, or `context` as an expression that yields current context.
- **Structure:** Context is dynamic (current session)? Or a value we pass? Scope: one per session, one per agent, or one per turn?
- **Semantics:** Define “context value” (e.g. list of messages or state entries); define append, rewrite, window; define max size and eviction (e.g. drop oldest or summarize).

**References:** Design doc “context as bounded mutable object”; ACE, context collapse.

---

## 4. Goal

- **Idea:** Goals are first-class; plans are generated, executed, revised.
- **Syntax options:** `goal(name, description)`, `set_goal(g)`, `check_goal(g)`, `replan()`. Or goals as data and a standard library for planning.
- **Structure:** Goal type? Stored in context or in a separate goal stack/store?
- **Semantics:** At least: “current goal” is part of runtime state; “replan” might trigger a turn that produces a new plan. Detailed semantics (e.g. how plans are represented) TBD.

**References:** Design doc “goals and plans in the type system or runtime”; BDI (desire, intention).

---

## 5. Tool

- **Idea:** Tools are callable with a name + arguments; execution may be external; result comes back and execution resumes.
- **Syntax options:** `tool(name, args)`, `call(tool_name, args)`, or `name(args)` if tools are in a dedicated namespace. Tool definition: `define_tool name(params) { ... }` or external registry.
- **Structure:** Tool registry in runtime; handlers can be in-language functions or external (HTTP, subprocess). Schema (input/output types) for validation and safety.
- **Semantics:** “Tool call” is a special kind of suspension: state is saved, handler is invoked, when result arrives we resume with the result. Need to specify whether tool calls are synchronous (blocking) or asynchronous (promise/future) in the language model.

**References:** Design doc “tools as standard interface”; [03-algorithms](03-algorithms.md) runtime model.

---

## 6. Cross-cutting

- **Governance/safety:** Policies (what tools, what memory writes) might be enforced in the runtime. Syntax could expose “allow”, “require_approval”, etc. Research: where do policies live (in language, in runtime config)?
- **Composition with ordinary code:** Expressions and control flow (if, loop, let) inside a turn. No special syntax for “just computation” unless we want a distinct `pure` or `expr` block.

---

## 7. Next steps

1. Lock **syntax** for at least one primitive (e.g. `turn` and one memory op) in [01-syntax.md](01-syntax.md).
2. Lock **runtime representation** (context, memory, turn state) in [03-algorithms](03-algorithms.md).
3. Write **one** operational rule for “execute one turn” in [04-foundations](04-foundations.md) or in a new `spec/semantics.md`.
4. Iterate: add one primitive at a time, with grammar + semantics + tests.

*(This doc will be updated as we make decisions.)*
