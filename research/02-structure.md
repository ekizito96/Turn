# Research: Structure

What we’re unpacking: **modules**, **scoping** (lexical vs dynamic), **names and bindings**, **type systems** (or lack thereof), and how the best languages organize programs and data.

---

## Open questions

- [ ] Turn: Is there a “module” or “agent” boundary? How does context/memory scope to modules?
- [ ] Lexical vs dynamic scope for turn-local vs session-global state?
- [ ] Do we have a type system in v1, or start untyped like Scheme/Lua?
- [ ] How do tool and goal definitions relate to scope?

---

## 1. Scoping

### Lexical (static) scope

- Bindings are resolved by **where the code is written** (definition site). Environment is fixed at definition time.
- **Scheme:** Lambdas close over the environment of their definition. Enables closures, predictable reasoning.
- **Most modern languages:** Lexical by default (JavaScript, Python, etc.).

### Dynamic scope

- Bindings are resolved by **where the code is running** (call site). Environment changes with call stack.
- **Historical:** Some early Lisps; useful for things like “current context” or “current resource” without passing parameters.
- **For Turn:** “Current context” and “current turn” are dynamic notions. We may want **one** or **two** dynamic dimensions (e.g. “current context” as implicit) while keeping most bindings lexical.

**Takeaway:** Decide explicitly what is lexical (names, functions, tools?) and what is dynamic (context handle, turn id?, memory backend?).

---

## 2. What the best creators did

### Scheme (Sussman & Steele)

- **Lexical scoping** for all lambda; first-class procedures; **tail recursion** as the main iteration mechanism (constant space).
- **First-class continuations** — control is a value; enables backtracking, coroutines, non-local exits.
- **Few namespaces** — one main namespace for bindings; simplicity over feature richness.

**For Turn:** Tail recursion might matter for long agent runs (bounded stack). Continuations could relate to “pause turn and resume later,” but add complexity; document as optional or future.

### Alan Kay (Smalltalk)

- **Everything is an object.** Objects have local state; they **communicate only by messages**. No shared memory; no “data structures” in the traditional sense—only objects that respond to messages.
- **The object is in control** — e.g. `3 + 4` is “send message `+` with argument 4 to object 3.”

**For Turn:** Agentic “tools” and “memory” could be modeled as objects/messages: send “recall” to memory, “execute” to tool. We don’t have to go full Smalltalk, but message-passing as a metaphor fits agents.

### Wirth (Pascal, Modula, Oberon)

- **Modules** as the unit of encapsulation (Modula-2). Clear interfaces (export/import). No hidden globals.
- **Structured programming** — blocks, procedures; scope is block-scoped and explicit.

**For Turn:** If we have “agents” or “skills,” they could be module-like: explicit exports (what tools/memory this agent exposes), explicit imports (what context/memory it uses).

### Modern languages (Go, Rust, JavaScript)

- **Explicit modules** (packages, crates, ES modules). Dependency graph is visible.
- **Single or few entry points** per module; no implicit global mutable state by default.

**For Turn:** A Turn “program” might be one or more modules; each module might define turns, tools, and memory schemas. Avoid implicit global state; make context and memory explicit in the type or signature where possible.

---

## 3. Names and bindings

- **Binding:** association between a name and a value (or type). **Scope:** region of program text where the binding is visible.
- **Shadowing:** inner scope reuses a name; outer binding is hidden. Decide if we allow it and to what extent.
- **Namespaces:** one flat namespace vs hierarchical (e.g. `module.foo`). Turn might need at least: **terms** (values, functions), **tools**, **goals**, **memory slots** (or schemas).

---

## 4. Type systems

- **Untyped (dynamic):** Types attach to values; checked at runtime. Scheme, Python, Lua. Fast to prototype; errors show up at run time.
- **Statically typed:** Types checked at compile time. Rust, Go, Haskell. Catches many errors early; can encode invariants (e.g. “this is a tool call”).
- **Gradual typing:** Mix of typed and untyped. TypeScript, gradual typing in Scheme. Allows incremental adoption.

**For Turn (v1):** Open. Starting untyped keeps the research focus on syntax and semantics of turns; we can add a type system once the core is stable. If we add types, “tool”, “context”, “memory” could be distinct types or type constructors.

---

## 5. Modules and composition

- **Module:** unit of compilation and/or loading; has an interface (what it exports) and dependencies (what it imports).
- **For Turn:** Possible units: “agent” (set of turns + tools + memory interface), “library” (shared tools/memory types). How do turns in module A call tools defined in module B? Clear import/export rules avoid circularity and hidden coupling.

---

## 6. Lessons to apply

| Lesson | Application to Turn |
|--------|----------------------|
| Lexical scope by default (Scheme) | Predictable binding for tools, goals, and helpers. |
| Explicit dynamic “context” if needed | One or two well-defined dynamic parameters (e.g. current context, current turn). |
| Message-passing metaphor (Kay) | Tools and memory as “receivers of messages” can guide API design. |
| Modules with clear interfaces (Wirth, Go) | Agent/skill as module with explicit exports and imports. |
| Decide typing later | Start without static types; document “type-friendly” design so we can add them. |

---

## 7. Citations and further reading

- Scheme reports: lexical scoping, tail recursion, first-class continuations.
- Alan Kay: “Early History of Smalltalk”; message-passing only.
- Wirth: Modula-2/Oberon module system.
- Harper: *Practical Foundations for Programming Languages* (binding, scope, types).

*(Add exact citations and links as we go.)*
