# Turn design mandate

**Status:** Locked. This is the mission and design constraints for Turn. All spec and implementation decisions must align with it.

---

## 1. What Turn is

**Turn is a new programming language for agentic software.**

We are not retrofitting an existing language. We are building a language whose primary abstractions are agentic: turn, context, memory, goal, tool. Traditional languages (Python, TypeScript, C, etc.) were built for a different execution model; retrofitting them for agentic software causes real pains. Turn exists to remove those pains by making agentic concepts native.

---

## 2. Pains we solve (no retrofit)

When you build agentic software in current languages, you encode agentic behavior with loops, lists, async, and ad-hoc state. That leads to:

- **Context** not first-class → manual lists, trimming, "lost in the middle," no shared semantics
- **Memory** not first-class → ad-hoc stores, no standard remember/recall/forget
- **Turn** and **tool call** not first-class → hand-rolled loops, fake suspension with async/await, no standard "one step"
- **State** smeared across variables and frameworks → hard to reason, serialize, or debug
- **Wrong mental model** → you think in turns and context; you code in loops and lists
- **Observability, cost, governance** → all bolted on; no first-class trace or policy

Turn makes **turn, context, memory, goal, and tool** **primitive and explicit**—language-native, with a fixed API and runtime-enforced invariants—so that the language matches how agentic systems actually work. In v1 context and memory are **runtime-managed state** with a fixed interface, not first-class values (you cannot pass or store them); we reserve "first-class" for values that can be stored, passed, and returned. Full problem space and science: [research/00-problems-we-solve.md](../research/00-problems-we-solve.md).

**Why this minimal set:** Each primitive earns its place. *Turn* = unit of execution and checkpointing; without it we cannot define "one step" or persist state. *Context* = bounded buffer the language can enforce (unbounded lists in other langs cannot). *Memory* = persistent store across turns; distinct from context ("send now" vs "keep"). *Tool call* = single suspension boundary for external effects. We cannot derive one from the others without losing expressiveness or invariants. So the minimal core is justified, not arbitrary.

---

## 3. What we optimize for

Turn is designed so that running agentic software is **minimal and disciplined** in every dimension that matters:

| Dimension | Goal |
|-----------|------|
| **Tokens** | Bounded context by construction; runtime enforces a strict bound (invariant: |context| ≤ N). Token use is **bounded and observable**—the language and runtime make it possible to enforce and reason about token budget, not "minimize" an unspecified quantity. |
| **Performance** | Execution model (one turn, one config, one transition) is transparent and optimizable. No hidden event loop or framework overhead. Path to compilation (bytecode, native) so compute is minimal. |
| **Security** | Governance in scope: what tools can be called, what can be written to memory, what can leave the agent. Injection, secrets, and capability boundaries. Audit trail follows from primitive turn and action. |
| **Boilerplate** | No hand-rolled agent loop. No "wire context, memory, and tools in 200 lines." One turn, one context, one memory, one call—the language does the wiring. |
| **Observability** | Turn and context are primitive and explicit, so traces (what happened this turn, what context at each step) are standard, not ad-hoc logs. |
| **Cost** | Token budget is enforceable (context bound); compute cost has a clear model (one config, one transition per step) so optimization and compilation have a target. |

We add more as we grow (e.g. multi-agent, richer types), but **minimal tokens, performance, security, and boilerplate** are non-negotiable design goals.

---

## 4. How the spec reflects this

- **Minimal core (01):** Smallest set of primitives—turn, context.append, remember, recall, call—so we don't carry retrofit baggage.
- **Grammar (02):** One obvious way to form a turn and use context/memory/tool; no 10 styles of agent loops.
- **Runtime (03):** One configuration, one transition; serializable state; default runtime with bounded context so token and cost are controllable.
- **Types and errors (05):** Type-friendly design for future safety; clear error model for debugging and tooling.

Future work (OOP shape, agent as value, multi-agent, modules, types) stays within this mandate: **new language for agentic software, solve retrofit pains, minimize tokens and compute, performance, security, boilerplate.**
