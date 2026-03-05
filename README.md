# Turn Language

<p align="center">
  <a href="https://turn-lang.dev"><strong>Website & Documentation</strong></a> ·
  <a href="https://turn-lang.dev/#playground"><strong>Live Playground</strong></a>
</p>

Turn is a compiled systems programming language designed specifically for autonomous, multi-agent AI workflows. It treats Large Language Models (LLMs) as native computational units (ALUs) rather than external API endpoints.

Powered by a custom Rust bytecode Virtual Machine, Turn solves the inherent unreliability of bolting probabilistic LLMs onto deterministic languages like Python and TypeScript.

## Why Turn?

Current AI frameworks rely on massive layers of Pydantic models, JSON-parsing retry loops, and async spaghetti to coordinate agents. Turn fixes this at the compiler level with five core primitives:

1. **Cognitive Type Safety (`infer Struct`)**: Define a struct and call `infer`. The VM natively intercepts the schema constraints and guarantees the inference provider returns exactly the memory shape you asked for. No manual parsing required.
2. **State as Epochs (`..spread`)**: Turn enforces strict immutability. Instead of modifying objects (which causes race conditions in multi-agent systems), agents naturally evolve their state into immutable "epochs" using structural spread syntax (`let next = State { ..current }`).
3. **Iteration as Delegation (`spawn_each`)**: Turn intentionally lacks a `for` loop. If you have a list of tasks, you don't loop over them sequentially; you map over them concurrently. `spawn_each` distributes workloads across the VM's Actor model natively.
4. **Probabilistic Control Flow (`confidence`)**: LLMs hallucinate. Turn makes uncertainty a first-class citizen. Use the `confidence` operator to build native fail-safes directly into your logic (`if confidence decision < 0.85 { return Fallback; }`).
5. **Erlang-style Actors (`spawn_link` & `receive`)**: Agents run in isolated VM threads and communicate safely via deterministic mailboxes, ensuring perfect OODA-loop isolation.

## Features

*   **Custom Rust Bytecode VM**: Fast, sandboxed, stack-based execution.
*   **Provider Agnostic via WASM Drivers**: LLM providers are isolated as pre-compiled WebAssembly drivers in `.turn_modules/`. Turn ships with drivers for **Anthropic**, **OpenAI**, **Google Gemini**, **xAI Grok**, **Ollama**, **Azure OpenAI**, and **Azure Anthropic** out of the box. Route to any provider via a single environment variable (`TURN_LLM_PROVIDER`). No bloated SDKs. No vendor lock-in.
*   **Semantic Memory**: Built-in `remember` and `recall` for cross-session vector persistence.
*   **Native Standard Library**: HTTP, Regex, JSON parsing, and File System operations built directly into the bytecode execution loop.
*   **Native List Primitives**: `map(list, closure)` and `filter(list, closure)` are compiler-expanded into efficient inline bytecode — no imports, no libraries.

## Quick Start

### Installation

You can install the Turn CLI using Cargo:

```bash
cargo install --git https://github.com/ekizito96/Turn turn
```

### Writing Your First Agent

Create a file named `hello.tn`:

```turn
struct Sentiment { score: Num, reasoning: Str };

turn {
    let input = "I absolutely love building systems in Rust!";
    
    let result = infer Sentiment {
        "Analyze the sentiment of the following text: " + input
    };
    
    if confidence result < 0.8 {
        call("echo", "Low confidence. Fallback triggered.");
        return null;
    }
    
    call("echo", "Score: " + result.score);
    call("echo", "Reasoning: " + result.reasoning);
    
    return result;
}
```

### Running the Agent

Set your preferred provider and API key, then run the script:

```bash
# OpenAI
export TURN_LLM_PROVIDER=openai
export OPENAI_API_KEY=sk-your-api-key

# Anthropic
export TURN_LLM_PROVIDER=anthropic
export ANTHROPIC_API_KEY=sk-ant-your-key

# Google Gemini
export TURN_LLM_PROVIDER=gemini
export GEMINI_API_KEY=your-key

# xAI Grok
export TURN_LLM_PROVIDER=grok
export XAI_API_KEY=your-key

# Ollama (no API key needed)
export TURN_LLM_PROVIDER=ollama

# Azure OpenAI
export TURN_LLM_PROVIDER=azure_openai
export AZURE_OPENAI_KEY=your-key
export AZURE_OPENAI_ENDPOINT=https://your-resource.openai.azure.com
export AZURE_OPENAI_DEPLOYMENT=your-deployment-name

turn run hello.tn
```

## How Provider Routing Works

Turn uses a **WASM Driver Model** for all LLM inference. Each provider is a standalone `.wasm` file in `.turn_modules/`. When you call `infer`, the VM:

1. Looks up `{TURN_LLM_PROVIDER}_provider.wasm` in `.turn_modules/`.
2. Passes the structured Turn request into the WASM driver's `transform_request` function.
3. The driver returns an HTTP config (URL, headers, body) — the host VM makes the actual HTTP call.
4. The raw HTTP response is passed back into the driver's `transform_response` function.
5. The driver normalises it to a standard Turn response.

This means **Turn itself never needs to know about any LLM API**. If xAI changes their endpoint tomorrow, you update one 10KB `.wasm` file — the Turn compiler and VM are untouched.

### Writing a Custom Provider

You can ship your own provider (e.g. for a private model or a new public LLM) by implementing two functions in any language that compiles to WASM:

```rust
// Your driver must export exactly these two functions:
pub unsafe extern "C" fn transform_request(ptr: u32, len: u32) -> u64 { ... }
pub unsafe extern "C" fn transform_response(ptr: u32, len: u32) -> u64 { ... }
```

Compile to `wasm32-unknown-unknown`, drop the `.wasm` into `.turn_modules/`, set `TURN_LLM_PROVIDER=your_provider_name`, and Turn will pick it up automatically. See the `providers/` directory for reference implementations.

## Advanced Examples

Check out the `impl/examples/` directory for production-grade multi-agent workflows:
*   [**Algorithmic Trading Syndicate**](impl/examples/quant_syndicate.tn): Three autonomous agents (Technical, Sentiment, and Risk) debate a trade concurrently, and a Chairman executes the final decision.
*   [**Investment Committee**](impl/examples/investment_committee.tn): Specialist agents evaluate a live equity position concurrently using Yahoo Finance data.
*   [**Marketing Agency**](impl/examples/marketing_agency.tn): An SEO Specialist, Copywriter, and Creative Director collaborate to generate high-converting ad copy using Wikipedia research.

## Documentation

For a comprehensive guide to the language grammar, runtime model, and ecosystem bridges, visit the official documentation at [turn-lang.dev/docs](https://turn-lang.dev/docs).

## License

Turn is open-source software licensed under the [MIT License](LICENSE).
