<div align="center">

# Turn

**A systems language for agentic computation.**

Turn's execution model is built from first principles for intelligent agents: bounded context, stochastic inference as a first-class primitive, durable suspension/resumption, and an actor model for multi-agent swarms.

[![v0.5.0-alpha](https://img.shields.io/badge/version-v0.5.0--alpha-orange)](https://github.com/ekizito96/Turn/releases)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Built in Rust](https://img.shields.io/badge/built_in-Rust-orange?logo=rust)](https://www.rust-lang.org/)

[**Docs**](https://turn-lang.dev/docs) · [**Why Turn**](https://turn-lang.dev/docs/why-turn) · [**The Book**](https://turn-lang.dev/docs/installation) · [**Providers**](PROVIDERS.md)

</div>

---

## In 60 Seconds

Install Turn and get a real LLM inference running locally — no framework, no prompt templates, no boilerplate.

```bash
# Install
curl -fsSL https://turn-lang.dev/install.sh | bash

# Configure your LLM provider
export TURN_INFER_PROVIDER=~/.turn/providers/turn_provider_openai.wasm
export OPENAI_API_KEY=sk-...

# Write your first Turn program
cat > sentiment.tn << 'EOF'
struct Sentiment {
    score: Num,
    label: Str,
    reasoning: Str
};

let review = "The new MacBook keyboard is finally good again.";
let result = infer Sentiment { review; };

call("echo", "Score:  " + result.score);
call("echo", "Label:  " + result.label);

return result;
EOF

# Run it
turn run sentiment.tn
```

```json
{
  "score": 0.88,
  "label": "positive",
  "reasoning": "Positive sentiment about keyboard improvement."
}
```

That's `infer` — LLM inference with compile-time schema guarantees, backed by a sandboxed WebAssembly driver. No HTTP calls in your code. No JSON parsing. A structured type, straight from the model.

---

## The Problem with Agent Frameworks

Every major "agentic" framework today is built on a fundamental lie: that you can wrap a loop around an LLM and call the result an "agent."

```python
# This is what every framework reduces to
while True:
    response = llm.chat(context)         # context: who manages this?
    if "DONE" in response: break         # brittle string matching
    context.append(response)             # unbounded list
    tool_result = execute(response)      # untyped, unsafe
    context.append(tool_result)          # still unbounded
```

The problems are structural:
- **Context overflows silently.** You append to a list. The model gets confused. You don't know why.
- **LLM outputs are untyped.** You parse JSON from a string. You hope the schema matched.
- **State is smeared everywhere.** Between the loop, the DB, the cache, the API response.
- **There is no language-level concept of a "turn."** The mental model doesn't match the implementation.

Turn solves these at the language level. Not with a better library — with a different execution model.

---

## The Turn Model

Turn programs execute as **stateful processes**, not loops. Every execution unit is a *turn* — an atomic, durable unit of agentic work with three built-in resources:

| Resource | What it is | Turn primitive |
|---|---|---|
| **Environment** | Lexically-scoped variable bindings | `let`, `return` |
| **Context** | Token-budgeted window of working knowledge | `context.append()` |
| **Memory** | Persistent semantic key-value store with HNSW vectors | `remember()` / `recall()` |

When the agent needs to call a tool or run inference, the VM **suspends** — serializing its entire state to durable storage — and **resumes** with the result. No threads blocked. No state lost.

```
Turn Agent
  │
  ├── env:     { score → 0.88, label → "positive" }
  ├── context: [ "Analyzing sentiment...", "Score: 0.88" ]    ← Token-bounded
  ├── memory:  { "last_review" → "The new MacBook..." }       ← Persisted
  └── mailbox: []                                              ← Actor inbox
```

---

## Key Primitives

### `infer` — Cognitive Type Safety

```turn
struct Sentiment {
    score: Num,
    label: Str
};

// The VM generates a JSON Schema from the struct at compile time.
// The LLM response is validated against it. You get a typed value.
let result = infer Sentiment { "I love this language!"; };

// result.score is a Num. result.label is a Str.
// Guaranteed. No parsing needed.
if result.score > 0.8 {
    call("echo", "Positive: " + result.label);
}
```

### `remember` / `recall` — Semantic RAM

```turn
// Persist any value to the agent's semantic memory
remember("preferred_style", "concise bullet points");

// Later — even across process restarts — retrieve by semantic similarity
let style = recall("preferred_style");
```

### `spawn` / `send` / `receive` — Actor Model

```turn
// Spawn a child agent process (returns a PID)
let analyst = spawn {
    let task = receive();
    let result = infer Analysis { task; };
    return result;
};

// Send work to it
send(analyst, "Analyze Q4 revenue trends");

// Each agent: isolated memory, isolated context, isolated mailbox
```

### `suspend` — Durable Checkpoints

```turn
// Serialize the full VM state to disk. Safely resume after restart.
call("echo", "Waiting for human approval...");
suspend;
// Execution picks up here after the operator resumes the agent
call("echo", "Approved. Continuing...");
```

---

## Architecture: Wasm-Sandboxed Inference Providers

Turn's `infer` primitive does **not** make HTTP calls directly. It delegates to a **WebAssembly plugin** loaded in a strict sandbox:

```
Turn VM (Host)
  │  LLM Request
  ▼
Wasm Driver (e.g., turn_provider_openai.wasm)
  │  → HTTP Config (URL, headers with $env:OPENAI_API_KEY template)
  ▼
Turn VM (Host)
  │  Substitutes real env vars, executes the HTTPS call
  ▼
Wasm Driver
  │  → Parses the HTTP response → structured Turn Value
  ▼
Turn VM (Host)
```

**Why this matters:**
- The `.wasm` driver is a 2MB file that runs on any OS — zero native binary distribution
- The driver **literally cannot** access your filesystem, network, or environment directly. It can only transform JSON strings
- API keys are injected by the host, never seen by the plugin code

See [PROVIDERS.md](PROVIDERS.md) for the full protocol spec and how to write your own driver.

---

## Official Providers

| Provider | Wasm Driver | Required Env Vars |
|---|---|---|
| Standard OpenAI | `turn_provider_openai.wasm` | `OPENAI_API_KEY`, `OPENAI_MODEL` |
| Azure OpenAI | `turn_provider_azure_openai.wasm` | `AZURE_OPENAI_ENDPOINT`, `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_DEPLOYMENT` |
| Azure Anthropic | `turn_provider_azure_anthropic.wasm` | `AZURE_ANTHROPIC_ENDPOINT`, `AZURE_ANTHROPIC_API_KEY` |

---

## Standard Library

| Module | Functions |
|---|---|
| `std/fs` | `read`, `write`, `exists` |
| `std/http` | `http_get`, `http_post` |
| `std/math` | `math.max`, `math.min`, `math.floor`, `math.sqrt` |
| `std/json` | `json.parse`, `json.stringify` |
| `std/time` | `time.now` |

---

## Project Layout

```
Turn/
├── README.md
├── PROVIDERS.md              ← Wasm driver protocol spec
├── ARCHITECTURE.md           ← Internal codebase guide
├── install.sh                ← One-line installer
├── impl/                     ← Rust bytecode VM
│   └── src/
│       ├── lexer.rs          ← Tokenizer
│       ├── parser.rs         ← Recursive-descent parser
│       ├── compiler.rs       ← AST → Bytecode
│       ├── vm.rs             ← Async executor (Tokio)
│       ├── wasm_host.rs      ← Wasmtime sandbox + HTTP delegation
│       ├── llm_tools.rs      ← infer instruction handler
│       ├── runtime.rs        ← HNSW semantic memory + WAL
│       ├── runner.rs         ← Host: tool dispatch, agent lifecycle
│       └── server.rs         ← HTTP server mode
├── providers/                ← Official Wasm inference drivers
│   ├── turn-provider-openai/
│   ├── turn-provider-azure-openai/
│   ├── turn-provider-azure-anthropic/
│   └── turn-provider-aws-anthropic/
└── editors/
    └── vscode/               ← VS Code extension
```

---

## Building from Source

```bash
# Prerequisites: Rust (https://rustup.rs/)
git clone https://github.com/ekizito96/Turn.git
cd Turn/impl
cargo build --release

# Run a script
./target/release/turn run examples/struct_infer.tn

# Build a specific Wasm provider
cd ../providers/turn-provider-openai
cargo build --target wasm32-unknown-unknown --release

# Start the LSP (for VS Code)
./target/release/turn lsp
```

---

## Reading Order

| Start here | Then |
|---|---|
| [Why Turn](https://turn-lang.dev/docs/why-turn) | [Installation](https://turn-lang.dev/docs/installation) |
| [The `infer` Primitive](https://turn-lang.dev/docs/inference) | [Inference Providers](https://turn-lang.dev/docs/providers) |
| [Memory & Context](https://turn-lang.dev/docs/memory) | [The Actor Model](https://turn-lang.dev/docs/concurrency) |
| [ARCHITECTURE.md](ARCHITECTURE.md) | [`impl/src/`](impl/src/) |

---

## License

Apache 2.0 — see [LICENSE](LICENSE).

---

<div align="center">

**Created by [Muyukani Ephraim Kizito](https://github.com/ekizito96)**  
Founder, [AI Dojo](https://ai-dojo.io) · [Prescott Data](https://prescottdata.io)

</div>
