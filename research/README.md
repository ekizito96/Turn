# Turn research index and plan

We are doing deep research so we don’t make mistakes. Turn is **object-oriented** (agent with context and memory as objects). This folder holds notes on **syntax**, **structure**, **algorithms**, and **foundational principles** from the best language creators, plus how they apply to Turn.

---

## Research plan (what we’re unpacking)

| Area | What we’re researching | Key questions |
|------|-------------------------|----------------|
| **Problems we solve** | Deep pain when using Python/TS/C for agentic software | What is the impedance mismatch? Context, memory, turn, state, semantics, cognitive load. |
| **Syntax** | Lexing, parsing, grammar, AST | How do we specify syntax? What did Wirth, McCarthy, Iverson do? BNF vs other formalisms? |
| **Structure** | Modules, scoping, types, names | How do we organize programs? Lexical vs dynamic scope? Type system or not? |
| **Algorithms** | Parsing, evaluation, compilation | Recursive descent vs table-driven? Interpreter vs compiler? Runtime representation of turns/context. |
| **Foundations** | Design principles, formal semantics | What did the best creators prioritize? Operational vs denotational semantics? How do we write down “what a turn means”? |
| **Turn-specific** | Agentic primitives | How do turn, memory, context, goal, tool map onto syntax, structure, and semantics? |
| **Critique and gaps** | Review as C/TS/Python creators | Appreciate, critique, consolidate gaps; order of work. |

---

## Index of research docs

| Doc | Topic | Status |
|-----|--------|--------|
| [00-problems-we-solve.md](00-problems-we-solve.md) | **Problems Turn solves:** deep pain using Python/TS/C for agentic software; impedance mismatch; context, memory, turn, state, semantics, cognitive load | Full |
| [01-syntax.md](01-syntax.md) | Syntax: grammar, lexing, parsing, AST; lessons from Wirth, McCarthy, Iverson, Lisp/Scheme | Seed content |
| [02-structure.md](02-structure.md) | Structure: modules, scoping, types, names; lessons from Scheme, Smalltalk, modern langs | Seed content |
| [03-algorithms.md](03-algorithms.md) | Algorithms: parsing, evaluation, compilation; front-end pipeline; references (Dragon Book, Crafting Interpreters) | Seed content |
| [04-foundations.md](04-foundations.md) | Foundations: principles from great creators; formal semantics (operational, denotational, axiomatic) | Seed content |
| [05-turn-primitives.md](05-turn-primitives.md) | Turn-specific: mapping agentic primitives to syntax/structure/semantics; open questions | Placeholder |
| [06-critique-and-gaps.md](06-critique-and-gaps.md) | **Critique and gaps:** doc-by-doc review; C / TypeScript / Python creator lens; consolidated gaps; recommended order of work | Full |

---

## How to use this

1. **Read** each doc; add citations, quotes, and “we should do X” notes.
2. **Cross-link** between docs when one decision (e.g. grammar choice) affects another (e.g. evaluation).
3. **Flag open questions** in the doc or as a short list at the top of each file.
4. **Don’t implement** until the research plan is satisfied enough to write a minimal spec in `spec/`.

---

## Key references (to expand)

- **Wirth** — “Design and implementation of Modula” / Oberon; simplicity, clarity, regular structure.
- **McCarthy** — “Recursive Functions of Symbolic Expressions”; S-expressions, minimalism.
- **Sussman & Steele** — Scheme reports; lexical scope, tail recursion, first-class procedures, “remove weaknesses.”
- **Kay** — “Early History of Smalltalk”; objects and messages only.
- **Iverson** — “A Programming Language” / APL; notation, uniformity, generality.
- **Dragon Book** — *Compilers: Principles, Techniques, and Tools*.
- **Crafting Interpreters** — Nystrom; practical front-end and interpreter.
- **Formal semantics** — Winskel (*The Formal Semantics of Programming Languages*); Harper (*Practical Foundations for Programming Languages*).
