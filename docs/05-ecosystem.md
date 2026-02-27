# Ecosystem Bridges

Turn connects to the existing ecosystem through three native primitives. None of them require an SDK, a package install, or a runtime dependency.

## Compile-Time Schema Adapters

`use schema::*` resolves a remote schema at **compile time** using a sandboxed Wasm macro. The macro runs once. The generated closures are native Turn code at runtime.

```turn
// GraphQL: fetches introspection schema at compile time, generates native Turn structs and tools
let gh = use schema::graphql("https://api.github.com/graphql");

// Swagger/REST: generates a tool closure per API path and operation
let stripe = use schema::swagger("https://raw.githubusercontent.com/stripe/openapi/master/openapi/spec3.json");

// gRPC: generates Turn structs from message definitions and closures from rpc definitions
let grpc = use schema::grpc("proto://billing.proto");

// FHIR: generates Turn structs and CRUD closures for each resource type
let ehr = use schema::fhir("https://fhir.example.com/metadata");
```

At runtime these are plain Turn closures. The LLM calls them natively via `infer with [tools]`. No HTTP. No JSON parsing. No SDK.

The Wasm macro runs in a `wasm32-unknown-unknown` sandbox. It cannot touch your filesystem, network, or secrets.

## The `mcp()` Bridge

For teams already running MCP servers, Turn can spawn them as stdio subprocesses:

```turn
// Spawns the subprocess and returns McpServer { pid, status }.
let legacy = mcp("stdio://npx @modelcontextprotocol/server-stripe");
```

`mcp()` is a migration bridge. Once your team rewrites the MCP server logic in Turn, replace the `mcp()` call with `use schema::openapi(...)` and the subprocess overhead disappears.

## CLI Domestication (`sys_exec`)

Instead of letting an LLM compose shell strings, `sys_exec` forces strict argument separation:

```turn
// Each argument is a separate typed Str. No shell is invoked. Injection is structurally impossible.
let output = call("sys_exec", {
    "bin":  "python3",
    "arg1": "analyze.py",
    "arg2": user_input_str
});
```

The VM calls `Command::new("python3").args(["analyze.py", user_input_str])` directly. There is no string to inject into.

## Design Tradeoffs

| Mechanism | When to use | Overhead |
|---|---|---|
| `use schema::*` adapter | Stable public API with a schema URL | Zero at runtime |
| `mcp()` bridge | Existing MCP server you operate | 1 subprocess per agent |
| `sys_exec` | Legacy script or binary that does one thing | 1 child process per call |

Back: [Actors](04-actors.md)
