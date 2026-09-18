# Turn Language

<p align="center">
  <a href="https://turn-lang.dev"><strong>Website & Documentation</strong></a> ·
  <a href="https://turn-lang.dev/#playground"><strong>Live Playground</strong></a>
</p>

Turn is a programming language where autonomous agents are not a pattern you implement — they are the execution model. Actors, managed context, confidence-aware decisions, durable suspension, and crash-recoverable execution are language and VM concepts rather than application-library conventions.

## Decision Models Preview

LLMs generate. Decision models judge. Turn coordinates both as durable effects.

```turn
let result = decide("The export button crashes in Safari", {
    "department": {
        "type": "choice",
        "instructions": "Which team should handle this?",
        "criteria": {
            "billing": "Charges and refunds",
            "technical": "Bugs and integrations"
        }
    }
});

return result["answers"]["department"];
```

Try the complete Choice + Score + Noul workflow without an API key:

```bash
git clone https://github.com/ekizito96/Turn.git
cd Turn
TURN_DECISION_PROVIDER=mock cargo run -- run examples/jev_triage.tn
```

Run the same program on TypeSafe Jev by setting `TYPESAFE_API_KEY` and removing the mock override. The TypeSafe adapter is bundled into the Turn binary.

---

## The Real Problem

Production agentic software is not hard because parsing JSON is tedious. It is hard because general-purpose runtimes do not natively model the full lifecycle of long-running, probabilistic agents.

An autonomous agent running a real workflow is:

- A **stateful OODA loop** that must survive crashes, LLM timeouts, and partial failures without corrupting its state
- A **belief system** — it forms hypotheses, accumulates evidence, retracts false assumptions, and updates its world model across turns
- A **DAG scheduler** — it receives a dependency graph of tasks, tracks which steps are ready, dispatches sub-agents concurrently, and waits for results before proceeding
- A **multi-tier memory architecture** — system context that never evicts, a sliding working memory window, and episodic storage for what overflows
- A **probabilistic decision-maker** — when a model exposes confidence, whether to act, retry, or escalate depends on that signal, not only on whether the response parsed

The industry solution is to bolt external infrastructure onto a general-purpose language: external stores for state, message queues for agent communication, schema validators for LLM output coercion, async runtimes for concurrency, and retry logic scattered across every layer. The larger the system, the more infrastructure you add — and none of it solves the root problem. It compensates for a language that has no concept of what an agent is.

**Turn solves it at the language and VM level.** These are not libraries. They are how the runtime works.

---

## What the VM Provides Natively

### Actor Isolation with Linked Failure Propagation

Every agent in Turn is an isolated process with its own frames, environment, managed context, memory, and mailbox. There is no shared mutable state. `spawn_link` creates a dependency between two processes: when a child agent completes or fails, the parent receives an `ExitSignal` in its mailbox. The parent decides whether to retry, escalate, or compensate. This is not an async framework. It is the execution model.

```turn
let analyst_pid = spawn_link turn(task) {
    let result = infer Analysis { task };
    return result;
};

let msg = receive;
if msg["type"] == "ExitSignal" {
    if msg["reason"] != null { return handle_failure(msg["reason"]); }
    return msg["result"];
}
```

### Immutable State Values and Durable Continuations

Turn bindings and structured values are immutable. State evolution produces a new value, and the struct spread operator (`..base`) makes that concise:

```turn
let next_state = WorkflowState {
    processed_steps: updated_processed,
    failed_steps: updated_failed,
    ..current_state
};
```

Because values are immutable and the VM checkpoints at suspension boundaries, recovery restores the last persisted continuation and replays its pending effect instead of injecting a placeholder result. Effect delivery is at-least-once, so non-idempotent tools should use idempotency keys.

### Durable Effect Journal

Every external tool, inference, and decision suspension receives an execution-scoped effect ID. Turn checkpoints the pending request before execution and atomically records its outcome afterward. If the process crashes after that outcome is committed, restart resumes from the journal instead of invoking the effect again.

```bash
turn effects factory-run
turn effects factory-run --json
turn effects factory-run --json --show-payloads
```

Payloads are hidden by default because effect arguments and results may contain source code, prompts, or customer data. Journal records remain after successful completion for audit and replay diagnostics.

This closes the duplicate-execution window **after journal commit**. It does not make an external service transactional with Turn: if a service accepts an operation and the process dies before Turn commits the result, the request may still be repeated. Non-idempotent integrations must accept the Turn effect ID or another idempotency key.

### Probabilistic Control Flow

Inference providers that expose measured confidence return `Uncertain(value, confidence)`, a first-class VM value. Confidence propagates through arithmetic and can gate execution directly:

```turn
let decision = infer AgentDecision {
    "Given this context, what is the next action?"
};

if confidence decision < 0.85 {
    send supervisor_pid, { "type": "low_confidence_escalation", "context": state };
    return null;
}
```

The `confidence` operator is a bytecode instruction. Values from providers without a confidence signal remain unwrapped rather than receiving a fabricated score.

### Decision Models as a Separate Effect

Generative inference and bounded decisions are different operations. `infer` asks a model to synthesize a typed value; `decide` evaluates explicit questions against state through a decision-model provider. The VM records each as a distinct durable effect, so applications can route them to different model classes without changing their control flow.

```turn
let questions = {
    "department": {
        "type": "choice",
        "instructions": "Which team should handle this?",
        "criteria": {
            "billing": "Charges and refunds",
            "technical": "Bugs and integrations"
        }
    }
};

let decision = decide(ticket, questions);
let answer = decision["answers"]["department"];

if answer["confidence"] < 0.8 {
    send reviewer_pid, ticket;
}
```

`decide(state, questions)` is provider-neutral. Turn ships a TypeSafe System One driver for Jev, while the language contract remains open to other bounded-decision models.

### Schema-Constrained Inference

Define a struct and call `infer`. The VM passes its schema to the inference driver and normalises the structured response into a Turn value. Drivers with strict structured-output support enforce the schema at the provider boundary; other drivers must report malformed responses as inference errors. Turn applications do not parse provider JSON directly.

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

`spawn_each` delegates a list of tasks concurrently to independent actor instances. `gather` collects all their results back in input order once every agent completes, with `allSettled` semantics: a failed agent returns its error as a value rather than aborting the entire batch.

```turn
let invoice_ids = [1001, 1002, 1003, 1004, 1005];

// Scatter: one agent per invoice
let agents = spawn_each(invoice_ids, turn(id: Num) {
    return infer Invoice { "Reconcile invoice " + id; };
});

// Gather: block until all complete, in order
let results = gather agents;
```

This is Turn's native local concurrency model. The VM scheduler handles actor execution and `gather` handles ordered collection without requiring an application-level queue abstraction.

### Compile-Time Type Enforcement

Turn's semantic analyzer runs before execution. Every function call is type-checked against its parameter declarations. If data flowing from `infer` into a downstream function does not match the expected schema, the program is rejected at compile time with a precise field-level error.

```turn
struct StripePayload { amount: Num, currency: Str };

let charge = turn(payload: StripePayload) { ... };
let bad    = infer WrongPayload { "charge 100 usd" };

call(charge, bad); // Compile-time error: type mismatch
```

Turn uses gradual typing. Full enforcement requires explicit parameter annotations.

### VM Observability with `turn inspect`

Suspended agents can be inspected from the command line. `turn inspect` reads the serialized `VmState` and prints a structured snapshot of the agent's context window, durable memory, actor mailbox, cognitive belief state (last inference confidence score), and supervisor tree.

```bash
turn run reconciliation_agent.tn --id recon_agent
turn inspect recon_agent
```

No instrumentation required. No code changes needed.

---

## Provider Agnosticism via WASM Drivers

LLM provider APIs change constantly. Turn isolates each provider behind a WebAssembly driver that translates a standard Turn request into the provider's HTTP format and normalises the response back.

Turn bundles generative drivers for **Anthropic**, **OpenAI**, **Google Gemini**, **xAI Grok**, **Ollama**, **Azure OpenAI**, and **Azure Anthropic**, plus a decision driver for **TypeSafe System One models**. Set `TURN_LLM_PROVIDER` for `infer` and `TURN_DECISION_PROVIDER` for `decide`. Project-local drivers in `.turn_modules/` override bundled drivers, so provider upgrades and private adapters do not require compiler or VM changes.

You can ship your own driver for any private or emerging model:

```rust
pub unsafe extern "C" fn transform_request(ptr: u32, len: u32) -> u64 { ... }
pub unsafe extern "C" fn transform_response(ptr: u32, len: u32) -> u64 { ... }
```

Compile to `wasm32-unknown-unknown`, drop it in `.turn_modules/`, and Turn picks it up. See the `providers/` directory for reference implementations.

---

## Experimental OpenAPI Schema Adapters

Turn includes an experimental OpenAPI-to-AST compiler that parses schemas and synthesises typed Turn closures. The `use schema::openapi` syntax below is the intended surface, but it is not yet wired into the stable runner path. GraphQL and FHIR adapters remain design targets rather than shipped features.

```turn
let time = use "std/time";
let gcal = use schema::openapi("https://googleapis.com/.../calendar/v3/rest");

let events = gcal.events.list({
    "calendarId": "primary",
    "timeMin": time.now()
});
```

Do not depend on schema adapters for production workflows until runner integration and conformance tests land.

---

## Zero-Trust Authentication

API keys and OAuth tokens in traditional frameworks are strings in the agent's memory. A prompt injection attack can extract them. Turn introduces `Identity` as a first-class primitive type.

```turn
let net       = use "std/net";
let my_google = grant identity::oauth("google_workspace");

let events = net.get({
    "url":      "https://www.googleapis.com/calendar/v3/users/me/calendarList",
    "identity": my_google
});
```

The `grant` keyword requests a cryptographic capability from the Turn VM host. The raw token never enters Turn's bytecode memory. When the agent passes the `Identity` handle to an HTTP tool, the Rust host intercepts the call, looks up the real credential from a secure environment variable (`TURN_IDENTITY_<PROVIDER>_TOKEN`), and injects the `Authorization` header before the request goes over the wire. The LLM cannot print, email, or exfiltrate the token because it only holds an opaque, unforgeable handle.

---

## Quick Start

### Installation

```bash
cargo install --git https://github.com/ekizito96/Turn turn
turn doctor
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

# TypeSafe Jev for decide(...)
export TURN_DECISION_PROVIDER=typesafe
export TYPESAFE_API_KEY=your-key

turn run hello.tn
```

---

## Examples

The `impl/examples/` directory contains multi-agent demonstrations:

- [**Algorithmic Trading Syndicate**](impl/examples/quant_syndicate.tn): Three agents (Technical, Sentiment, Risk) run concurrently, debate a trade via mailboxes, and a Chairman agent executes the final decision with confidence gating.
- [**Investment Committee**](impl/examples/investment_committee.tn): Specialist agents evaluate an equity position concurrently using live Yahoo Finance data.
- [**Marketing Agency**](impl/examples/marketing_agency.tn): An SEO Specialist, Copywriter, and Creative Director collaborate to produce ad copy using Wikipedia research.
- [**Jev Support Triage**](examples/jev_triage.tn): A TypeSafe decision model classifies a support ticket and Turn gates autonomous routing on confidence.
- [**Software Factory**](examples/software_factory.tn): Decision gates, LLM planning, repository tools, tests, and crash-safe pull-request creation in one durable workflow.

## CLI Reference

| Command | Description |
|---------|-------------|
| `turn run <file> [--id <id>] [--store <path>]` | Compile and run a Turn program |
| `turn run <file> --sandbox` | Run with host I/O, imports, and identity grants denied |
| `turn inspect <id> [--store <path>]` | X-ray a suspended agent's full VM state |
| `turn effects <id> [--store <path>]` | List durable effect outcomes with payloads redacted by default |
| `turn serve [--port <n>] [--store <path>]` | Start the HTTP server for remote agent execution |
| `turn lsp` | Start the Language Server Protocol server (stdio) |
| `turn add <name> <url>` | Add a package dependency |
| `turn doctor` | Show selected providers, bundled-driver availability, and credential status |

---

## Documentation

Full language reference, runtime model, and architecture guide at [turn-lang.dev/docs](https://turn-lang.dev/docs). Release compatibility follows [Semantic Versioning](VERSIONING.md).

## License

Turn is open-source software licensed under the [MIT License](LICENSE).
