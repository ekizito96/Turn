# Turn Language

<p align="center">
  <a href="https://turn-lang.dev"><strong>Website & Documentation</strong></a> ·
  <a href="https://turn-lang.dev/#playground"><strong>Live Playground</strong></a>
</p>

Turn is the first programming language where autonomous agents are not a pattern you implement — they are the execution model. Actors, belief states, confidence-gated decisions, immutable state epochs, and crash-recoverable OODA loops are not library abstractions. They are how the VM works.

---

## The Real Problem

Production agentic software is not hard because parsing JSON is tedious. It is hard because **no general-purpose language has a runtime model that matches how agents actually work**.

An autonomous agent running a real workflow is:

- A **stateful OODA loop** that must survive crashes, LLM timeouts, and partial failures without corrupting its state
- A **belief system** — it forms hypotheses, accumulates evidence, retracts false assumptions, and updates its world model across turns
- A **DAG scheduler** — it receives a dependency graph of tasks, tracks which steps are ready, dispatches sub-agents concurrently, and waits for results before proceeding
- A **multi-tier memory architecture** — system context that never evicts, a sliding working memory window, and episodic storage for what overflows
- A **probabilistic decision-maker** — every inference has a confidence attached, and whether to act, retry, or escalate depends on that confidence, not on whether the response parsed

The industry solution is to bolt external infrastructure onto a general-purpose language: external stores for state, message queues for agent communication, schema validators for LLM output coercion, async runtimes for concurrency, and retry logic scattered across every layer. The larger the system, the more infrastructure you add — and none of it solves the root problem. It compensates for a language that has no concept of what an agent is.

**Turn solves it at the language and VM level.** These are not libraries. They are how the runtime works.

---

## What the VM Provides Natively

### Actor Isolation with Linked Failure Propagation

Every agent in Turn is an isolated process with its own stack, heap, and LLM context window. There is no shared state. `spawn_link` creates a dependency between two processes: when a child agent completes or fails, the parent receives an `ExitSignal` in its mailbox. The parent decides whether to retry, escalate, or compensate. This is not an async framework. It is the execution model.

```turn
let analyst_pid = spawn_link turn(task) {
    let result = infer Analysis { task };
    return result;
};

let msg = receive;
if msg["type"] == "exit" {
    if msg["reason"] == "normal" { return msg["result"]; }
    return handle_failure(msg["reason"]);
}
```

### State as Immutable Epochs

Turn enforces strict immutability. An agent's state is never mutated — every OODA cycle produces a new, complete state snapshot called an **epoch**. The struct spread operator (`..base`) makes this concise:

```turn
let next_state = WorkflowState {
    processed_steps: updated_processed,
    failed_steps: updated_failed,
    ..current_state
};
```

Because state is immutable and the VM checkpoints on every suspension, crash recovery is structural. The VM re-loads the last epoch and resumes from exactly where it left off. This is not a retry decorator. The language model makes it impossible for an agent to corrupt its own state.

### Probabilistic Control Flow

Every `infer` call returns an `Uncertain(value, confidence)` — a first-class VM type. Confidence propagates through arithmetic. You gate execution on it directly:

```turn
let decision = infer AgentDecision {
    "Given this context, what is the next action?"
};

if confidence decision < 0.85 {
    send supervisor_pid, { "type": "low_confidence_escalation", "context": state };
    return null;
}
```

This is not an if-statement around a try/catch. The `confidence` operator is a bytecode instruction. The VM enforces that uncertain values cannot be used as if they were certain without explicitly acknowledging the uncertainty.

### Cognitive Type Safety

Define a struct and call `infer`. The VM passes the schema to the LLM inference driver and guarantees the returned value matches that shape. No manual JSON parsing. No retry loop. If the value doesn't conform, the VM surfaces an error before your code ever touches the result.

```turn
struct BeliefUpdate {
    new_facts: List<Str>,
    retracted_assumptions: List<Str>,
    confidence: Num
};

let update = infer BeliefUpdate {
    "Review this evidence and update the belief state: " + evidence
};
```

### Three-Tier Working Memory

Every actor's runtime maintains a structured context window with three tiers: a system tier that never evicts, a working memory tier (sliding window, LRU), and an episodic tier that holds overflow. Context is isolated per actor. You manage it with `context.append()`:

```turn
context.append("Previous analysis: " + prior_result.summary);
let next = infer Hypothesis { current_evidence };
```

This is not a list you pass to an API call. It is the actor's memory state, automatically prepended to every inference that actor makes.

### DAG Scheduling and Sub-Agent Orchestration

`spawn_each` delegates a list of tasks concurrently to independent actor instances. Combined with `spawn_link` and typed mailboxes, you implement full dependency-graph schedulers entirely in Turn:

```turn
let ready_steps = filter(workflow.steps, turn(step) {
    return check_deps_met(step.dependencies, state.processed);
});

spawn_each ready_steps turn(step) {
    send kernel_pid, { "type": "execute_step", "step": step };
};
```

---

## Provider Agnosticism via WASM Drivers

LLM provider APIs change constantly. Turn does not track them. Every provider is an isolated WebAssembly module in `.turn_modules/` — a driver that translates a standard Turn inference request into that provider's HTTP format and normalises the response back.

Turn ships with drivers for **Anthropic**, **OpenAI**, **Google Gemini**, **xAI Grok**, **Ollama**, **Azure OpenAI**, and **Azure Anthropic**. Set `TURN_LLM_PROVIDER` and the VM routes accordingly. If a provider changes their API, you update one `.wasm` file. The Turn compiler and VM are untouched.

You can ship your own driver for any private or emerging model:

```rust
pub unsafe extern "C" fn transform_request(ptr: u32, len: u32) -> u64 { ... }
pub unsafe extern "C" fn transform_response(ptr: u32, len: u32) -> u64 { ... }
```

Compile to `wasm32-unknown-unknown`, drop it in `.turn_modules/`, and Turn picks it up. See the `providers/` directory for reference implementations.

---

## Compile-Time Schema Adapters

When an API publishes a structured specification (OpenAPI, GraphQL, FHIR), Turn absorbs it directly into the compiler. `use schema::openapi` fetches the schema, parses it, and synthesises native bytecode closures at compile time. No SDK dependencies, no boilerplate HTTP wrappers, no "function registry."

```turn
let gcal = use schema::openapi("https://googleapis.com/.../calendar/v3/rest");

let events = gcal.events.list({
    "calendarId": "primary",
    "timeMin": call("time_now")
});
```

The API's types become the actual memory layout of the Turn VM. For unstructured or undocumented data, the `infer Struct` primitive coerces raw payloads into typed memory using the LLM as a native type coercer.

---

## Zero-Trust Authentication

API keys and OAuth tokens in traditional frameworks are strings in the agent's memory. A prompt injection attack can extract them. Turn introduces `Identity` as a first-class primitive type.

```turn
let my_google = grant identity::oauth("google_workspace");

let events = call("http_get", {
    "url": "https://www.googleapis.com/calendar/v3/users/me/calendarList",
    "identity": my_google
});
```

The `grant` keyword requests a cryptographic capability from the Turn VM host. The raw token never enters Turn's bytecode memory. When the agent passes the `Identity` handle to an HTTP tool, the Rust host intercepts the call, looks up the real credential from a secure environment variable (`TURN_IDENTITY_<PROVIDER>_TOKEN`), and injects the `Authorization` header before the request goes over the wire. The LLM cannot print, email, or exfiltrate the token because it only holds an opaque, unforgeable handle.

---

## Quick Start

### Installation

```bash
cargo install --git https://github.com/ekizito96/Turn turn
```

### Your First Agent

```turn
struct Sentiment { score: Num, reasoning: Str };

turn {
    let input = "I absolutely love building systems in Rust!";

    let result = infer Sentiment {
        "Analyze the sentiment of the following text: " + input
    };

    if confidence result < 0.8 {
        call("echo", "Low confidence — escalating.");
        return null;
    }

    call("echo", "Score: " + result.score);
    call("echo", "Reasoning: " + result.reasoning);

    return result;
}
```

### Running It

```bash
# OpenAI
export TURN_LLM_PROVIDER=openai
export OPENAI_API_KEY=sk-your-key

# Anthropic
export TURN_LLM_PROVIDER=anthropic
export ANTHROPIC_API_KEY=sk-ant-your-key

# Google Gemini
export TURN_LLM_PROVIDER=gemini
export GEMINI_API_KEY=your-key

# xAI Grok
export TURN_LLM_PROVIDER=grok
export XAI_API_KEY=your-key

# Ollama (local, no key required)
export TURN_LLM_PROVIDER=ollama

# Azure OpenAI
export TURN_LLM_PROVIDER=azure_openai
export AZURE_OPENAI_KEY=your-key
export AZURE_OPENAI_ENDPOINT=https://your-resource.openai.azure.com
export AZURE_OPENAI_DEPLOYMENT=your-deployment-name

turn run hello.tn
```

---

## Examples

The `impl/examples/` directory contains multi-agent demonstrations:

- [**Algorithmic Trading Syndicate**](impl/examples/quant_syndicate.tn): Three agents (Technical, Sentiment, Risk) run concurrently, debate a trade via mailboxes, and a Chairman agent executes the final decision with confidence gating.
- [**Investment Committee**](impl/examples/investment_committee.tn): Specialist agents evaluate an equity position concurrently using live Yahoo Finance data.
- [**Marketing Agency**](impl/examples/marketing_agency.tn): An SEO Specialist, Copywriter, and Creative Director collaborate to produce ad copy using Wikipedia research.

---

## Documentation

Full language reference, runtime model, and architecture guide at [turn-lang.dev/docs](https://turn-lang.dev/docs).

## License

Turn is open-source software licensed under the [MIT License](LICENSE).
