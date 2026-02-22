# Turn: A Systems Language for Agentic Computation

**Version:** 0.5.0 (Alpha)  
**Date:** February 23, 2026  
**Authors:** Muyukani Ephraim Kizito et al.

---

## Abstract

Turn is a compiled, statically-typed programming language designed for agentic software: systems that combine deterministic execution with probabilistic inference and long-lived state. Traditional languages force agentic systems into ad-hoc patterns for context management, suspension, and uncertainty, producing brittle control flow and inconsistent semantics.

Turn makes these constraints explicit by providing language primitives for: (1) suspension and resumption of execution, (2) cognitive type safety for `infer`, (3) first-class context and memory as managed resources, and (4) budgeted execution over token expenditure. This paper defines the physical constraints motivating Turn, specifies the computational model and operational semantics boundaries, and outlines a reference runtime architecture.

---

## 1. Motivation: Agentic Computation Is a Different Model

Agentic systems are not simply programs that call a model. They are long-running processes that:

- operate under **finite context capacity**,
- perform **stochastic inference**,
- interact with the world through **effects** (tools, human-in-the-loop, I/O),
- and require **durable state** across pauses, failures, and restarts.

In Turn, these properties are not a framework convention; they are language semantics.

---

## 2. The Physics of Agentic Engineering

Turn is grounded in measurable constraints of model-based computation.

### 2.1 Law of Finite Attention

**Statement:** Recall decays with distance from the decision boundary in a prompt. This yields a practical requirement: the runtime must control *where* invariants and constraints are placed.

### 2.2 Law of Stochastic Accumulation

**Statement:** Multi-step inference workflows compound error. If a workflow has per-step error rate \( \varepsilon \) and length \( n \), then:

\[
P(\text{success}) = (1-\varepsilon)^n
\]

**Implication:** Reliability requires validation, checkpoint boundaries, and policy hooks for drift.

### 2.3 Law of Entropic Expansion

**Statement:** Without active compression, context grows unbounded while capacity is fixed. Turn treats context quality as a measurable quantity and exposes policy triggers with configurable thresholds.

---

## 3. Computational Model

Turn programs execute in a VM with explicit suspension points. The runtime state is a configuration \(S\) that is serializable and replayable.

### 3.1 State Space

At any instant, a process state is:

\[
S = \langle pc, stack, frames, env, mailbox, context, memory, budgets \rangle
\]

### 3.2 Suspension Semantics (Effects)

Certain operations are *effects* and may suspend execution:

- `infer` (probabilistic inference),
- `call` (external effect),
- `suspend` (explicit checkpoint boundary).

The VM yields a typed suspension event containing:

- the tool/effect identifier,
- a canonicalized argument digest,
- and a continuation reference (serialized state).

### 3.3 Operational Boundary (Control Cycle)

Execution proceeds in control cycles: bounded instruction execution between suspension points. At each boundary, Turn enforces:

- **Deterministic replay** (given the same tool results),
- **atomic durable commit** of state mutations,
- **measured cost accounting** (execution steps and token budget deltas).

---

## 4. Language Primitives

Turn introduces primitives that encode the “physics” as semantics.

### 4.0 Capability-Oriented Objects

Turn is object-oriented in the sense required by agentic software: **encapsulation + capabilities + explicit effects**.

- **Value objects:** `struct` values model state with methods and clear invariants.
- **Process objects:** actor-style processes (PID + mailbox + durable state) are “live objects” that execute control cycles.
- **Contracts over hierarchies:** interfaces/traits and composition are preferred to inheritance-heavy designs because they preserve analyzability, determinism at boundaries, and replay.

### 4.1 `infer` (Native Intelligence) and Cognitive Type Safety

`infer` is an expression that requests a value of a declared type. The runtime enforces **cognitive type safety** by validating the returned value against the type schema and applying policy (retry, escalate, fail) on mismatch.

### 4.2 `suspend` (Orthogonal Persistence)

`suspend` forces a checkpoint boundary by serializing the current continuation into the Durable Heap, enabling long-lived agents that can pause and resume without re-executing the entire program.

### 4.3 Concurrency: Actor-Style Processes

Turn supports lightweight concurrent processes with isolated state and explicit message passing:

- `spawn`, `send`, `receive`

This provides a semantics for multi-agent decomposition without shared-memory races.

### 4.4 Context and Memory as Managed Resources

Turn exposes:

- bounded context mutation (`context.append(...)`),
- content-addressable memory (`remember`, `recall`),
- and a semantic addressing mechanism implemented as a pluggable ANN index inside the memory manager.

The language provides primitives and policy hooks; libraries define strategies (compression, retention, promotion).

### 4.5 Probabilistic Values: `confidence`

Values may carry confidence/provenance. `confidence(x)` extracts the confidence signal; operators propagate uncertainty through computation.

---

## 5. Runtime Architecture

The reference implementation compiles Turn source to bytecode and executes it on a VM with suspension/resumption.

### 5.1 Pipeline

Lexer → Parser → AST → Compiler → Bytecode → VM

### 5.2 The Provider-Agnostic Boundary (Solving Vendor Lock-In)

A critical architectural anti-pattern in agentic engineering is binding the application lifecycle to a specific foundation model provider (e.g., OpenAI, Anthropic). If a provider changes their API endpoint, deprecates a model, or alters their JSON Schema specification, traditional agent architectures break completely.

Turn explicitly prevents this by formalizing a **Language vs. Provider Boundary**:
- **The Turn AST** has zero knowledge of "OpenAI", "Tokens", or "REST endpoints". The language only understands the `infer` keyword, cognitive types (`Struct`, `List`), and `confidence`.
- **The Execution Layer (VM)** evaluates `infer` by emitting a generic `VmEvent::Suspend` Host Trap. It requests a capability fulfillment from the physical Host running the VM.
- **The Host Runtime** (e.g., `runner.rs` in Rust) intercepts the trap, maps Turn's canonical AST Type into the provider's specific proprietary format (e.g., OpenAI's structured outputs or Anthropic's tool use XML), executes the physical HTTP/gRPC request, and casts the payload back into a generic Turn `Value::Struct`.

This guarantees **Absolute Provider Agnosticism**. 
If a new LLM provider emerges tomorrow, or an old one introduces breaking API changes, **not a single line of Turn script needs to be rewritten**. Only the Rust Host executing the VM needs its HTTP adapter updated, keeping the ecosystem completely decoupled from vendor volatility.

### 5.3 Durable Heap

The runtime uses a durable state store for continuations and long-lived state. This enables:

- checkpointing at suspension boundaries,
- replay and time-travel debugging (reconstructing the state sequence),
- and durable mailboxes (messages are not lost across restarts).

### 5.3 Policy Hooks and Typed Observability

To avoid “framework creep,” policies are invoked at boundaries and operate through typed hooks and events (not implicit mutation). The runtime produces a deterministic trace digest for audit and replay.

---

## 6. Safety, Governance, and Non-Magic Semantics

Turn’s runtime avoids hidden behavior by construction:

- **Explicit bindings**: projecting `env` into `infer` or tool calls is allowlisted, typed, and traced.
- **Error lenses**: tool failures are represented as typed errors with bounded summaries and durable references to full payloads.
- **Drift monitors**: repetition/stalls are detected via deterministic trace digests and surfaced as typed events for policy handling.

---

## 7. Evaluation Methodology (What We Measure)

Turn is intended to be evaluated on:

- **Reliability**: schema mismatch rate, drift/stall rate, and recovery success rate.
- **Cost control**: token expenditure under budgets; prevention of runaway inference loops.
- **Determinism**: replay equivalence across runs given identical tool results.
- **Context quality**: retention of invariants under bounded windows; policy effectiveness under entropy growth.

---

## 8. Implementation Status (Alpha)

Turn includes a reference VM and tool/effect suspension mechanism. The implementation status for specific primitives and policies is tracked in `VISION.md` and `spec/`.

---

## 9. Roadmap

This whitepaper defines the model and semantics. The engineering roadmap and the “physics-first” design mandate are defined in:

- `VISION.md`
- `spec/` (formal grammar and runtime model)

---

## References

- `VISION.md` — unified vision and technical roadmap
- `spec/` — formal language specification (grammar, runtime model, types)
- “The Physics of AI Engineering” — cited in `VISION.md`
