# Turn Language

<p align="center">
  <a href="https://turn-lang.dev"><strong>Website & Documentation</strong></a> ·
  <a href="https://turn-lang.dev/#playground"><strong>Live Playground</strong></a>
</p>

Turn is a compiled systems programming language designed specifically for autonomous, multi-agent AI workflows. It treats Large Language Models (LLMs) as native computational units (ALUs) rather than external API endpoints.

Powered by a custom Rust bytecode Virtual Machine, Turn solves the inherent unreliability of bolting probabilistic LLMs onto deterministic languages like Python and TypeScript.

## Why Turn?

Current AI frameworks rely on massive layers of Pydantic models, JSON-parsing retry loops, and async spaghetti to coordinate agents. Turn fixes this at the compiler level with three core primitives:

1. **Cognitive Type Safety (`infer Struct`)**: Define a struct and call `infer`. The VM natively intercepts the schema constraints and guarantees the inference provider returns exactly the memory shape you asked for. No manual parsing required.
2. **Probabilistic Routing (`confidence`)**: LLMs hallucinate. Turn makes uncertainty a first-class citizen. Use the `confidence` operator to build native fail-safes directly into your control flow (e.g., `if confidence decision < 0.85 { return Fallback; }`).
3. **Erlang-style Actors (`spawn_link` & `receive`)**: Multi-agent orchestration in Python is a race-condition nightmare. Turn uses an Actor model. Agents run in isolated VM threads (`spawn_link`) and communicate safely via deterministic mailboxes (`receive`).

## Features

*   **Custom Rust Bytecode VM**: Fast, sandboxed, stack-based execution.
*   **Provider Agnostic**: Natively routes to Anthropic, Azure OpenAI, standard OpenAI, Google Gemini, xAI Grok, and Ollama via a single environment variable (`TURN_LLM_PROVIDER`). No bloated SDKs required.
*   **Semantic Memory**: Built-in `remember` and `recall` for cross-session vector persistence.
*   **Native Standard Library**: HTTP, Regex, JSON parsing, and File System operations built directly into the bytecode execution loop.

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
export TURN_LLM_PROVIDER=openai
export OPENAI_API_KEY=sk-your-api-key
turn run hello.tn
```

## Advanced Examples

Check out the `impl/examples/` directory for production-grade multi-agent workflows:
*   [**Algorithmic Trading Syndicate**](impl/examples/quant_syndicate.tn): Three autonomous agents (Technical, Sentiment, and Risk) debate a trade concurrently, and a Chairman executes the final decision.
*   [**Investment Committee**](impl/examples/investment_committee.tn): Specialist agents evaluate a live equity position concurrently using Yahoo Finance data.
*   [**Marketing Agency**](impl/examples/marketing_agency.tn): An SEO Specialist, Copywriter, and Creative Director collaborate to generate high-converting ad copy using Wikipedia research.

## Documentation

For a comprehensive guide to the language grammar, runtime model, and ecosystem bridges, visit the official documentation at [turn-lang.dev/docs](https://turn-lang.dev/docs).

## License

Turn is open-source software licensed under the [MIT License](LICENSE).