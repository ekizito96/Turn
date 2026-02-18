# Turn: A Programming Language for Agentic Software

**Executive Summary for Stakeholders, Contributors, and Researchers**

---

## What is Turn?

Turn is a **new, object-oriented programming language** designed specifically for building agentic software. It is not a retrofit of Python, TypeScript, or any existing language. Turn makes the **agent** and its core abstractions—**context**, **memory**, **turns**, and **tool calls**—first-class primitives in the language itself.

### The Core Problem

Today's AI engineers build agents using languages designed for traditional software (functions, loops, lists). This creates an **impedance mismatch**: you think in "turns" and "context windows," but you code in "while loops" and "message lists." The result is:

- **500+ lines of boilerplate** per project (context management, tool dispatch, memory stores)
- **$500+/day costs** at scale due to unbounded context growth
- **Non-deterministic failures** that are hard to reproduce
- **Manual token budgeting** and context trimming
- **No standard semantics** for "one turn" or "one step"

Turn solves these problems by making agentic abstractions **native to the language**.

---

## Design Philosophy

### 1. Object-Oriented for Agents

The primary abstraction is the **Agent**—an object with:
- **Context** (a bounded buffer object)
- **Memory** (a persistent key-value object)
- **Turns** (units of execution)
- **Tool calls** (suspend/resume capability)

### 2. First Principles

Turn is built on deep science:
- **Physics of AI Engineering:** Finite attention, stochastic accumulation, entropic expansion
- **Formal semantics:** Operational semantics with deterministic transitions
- **Effect handlers:** Tool calls as effects with delimited continuations
- **Empirical validation:** Analyzed production agent codebases to validate design

### 3. Optimization Targets

| Dimension | Goal |
|-----------|------|
| **Tokens** | Bounded context by construction (|context| ≤ N) |
| **Performance** | Rust bytecode VM from day one (native speed) |
| **Determinism** | Reproducible execution (same inputs → same state transitions) |
| **Security** | Governance in scope (tool policies, audit trails) |
| **Boilerplate** | No hand-rolled agent loops |
| **Observability** | Standard trace format (turn, context, tools) |
| **Cost** | Token budget enforceable; minimal CPU/memory overhead |

---

## Language Features (v0.2.0)

### Primitives

```turn
turn {
  let name = infer Str { "What is your name?" }; // Native Intelligence
  spawn {                                        // Concurrency
      print("Thinking...");
  };
  context.append("Hello, " + name);              // Context: bounded buffer
  let result = call("echo", "Hello");            // Tool call: suspend/resume
  return result;
}
```

### Key Capabilities

1. **Native Intelligence**: `infer` keyword for direct LLM calls.
2. **Probabilistic Logic**: `confidence` operator and uncertainty propagation.
3. **Concurrency**: Actor model (`spawn`, `send`, `receive`).
4. **Vector Embeddings**: `vec[...]` literals and `~>` similarity.
5. **Turn**: Unit of execution and checkpointing.
6. **Context**: Bounded buffer (runtime-enforced max size).
7. **Memory**: Persistent store (`remember`, `recall`).
8. **Tool Call**: Suspension boundary for external effects.
9. **Control Flow**: `if`, `while`, `return`, `try/catch`.

### What Makes It Different

| Traditional Languages | Turn |
|----------------------|------|
| Context = manual list + trim | Context = bounded object (runtime-enforced) |
| Memory = hand-rolled caches + vector DBs | Memory = first-class primitive |
| Turn = while loop + async/await | Turn = language primitive |
| Tool call = fake suspension | Tool call = true suspension (VM pauses) |
| State = smeared across variables | State = single configuration |
| Semantics = implementation-defined | Semantics = formal spec |

---

## Implementation

### Architecture

**Rust Bytecode VM** (not Python/TypeScript):
- **Lexer** → **Parser** → **Compiler** → **Bytecode** → **VM**
- **Runtime**: Agent state (context, memory, env, turn_state)
- **Tools**: Registry with handlers

### Why Rust?

Turn's mandate is **fast, cost-efficient, minimal overhead**. Python/TypeScript add 10–100× overhead, contradicting these goals.

**Rust provides:**
- Native speed (10–100× faster than Python)
- Minimal memory (no GC pauses)
- Single binary (no runtime dependencies)
- True concurrency (no GIL)
- Fast serialization (cheap checkpointing)

### Performance

- **Target**: <1ms per turn (excluding tool calls)
- **Deployment**: Single static binary (~2–5MB)
- **Startup**: <10ms (vs 100–500ms for Python)

---

## Validation

### Empirical Evidence

Analyzed production agent codebases (integration agents, research agents):

**Found:**
- 500-line `ContextManager` class manually counting tokens, implementing priority stacks, slicing strings
- 3,400-line kernel file implementing the agent loop, state checkpointing, error handling
- Fake suspension with async/await (blocks logical flow)
- State smeared across Redis, global variables, and local dicts

**Turn's Solution:**
- Context primitive replaces entire ContextManager class
- Turn primitive replaces kernel loop
- True suspension (VM pauses, serializes state)
- Single configuration object

### Scientific Grounding

**10 deep problems identified** (with citations):
1. Context not first-class (Lost in the Middle, length-alone degradation)
2. Memory as infrastructure (STM/LTM, vector stores)
3. Turn and tool as hacks (effect handlers, AsyncLM)
4. State smeared (state drift, goal drift)
5. No shared semantics (operational semantics)
6. Wrong mental model (cognitive load, plan-based comprehension)
7. Observability ad-hoc (trace shape, replay)
8. Cost and budget invisible (token metering, death by accumulation)
9. Governance bolted on (AGENTSAFE, AgentSpec)
10. Computation power overhead (semantic + runtime costs)

**References:** 50+ academic papers, production postmortems, industry frameworks

---

## Project Status

### v1 Specification: Locked ✅

All core documents are stable and authoritative:
- Design mandate
- Minimal core primitives
- Grammar (BNF, lexer, operators)
- Runtime model (configuration, transitions)
- Type-friendly design
- Implementation strategy

### Implementation: In Progress 🚧

- ✅ Lexer, Parser, AST
- ✅ Compiler (AST → Bytecode)
- ✅ VM (Bytecode execution)
- ✅ Runtime (Agent state)
- ✅ Suspension/Resume
- ✅ Tool registry
- 🚧 Test suite completion
- 🚧 Standard library

### Running Turn

```bash
cd impl
./run.sh test    # Run tests
./run.sh hello   # Run hello_turn.turn
./run.sh build   # Build release binary
```

---

## Documentation Structure

### For Decision Makers
1. This document (PROJECT_SUMMARY.md)
2. [spec/00-design-mandate.md](spec/00-design-mandate.md) — Mission and goals
3. [research/00-problems-we-solve.md](research/00-problems-we-solve.md) — Deep science
4. [research/07-empirical-analysis.md](research/07-empirical-analysis.md) — Real-world validation

### For Developers
1. [DOCUMENTATION_INDEX.md](DOCUMENTATION_INDEX.md) — Complete navigation
2. [spec/04-hello-turn.md](spec/04-hello-turn.md) — First program
3. [spec/06-example-agent.md](spec/06-example-agent.md) — Realistic example
4. [spec/02-grammar.md](spec/02-grammar.md) — Syntax reference

### For Implementers
1. [spec/03-runtime-model.md](spec/03-runtime-model.md) — Formal semantics
2. [spec/07-implementation-strategy.md](spec/07-implementation-strategy.md) — Rust VM architecture
3. [impl/src/](impl/src/) — Reference implementation

### For Researchers
1. [research/00-problems-we-solve.md](research/00-problems-we-solve.md) — Problem space (50+ citations)
2. [research/07-empirical-analysis.md](research/07-empirical-analysis.md) — Empirical validation
3. [spec/03-runtime-model.md](spec/03-runtime-model.md) — Operational semantics
4. [research/06-critique-and-gaps.md](research/06-critique-and-gaps.md) — Critical analysis

---

## Why Turn Will Succeed

### Technical Excellence
- **Solves real problems** (validated by production codebases)
- **Grounded in science** (50+ academic papers, formal semantics)
- **Fast from day one** (Rust VM, native speed)
- **Deterministic** (reproducible execution, replay)

### Developer Experience
- **Speaks the problem** (turn, context, memory, tools)
- **Less boilerplate** (no hand-rolled loops)
- **Reason about it** (formal spec, standard trace)
- **Production-ready** (checkpoint, audit, governance)

### Market Fit
- **Universal problem** (every agent framework has these pains)
- **No competition** (no other agent-native language)
- **Growing market** (agentic software is exploding)
- **Clear value** (reduce cost, increase reliability)

---

## Roadmap

### v1.0 (Current)
- ✅ Spec locked
- 🚧 Rust VM implementation
- 🚧 Test suite
- 🚧 Documentation complete

### v1.1 (Next)
- Structured context (`context.pin`, `context.working`)
- Goal primitive
- Secure memory (secrets channel)
- Standard library (common tools)

### v2.0 (Future)
- Multi-agent primitives
- Agent as value
- Message/delegate operations
- Module system
- Type annotations

---

## Contributing

### Current Priorities
1. Complete test suite
2. Performance benchmarks
3. Standard library design
4. Documentation examples

### How to Contribute
1. Read [DOCUMENTATION_INDEX.md](DOCUMENTATION_INDEX.md)
2. Study the spec (`spec/`)
3. Review implementation (`impl/src/`)
4. Submit issues/PRs with justification

---

## Key Insights

### The "Context Manager Anomaly"
Production agents have 500-line classes just to manage text buffers. Turn's Context primitive replaces this entire class.

### The "God Class Kernel"
Production agents have 3,400-line files implementing the agent loop. Turn's turn primitive replaces this entire pattern.

### The "Universal Loop"
Turn's suspension/resume enables true durable agents: pause for days, serialize to disk, resume later. Impossible in Python without massive effort.

### The "Two-Layer Solution"
1. **Semantic reduction**: Bounded context, explicit turn, memory discipline
2. **Runtime reduction**: Rust VM = native speed, minimal overhead

Both layers are necessary. Solving semantic problems on a slow runtime contradicts the goals.

---

## Contact & Resources

- **Repository**: github.com/ekizito96/Turn
- **Documentation**: [DOCUMENTATION_INDEX.md](DOCUMENTATION_INDEX.md)
- **Research**: [research/](research/)
- **Spec**: [spec/](spec/)
- **Implementation**: [impl/](impl/)

---

## Final Thought

**Turn is not a "better Python for agents."** It is a **new language** built from first principles to solve the universal problems of agentic software. The execution model of agents (turns, bounded context, memory, suspend/resume) is fundamentally different from the execution model of traditional languages (functions, lists, single-call). Turn aligns the language with the problem.

**We're not retrofitting. We're building it right.**

---

**Last Updated**: 2026-02-18  
**Version**: 0.2.0  
**Status**: Core Implementation Complete (Phases 1-4)
