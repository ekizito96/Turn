# Turn design mandate

**Status:** Locked. This is the mission and design constraints for Turn. All spec and implementation decisions must align with it.

---

## 1. What Turn is

**Turn is a systems language for agentic computation.**

We are not retrofitting an existing language. Turn’s execution model is built around long-lived processes that operate under finite context capacity, stochastic inference, explicit effects (tool calls), and durable state. Turn exists to remove the common failure modes of agentic systems by making these constraints language-native and mechanically enforceable.

**Object model stance:** Turn is object-oriented by **capability and encapsulation**. Value objects (`struct` + methods) and process objects (PID + mailbox + durable state) are the primary organizing units. Turn prefers composition and behavioral contracts (interfaces/traits) over inheritance-heavy hierarchies.

---

## 2. Pains we solve (no retrofit)

When you build agentic software in current languages, you encode agentic behavior with loops, lists, async, and ad-hoc state. That leads to:

- **Context** not first-class → manual lists, trimming, "lost in the middle," no shared semantics
- **Memory** not first-class → ad-hoc stores, no standard remember/recall/forget
- **Turn** and **tool call** not first-class → hand-rolled loops, fake suspension with async/await, no standard "one step"
- **State** smeared across variables and frameworks → hard to reason, serialize, or debug
- **Wrong mental model** → you think in turns and context; you code in loops and lists
- **Observability, cost, governance** → all bolted on; no first-class trace or policy

Turn makes the **process** and its **context**, **memory**, **turn/control-cycle boundaries**, and **effects** primitives explicit, with runtime-enforced invariants. In v1 the program is the behavior of a single process instance with runtime-managed context and memory objects. We reserve "first-class" for values that can be stored, passed, and returned.

**Why this minimal set:** Each primitive earns its place. *Turn* = unit of execution and checkpointing; without it we cannot define "one step" or persist state. *Context* = bounded buffer the language can enforce (unbounded lists in other langs cannot). *Memory* = persistent store across turns; distinct from context ("send now" vs "keep"). *Tool call* = single suspension boundary for external effects. We cannot derive one from the others without losing expressiveness or invariants. So the minimal core is justified, not arbitrary.

---

## 3. What we optimize for

Turn is designed so that running agentic software is **minimal and disciplined** in every dimension that matters:

| Dimension | Goal |
|-----------|------|
| **Tokens** | Bounded context by construction; runtime enforces a strict bound (invariant: |context| ≤ N). Token use is **bounded and observable**—the language and runtime make it possible to enforce and reason about token budget, not "minimize" an unspecified quantity. |
| **Performance** | Execution model (one turn, one config, one transition) is transparent and optimizable. No hidden event loop or framework overhead. **Rust runtime from day one** (bytecode VM) for native speed, minimal overhead, single binary. |
| **Computation power** | **Two-layer solution:** (1) **Semantic reduction** (big wins): bounded context, explicit turn/checkpoint, memory discipline, tool output control. (2) **Runtime reduction** (steady wins): Rust VM = minimal overhead per turn, fast serialization, cheap policy checks. More compute budget goes to **actual agent work**, not interpreter overhead. |
| **Deterministic semantics** | Turn's core language is **deterministic**: given configuration and external inputs (tool results), execution is reproducible. Non-determinism is **quarantined** at effect boundaries (tool calls, LLM calls). This enables debugging, audit, replay: same inputs → same state transitions. |
| **Security** | Governance in scope: what tools can be called, what can be written to memory, what can leave the agent. Injection, secrets, and capability boundaries. Audit trail follows from primitive turn and action. |
| **Boilerplate** | No hand-rolled agent loop. No "wire context, memory, and tools in 200 lines." One turn, one context, one memory, one call—the language does the wiring. |
| **Observability** | Turn and context are primitive and explicit, so traces (what happened this turn, what context at each step) are standard, not ad-hoc logs. Deterministic semantics enable **replay**: same inputs → same trace. |
| **Cost** | Token budget is enforceable (context bound); compute cost has a clear model (one config, one transition per step). Rust runtime = minimal CPU/memory overhead, so more budget for tokens and actual work. |

We add more as we grow (e.g. multi-agent, richer types), but **minimal tokens, performance, security, and boilerplate** are non-negotiable design goals.

---

## 4. How the spec reflects this

- **Minimal core (01):** Smallest set of primitives—turn, context.append, remember, recall, call—so we don't carry retrofit baggage.
- **Grammar (02):** One obvious way to form a turn and use context/memory/tool; no 10 styles of agent loops.
- **Runtime (03):** One configuration, one transition; serializable state; default runtime with bounded context so token and cost are controllable.
- **Types and errors (05):** Type-friendly design for future safety; clear error model for debugging and tooling.

**Implementation:** Turn is built in **Rust from day one** (bytecode VM). Not Python/TypeScript—those languages' overhead contradicts our goals (fast, cost-efficient). Rust gives us native speed, minimal memory, single binary, true concurrency. See [07-implementation-strategy.md](07-implementation-strategy.md).

Future work (multi-agent semantics, richer types, stronger policies) stays within this mandate: **physics-first semantics, determinism at boundaries, durable state, bounded context, and measurable cost control.**
