# Turn: The Language for the Age of Agentic Intelligence

> "We are not building a framework. We are building a physics engine for cognition."

**Version:** 0.4.0 (Alpha)  
**Date:** February 18, 2026  
**Status:** Alpha — Cognitive Type Safety Implemented

---

## 1. The Manifesto

We are witnessing a shift in computing. We are moving from **Deterministic Computing** (logic) to **Probabilistic Computing** (cognition).

Today, developers build "Agents" using Python or TypeScript—languages designed for deterministic systems. They rely on heavy frameworks (LangChain, AutoGen) to patch the gaps, resulting in "Gluestick Spaghetti": brittle, verbose code that spends 50% of its lines managing context strings and retrying failed JSON parses.

**Turn** is the answer. It is a compiled, statically-typed, concurrent programming language designed explicitly for **Agentic Software**. It treats LLMs not as external APIs, but as a native computational unit (`infer`). It treats Context not as a string buffer, but as a managed memory resource.

We are building Turn to be the **Systems Language for AI**—a language whose primitives match the physics of agentic computation.

---

## 2. The Prime Directive: Language, Not Library

To ensure Turn remains a powerful **Systems Language** and does not degrade into "Interpreter Rust" or a "Python Framework," we adhere to this directive:

> **"If it describes the *Physics of Cognition*, it must be a Keyword/Opcode. If it describes a *Task*, it is a Library."**

| Feature | Wrong Approach (Library) | Right Approach (Turn Language) |
| :--- | :--- | :--- |
| **Persistence** | `db.save(state)` | `suspend;` (Opcode 0xA1) |
| **LLM Call** | `client.chat.completions(...)` | `infer Struct { ... }` (Opcode 0xB2) |
| **Context** | `ctx = ContextManager()` | `context.append(...)` (Opcode 0xC3) |
| **Agents** | `class MyAgent(AgentBase):` | `spawn turn() { ... }` (Opcode 0xD4) |
| **Uncertainty** | `if result.score > 0.8:` | `confidence(result)` (Opcode 0xE5) |
| **Memory** | `store.embedding(x)` | `remember x;` (Opcode 0xF6) |

We do not build "Classes" to solve physics problems. We build **Instructions**.

### The Boundary: Language vs. Library

To prevent "framework creep," here is what Turn explicitly does NOT include:

| Feature | Classification | Reason |
| :--- | :--- | :--- |
| **Knowledge Graphs** | Library | Data structure built using `remember`/`recall` |
| **RAG Pipelines** | Library | Chunking strategy, not memory primitive |
| **Prompt Templates** | Library | Application logic, not opcode |
| **Tool Schemas** | Library | External API contracts, not language types |

**The Principle:**  
Turn provides **memory primitives** (addressing, persistence, allocation). Developers build **data structures** (graphs, pipelines, caches) using those primitives.

### The Doctrine: Capability OOP (Without Framework Gravity)

Turn is **object-oriented by capability and encapsulation**, not by inheritance hierarchies.

*   **Objects are capability-bearing values:** A value is an object if it carries state and exposes methods that operate on that state.
*   **Composition over inheritance:** Turn prioritizes interfaces/traits (behavioral contracts) and composition. Inheritance-heavy design is avoided because it increases implicit coupling and makes agent state harder to reason about and replay.
*   **Two kinds of objects:**
    1.  **Value objects:** `struct` values with fields and methods (domain modeling, policies, plans).
    2.  **Process objects:** actor-style processes with identity (PID), mailbox, and durable state. These are “live objects” that execute control cycles and interact via messages and effects.
*   **Effects stay explicit:** `infer`, `call`, and `suspend` are effect boundaries. This keeps nondeterminism visible and preserves deterministic replay.

---

## 3. The Physics of AI Engineering

Turn's design is grounded in the measurable constraints of Large Language Models. These are not design preferences—they are **physical laws** derived from the mathematics of transformer attention and empirical observations from production deployments.

### Law 1: The Law of Finite Attention
**Statement:** An LLM's ability to recall information decays exponentially as a function of positional distance from the decision boundary.

**Mathematical Form:**
```
Recall(pos) = exp(-λ * |pos - boundary|)
```
Where λ is an empirically measured attention-decay constant for a given model and prompting regime (λ is not universal; it is a runtime-calibrated parameter).

**Implication:** Critical data (mission, invariants, constraints) must be placed at the start (0–10%) or end (85–100%) of context, never in the middle (40–60%) where recall drops to 50%.

**Turn's Response:** The Runtime implements a **Priority Stack Architecture** (P0/P1/P2 tiers), automatically structuring context to respect the U-shaped attention curve.

### Law 2: The Law of Stochastic Accumulation
**Statement:** In a multi-step reasoning chain, errors compound exponentially.

**Mathematical Form:**
```
P(success) = (1 - ε)^n
```
Where ε is the per-step error rate and n is the number of steps. With ε=0.02 and n=50, P(success) = 0.364 (63.6% failure rate).

**Implication:** You cannot build reliable systems on probabilistic chains alone. Checkpointing and validation are mandatory.

**Turn's Response:** 
- The `suspend` opcode creates automatic checkpoints after every control cycle.
- The `infer` opcode validates types before returning, catching hallucinations at the boundary.

### Law 3: The Law of Entropic Expansion
**Statement:** Without intervention, context grows linearly with time while capacity is constant. Eventually, overflow occurs.

**Mathematical Form:**
```
Entropy(t) = len(logs(t)) / len(context_capacity)
```
When Entropy > \( \theta \), signal-to-noise ratio becomes critical (with \( \theta \) configurable; default \( \theta = 0.7 \)).

**Implication:** Context must be actively compressed. Naive FIFO eviction loses critical information.

**Turn's Response:** The Runtime implements **Semantic Compression** and **Entropy Monitoring**, automatically summarizing old turns when entropy exceeds a configurable threshold \( \theta \) (default \( \theta = 0.7 \)).

---

## 4. The Four Pillars of Turn

To build robust, production-grade agentic software, Turn relies on four non-negotiable pillars:

### I. Cognitive Type Safety
LLMs are inherently untyped and hallucinate. Python treats their output as `Any` or `dict`.
Turn enforces **Cognitive Type Safety**.
*   **Opcode**: `infer Struct`.
*   **Mechanism**: The Runtime (not the user) negotiates with the LLM to ensure outputs strictly match defined schemas. If the LLM fails, the Runtime catches it, retries, or raises a typed error.
*   **Result**: Zero "JSON parse errors" in user code.

### II. Sovereign Concurrency
Agents are not callbacks. They are autonomous entities that run for days, wait for emails, and negotiate with other agents.
Turn adopts the **Actor Model** (inspired by Erlang/Elixir).
*   **Opcodes**: `spawn`, `send`, `receive`.
*   **Mechanism**: Agents are lightweight processes (Green Threads) with isolated memory and mailboxes. They share nothing. Each process executes a **Sovereign Control Cycle** (a feedback loop over state and action).
*   **Result**: A crash in one agent (e.g., "Analyst") never brings down the system (e.g., "Manager").

### III. First-Class Context
In Python, "context window" is an abstraction leak. You manually slice strings.
In Turn, **Context is Memory**.
*   **Opcode**: `context.append(...)`.
*   **Mechanism**: The Runtime manages the context window like a heap. It automatically handles eviction, summarization, and prioritization based on the Physics of Finite Attention (Priority Stack: P0/P1/P2).
*   **Result**: No more "context length exceeded" errors.

### IV. Probabilistic Control Flow
Traditional `if` statements are binary. Agent decisions are fuzzy.
Turn introduces **Confidence** as a language primitive.
*   **Opcode**: `confidence(val)`.
*   **Mechanism**: Every value carries a provenance and confidence score (0.0–1.0). The Runtime tracks uncertainty propagation through the call stack.
*   **Result**: Logic like `if confidence(plan) < 0.8 { ask_human() }` becomes trivial.

---

## 5. The Architecture: Three Subsystems

Turn's runtime is composed of three tightly integrated subsystems, each solving a fundamental constraint of agentic computing:

### 5.1. The Sovereign Runtime (Solving "The Loop")

**The Problem:** In Python, an agent is a `while(true)` loop. If the server restarts, the loop dies.

**The Turn Solution:** **The Sovereign Control Cycle.**
The Turn Runtime does not just "run code". It executes a **Sovereign Control Cycle** (a closed-loop feedback system) for every process:
1.  **Sense:** Read mailbox (`receive`) and durable state (`remember`/`recall`).
2.  **Update:** Update internal belief state and `confidence` scores.
3.  **Select:** Select the next opcode or `infer` call (policy selection).
4.  **Commit:** Apply state transitions (including persistence) and execute the instruction.

This cycle is **Orthogonally Persistent**. The VM implements a **Durable Heap**. Persistence is a property of the language memory model.

#### 5.1.0. Control Cycle Semantics (Operational Boundary)

The Sovereign Control Cycle is not a metaphor. It defines the VM's **operational semantics boundary** for correctness, persistence, and measurement.

*   **State Space:** At any instant, a process state is \(S = \langle pc, stack, frames, env, mailbox, context, memory, budgets \rangle\).
*   **Cycle Step:** One control cycle is a transition \(S_t \rightarrow S_{t+1}\) produced by executing a bounded number of instructions and at most one external suspension point (`infer`, `call`, `suspend`).

**Cycle Invariants (must hold at every boundary):**
*   **Deterministic Replay:** Given the same initial \(S_0\) and the same sequence of external tool results, the VM produces the same sequence of states.
*   **Persistent Commit:** All mutations to durable state (heap, mailboxes, semantic index metadata, trace) are committed as an atomic unit at the boundary. If a crash occurs mid-cycle, the VM must resume from the last committed boundary without partial effects.
*   **Measured Cost:** Every cycle emits a deterministic accounting record (execution steps, token budget deltas, tool latency if available). Budgets can halt or suspend execution by policy.

**Boundary Events (typed, auditable):**
*   **`ToolSuspended{ tool, args_digest, continuation_ref }`**: Execution yields with a serialized continuation.
*   **`StateCommitted{ root_hash, trace_digest }`**: Durable Heap commit finalized with a stable identifier.
*   **`DriftDetected{ kind, evidence_ref }`**: Cycle-level monitor detected repetition/stall/convergence failure.

**Policy Hooks (language-level control points):**
*   Policies do not mutate state implicitly. They may only:
    - request compression/promotion actions,
    - change rendering priorities for context,
    - adjust scheduling/budget parameters,
    - decide whether to `suspend`, retry, escalate, or terminate.
*   Policies are invoked only at cycle boundaries, never mid-instruction, preserving determinism.

#### 5.1.1. The Log-Structured Heap
*   **Requirement:** The VM's allocator must treat the Durable Heap as the source of truth, with in-memory state as a cache.
*   **Implementation:**
    *   **Page-Based Allocator:** VM objects are mapped to 4KB pages on disk.
    *   **Write-Ahead Log (WAL):** Every variable assignment (`let x = ...`) is an atomic append to the VM's journal.
    *   **Merkle State Tree:** The VM state is a Merkle Tree. We can "time travel" to any previous state hash.
    *   **Durable Mailboxes:** Messages sent to a PID (`send(pid, msg)`) are written to disk *before* delivery is acked.

#### 5.1.2. The `suspend` Opcode ✅ (Implemented)
*   **Requirement:** A VM instruction to yield execution until an external signal, costing 0 execution while waiting.
*   **Status:** Implemented in v0.4.0.
    *   [x] **`suspend` Keyword:** Serializes the current Stack Frame and Instruction Pointer to the Durable Heap and halts the OS thread.
    *   [ ] **Interrupt Handling:** A mechanism to wake the frozen VM via external IO (webhook/timer).

#### 5.1.3. The Priority Stack (Law of Finite Attention)
*   **Requirement:** LLMs have U-shaped attention. Critical info must be at the edges.
*   **Implementation:**
    *   **Structured Context Object:**
        ```rust
        struct Context {
            system_prompt: String, // P0 (Top 0-10%)
            mission: String,       // P0 (Top 0-10%)
            history: Vec<Turn>,    // P2 (Middle - Compressed)
            working_mem: String,   // P1 (Bottom 85-100%)
        }
        ```
    *   **Render Strategy:** When calling `infer`, the Runtime assembles the prompt by placing P0/P1 items at the boundaries and compressing P2 items in the middle.

#### 5.1.4. Supervisor Trees (Fault Tolerance)
*   **Requirement:** If a worker crashes, a supervisor must restart it.
*   **Implementation:**
    *   **`link` Keyword:** Bidirectional lifecycle binding between processes.
    *   **`monitor` Keyword:** Unidirectional death notification.
    *   **Restart Strategies:** One-for-one, one-for-all, rest-for-one.

---

### 5.2. The Memory Hierarchy (Solving "Amnesia")

**The Problem:** "Agents keep forgetting."

**The Turn Solution:** **Content-Addressable Memory (CAM).**
Turn supports content-addressable memory: values can be retrieved by semantic similarity (`recall "concept"`).

#### 5.2.1. Native Semantic Pointers (The Addressing Primitive)
*   **Requirement:** The runtime must support "Semantic Addressing" as a native addressing mode.
*   **What Turn Provides (Language Primitive):**
    *   **`remember` Opcode:** Stores a `Value` and its semantic hash (embedding) in the VM's Durable Heap.
        ```turn
        remember x; // VM computes a semantic representation, inserts into ANN index
        ```
    *   **`recall` Opcode:** Retrieves the nearest `Value` by semantic similarity.
        ```turn
        let y = recall "find similar bugs"; // ANN lookup (mechanism is pluggable)
        ```
    *   **The ANN Index:** An approximate nearest-neighbor index maintained *inside the VM's memory manager*. This is an **addressing mechanism**; the concrete structure is replaceable (e.g., graph-based indices).

*   **What Developers Build (Libraries):**
    Developers use `remember`/`recall` to build higher-level data structures (graphs, caches, catalogs). The language provides the primitive; libraries provide the data structures.

*   **Critical Distinction:** Turn does NOT ship with "RAG pipelines" or "knowledge graphs". Those are libraries or applications built using `remember`/`recall`.

#### 5.2.2. Automatic Context Tiering (Law of Entropic Expansion)
*   **Requirement:** The `context` window is finite. Old turns shouldn't be dropped; they should be compressed.
*   **Physics:** The VM maintains a three-tier memory hierarchy:
    *   **Tier 1 (Working Context):** The actively rendered context passed into `infer`. Capacity is bounded by the model context window.
    *   **Tier 2 (Episodic Summaries):** Compressed representations of past turns produced by a policy hook (often via `infer`). Summaries are stored in the Durable Heap.
    *   **Tier 3 (Archival Memory):** Raw historical turns stored in the Durable Heap and indexed by the semantic addressing subsystem (ANN index). Storage is bounded by available durable storage.
    *   **Auto-Recall (Language Feature):** Before every `infer`, the Runtime queries semantic memory for relevant past values and promotes them into Tier 1.
    *   **Entropy Monitoring (VM Diagnostic):** The Runtime exposes a deterministic context-quality metric and a configurable policy trigger (default \( \theta = 0.7 \)).

*   **What the VM Provides:**
    - Memory tiers (Tier 1/2/3 addressing)
    - ANN index (semantic lookup)
    - Policy hooks (e.g., `on_context_full`)
    - Diagnostics (e.g., `context_entropy()`)

*   **What the Developer Provides:**
    - Compression policy (what to compress, how to compress)
    - Retention policy (what to keep, what to evict)
    - Promotion policy (when and how to promote archived values into the active context)

The VM provides tiering, metrics, and hooks; policies decide what to compress and when.

#### 5.2.3. Cognitive Offloading (Law of Stochastic Accumulation)
*   **Requirement:** LLMs hallucinate when doing too much. The Runtime must reduce cognitive load.
*   **Implementation:**
    *   **Explicit Bindings (No Magic Injection):** The Runtime supports explicit, typed bindings from the current environment into `infer` and tool calls. This is deterministic and auditable.
        - The default rule is **deny-by-default**; only allowlisted bindings may be projected.
        - Projection is recorded in the trace and stored in the Durable Heap.
    *   **Error Normalization (Error Lenses):** Tool failures are represented as a typed `Error` value with:
        - `kind` (stable enum)
        - `summary` (bounded string)
        - `digest` (content hash)
        - `details_ref` (pointer into Durable Heap for full payload)
        The LLM sees the summary; the system retains full fidelity via `details_ref`.
    *   **Loop / Drift Monitors (Policy Hooks, Not Prompt Hashes):** The Runtime maintains a deterministic trace digest over actions (opcodes, tool names, canonicalized args). When cycles or stalls are detected, it emits a typed event (`LoopDetected`, `ConvergenceStall`) and triggers a policy hook. Policies may escalate, pivot strategy, or suspend—without injecting ad-hoc natural language as a control mechanism.

---


---

### 5.3. Token Economics (Solving "The Gas Problem")

**The Problem:** A `while(true)` loop in Python costs electricity. A `while(true)` loop in an Agent costs **Money** (Tokens). Infinite loops bankrupt companies.

**The Turn Solution:** **The "Gas" Model.**
Computation is measured in "Credits," not just time.

#### 5.3.1. The Token Scheduler
*   **Requirement:** The Turn scheduler manages Token Budgets (a finite resource distinct from execution steps).
*   **Implementation:**
    *   **Budgeted Process:**
        ```turn
        spawn(budget: 1.00) turn() { ... }; // Agent dies if it spends > $1.00
        ```
    *   **Cost Tracking:** The Runtime tracks token usage per PID.
    *   **Rate Limiting:** "This agent can only make 5 LLM calls per minute."

#### 5.3.2. Speculative Execution
*   **Requirement:** LLMs are slow. Parallelize inference.
*   **Implementation:**
    *   **Parallel Inference:**
        ```turn
        let a = spawn { infer ... };
        let b = spawn { infer ... };
        let result = await(a) + await(b);
        ```
    *   **Model Cascading:** The `infer` keyword accepts a "strategy."
        *   *Strategy*: Try `gpt-4o-mini` (cheap). If `confidence < 0.8`, automatically retry with `gpt-4o` (expensive). This is handled by the VM, not user `if/else` blocks.

---

## 6. The Engineering Principles

These principles guide every architectural decision in Turn's development:

1.  **Physics First**: Every architectural decision must respect the measurable constraints of the model—attention decay, token limits, stochastic drift.
2.  **Measure, Don't Guess**: Use tiktoken to count tokens, profile attention patterns, instrument error rates, and design based on data rather than intuition.
3.  **Determinism at Boundaries**: Use probabilistic AI for reasoning (`infer`) but deterministic code for execution (`call`), with validated interfaces between them.
4.  **State is Sacred**: Persist state after every control cycle; checkpointing is not optional because the agent must survive process death.
5.  **Entropy is the Enemy**: Actively manage context growth through early and frequent compression, never allowing noise to drown signal.
6.  **Cognitive Offloading**: Free the LLM from plumbing by letting it reason about *what* to do while code handles *how* to do it.
7.  **Validate Everything**: Never trust LLM outputs blindly—validate parameters, check schemas, and verify logic before execution.
8.  **Design for Failure**: Accept that errors will happen and build retry logic, error recovery, and escalation paths that assume failure and plan for resilience.

---

## 7. The Roadmap to v1.0

We are currently at **Phase 1 (Alpha)**. Here is the engineering path to a production-ready, public v1.0.

### Phase 1: The Spark ✅ (Completed)
*Goal: Prove the core physics work.*
- [x] **VM & Bytecode**: Custom Rust-based stack machine.
- [x] **Basic Concurrency**: `spawn`, `send`, `receive` implemented.
- [x] **Cognitive Typing**: `infer Struct` implemented and working (v0.4.0).
- [x] **LLM Integration**: Model-agnostic provider dispatch.
- [x] **Suspend Primitive**: `suspend;` opcode for orthogonal persistence.

---

### Phase 2: The Sovereign Runtime (In Progress)
*Goal: Make it crash-proof and persistent.*
- [ ] **Log-Structured Heap**: Page-Based Allocator, WAL, Merkle State Tree, Durable Mailboxes.
- [ ] **Priority Stack**: Structured Context Object (P0/P1/P2), Render Strategy.
- [ ] **Supervisor Trees**: `link`, `monitor`, Restart Strategies.
- [ ] **Interrupt Handling**: Wake suspended VMs via external IO.

---

### Phase 3: The Memory Hierarchy (Semantic RAM)
*Goal: Solve "Agents keep forgetting."*
- [ ] **Content-Addressable Memory**: ANN index inside VM (pluggable structure), `remember`/`recall` opcodes.
- [ ] **Automatic Context Tiering**: Tier 1/2/3 (Working/Episodic/Archival), Auto-Recall, Entropy Monitoring.
- [ ] **Cognitive Offloading**: Parameter Injection, Sanitized Errors, Cognitive Injection.

---

### Phase 4: Token Economics (The Gas Model)
*Goal: Treat tokens as a finite resource with enforceable budgets.*
- [ ] **Token Scheduler**: Budgeted Process, Cost Tracking, Rate Limiting.
- [ ] **Speculative Execution**: Parallel Inference, Model Cascading.

---

### Phase 5: Developer Experience (DX)
*Goal: Make it joyful to write.*
- [ ] **Language Server Protocol (LSP)**: VS Code extension with syntax highlighting, go-to-definition, and auto-complete.
- [ ] **Time-Travel Debugger**: Since Turn is deterministic (replayable), we can build a debugger that steps *backwards* through an agent's control cycle.
- [ ] **Package Manager (`tpm`)**: "Turn Package Manager". Share skills (`@turn/researcher`, `@turn/coder`) easily.

---

### Phase 6: Public Launch (v1.0)
*Goal: Community adoption.*
- [ ] **Documentation**: Complete language reference, "Turn by Example".
- [ ] **The Playground**: Web-based WASM runner to try Turn without installing.
- [ ] **Benchmarks**: "Turn vs Python" suite demonstrating 90% code reduction and 10x reliability.

---

### Phase 7: The Distributed Swarm (Node-to-Node)
*Goal: Agents that span servers.*
- [ ] **Distributed Actor Model**: A Turn process on Server A can `send(pid, msg)` to a process on Server B transparently.
- [ ] **Node Identity**: Every Turn Runtime has a public/private keypair.
- [ ] **Remote PIDs**: `pid` format becomes `node_id:process_id`.
- [ ] **Encrypted Transport**: `send(remote_pid, msg)` opens a mTLS connection to the remote node.
- [ ] **Mesh Networking**: Automatic discovery of Turn nodes on a private network.
- [ ] **Swarm Consensus**: Built-in Raft/Paxos primitives for agents to vote on decisions.

---

### Phase 8: The Global Agent Web (The Protocol)
*Goal: A World Wide Web for Agents.*
- [ ] **Turn Protocol (`tap://`)**: A standard protocol for agents to negotiate services and payments.
- [ ] **Universal Registry**: A DHT (Distributed Hash Table) for agents to publish capabilities ("I am a Coder") and discover peers.
- [ ] **Marketplace**: Agents can hire other agents (e.g., a "Coder" agent hires a "Reviewer" agent) using micro-payments.

---

## 8. Summary: What We Are Building

Turn is a **Physics Engine for Cognition**. We are building:

1.  **A Durable Heap:** Memory that survives power loss (Log-Structured Allocator, WAL, Merkle Trees).
2.  **A Semantic Memory Manager:** Addressing by meaning (ANN index, `remember`/`recall` opcodes).
3.  **A Priority-Aware Context Renderer:** Automatic structuring of prompts to respect attention physics (P0/P1/P2 tiering).
4.  **A Token-Aware Scheduler:** Budget enforcement, rate limiting, and cost tracking at the VM level.
5.  **A Distributed Actor Mesh:** Transparent cross-node messaging with encrypted transport.

We are building the laws of conservation (Memory), time (Persistence), and energy (Tokens) into the language itself.

This is not a framework. This is not a library. This is **Turn**—the Systems Language for AI.

---

**References:**
- [The Physics of AI Engineering](https://ai-dojo.io/papers/the-physics-of-ai-engineering) — Muyukani Kizito, 2026
- [Turn Whitepaper](./WHITEPAPER.md) — Technical foundations and abstract
- [Turn Specification](./spec/) — Formal language grammar and semantics
