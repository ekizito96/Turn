# Inference

`infer` is the most important statement in Turn. It is a **VM trap** — it suspends the process, delegates computation to a sandboxed Wasm provider, validates the LLM response against a compile-time JSON Schema, and resumes with a typed value.

## Basic Form

```turn
struct Sentiment {
    score: Num,
    label: Str,
    reasoning: Str
};

let result = infer Sentiment { "Analyze: I love this product!"; };

// result.score, result.label, result.reasoning are typed — no parsing needed
call("echo", result.label);
```

## Cognitive Type Safety

When you write `infer Sentiment { ... }`, the compiler:
1. Generates a full **JSON Schema** from the `Sentiment` struct at compile time
2. Embeds it in the bytecode as an `Infer` instruction
3. The schema is sent to the LLM provider as a `response_format` constraint
4. The VM validates the response against the schema before binding it to your variable

If the LLM response does not match the schema, the VM does not silently produce garbage — it returns an error that you can handle.

## Free-Form Inference

```turn
let summary = infer { "Summarize this document in 3 bullet points: " + doc; };
// summary is a raw Str
```

## Context Enrichment

The VM automatically prepends your `context.append()` stack to every `infer` call. It also performs Semantic Auto-Recall: before each `infer`, it queries the HNSW memory index for the most relevant past memories and injects them.

## The Wasm Provider

`infer` does **not** make HTTP calls. It delegates to a `.wasm` driver that:
1. Receives the Turn request JSON
2. Returns an HTTP Config (URL, headers with `$env:API_KEY` templates, body)
3. The VM Host substitutes real env vars and executes the call
4. The driver receives the HTTP response and converts it to a typed Turn value

The Wasm driver **cannot** access the network, filesystem, or your API keys directly.

→ Next: [Memory and Context](03-memory-and-context.md)
