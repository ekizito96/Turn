# Why Turn

Turn is a domain-specific language for agentic computation. It exists because existing tools - Python, TypeScript, LangChain, AutoGen — are built for general-purpose programming and retrofitted for agentic use. The mismatch produces fragile, unpredictable systems.

## The Five Failure Modes

**1. Context overflows silently.** You append to a messages list. At some point the model halluccinates from stale context. There is no language signal — the list just became too long.

**2. Inference outputs are untyped.** The model returns a string. You `json.loads()` it and hope the schema matched. Type errors happen at runtime, far from their source.

**3. State is smeared everywhere.** Agent state lives across a loop variable, an external DB, a cache, and API responses. There is no single, inspectable agent state.

**4. There is no concept of a "turn".** The fundamental unit of agentic work has no representation in the language and cannot be serialized, suspended, or resumed.

**5. The mental model doesn't match the code.** You think in turns, context windows, and memory. You code in loops, lists, and dictionaries.

## The Turn Solution

Turn closes each gap at the language level:

| Failure Mode | Turn Solution |
|---|---|
| Context overflows | `context.append()` is token-metered. The VM enforces bounds via the Priority Stack. |
| Untyped inference | `infer Struct { prompt }` generates a JSON Schema at compile time and validates the response. |
| Smeared state | Agent state is `(env, context, memory, mailbox)` — serializable, inspectable, well-defined. |
| No durable execution | `suspend` checkpoints the full VM state. The agent resumes exactly where it stopped. |
| Mental model mismatch | Turns, context, memory, inference, and actors are first-class language primitives. |

## What Turn Is Not

Turn is not a general-purpose language. It does not replace Python or Rust. It is a **domain-specific language for agentic computation** — the way SQL is for relational queries.

→ Next: [The Process Model](01-the-process-model.md)
