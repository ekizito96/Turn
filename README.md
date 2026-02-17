# Turn

**A new, object-oriented programming language for agentic software.** Turn is not a retrofit. The primary abstraction is the **agent**—an object with context, memory, and the ability to run turns and call tools. We solve the pains of building agentic systems in Python, TypeScript, or C and optimize for minimal tokens, performance, security, less boilerplate, and clear observability.

## Status

**Spec locked for v1 minimal core.** Design mandate and primitives are fixed. **Implementation:** Rust bytecode VM from day one (see [spec/07-implementation-strategy.md](spec/07-implementation-strategy.md)). Not Python/TypeScript—those languages' overhead contradicts Turn's goals (fast, cost-efficient).

## Design mandate

Mission, design goals, and first-principles justification: [spec/00-design-mandate.md](spec/00-design-mandate.md).

High-level design and rationale: [Prescott-Data-Applications/ai-engineering-blog/docs/agentic-programming-language-design.md](../Prescott-Data-Applications/ai-engineering-blog/docs/agentic-programming-language-design.md)

## Project layout

```
Turn/
├── README.md           # This file
├── research/           # Research notes and references
│   ├── README.md       # Research index and plan
│   ├── 01-syntax.md
│   ├── 02-structure.md
│   ├── 03-algorithms.md
│   ├── 04-foundations.md
│   └── 05-turn-primitives.md
├── spec/               # 00-design-mandate, 01-minimal-core, 02-grammar, 03-runtime-model, 04-hello-turn, 05-types-and-errors
└── impl/               # (Later) Interpreter / compiler
```

## Research first

Before writing grammar or code, we are:

0. **Problems we solve** — Deep pain when using Python, TypeScript, C, etc. for agentic software: context, memory, turn/tool as hacks, state smeared, no shared semantics, wrong mental model. Science and references in [research/00-problems-we-solve.md](research/00-problems-we-solve.md).
1. **Syntax** — What great designers did (BNF, lexing, parsing, AST). Lessons from Wirth, McCarthy, Iverson.
2. **Structure** — Modules, scoping, types. Lessons from Scheme, Smalltalk, modern languages.
3. **Algorithms** — Parsing, evaluation, compilation. Front-end and runtime pipeline.
4. **Foundations** — Principles from the best creators; formal semantics (operational, denotational, axiomatic).
5. **Turn-specific** — How agentic primitives (turn, memory, context, goal, tool) map onto these foundations.

See [research/README.md](research/README.md) for the full research plan and index.
