# 11. Ecosystem Bridges

Turn's goal is to absorb the existing ecosystem without asking developers to rewrite anything. Three native mechanisms enable this: Wasm Compile-Time Schema Adapters, the `mcp()` Legacy Bridge, and the `sys_exec` CLI Boundary.

---

## 11.1 Wasm Compile-Time Schema Adapters

### Motivation

Every existing API framework (REST/OpenAPI, GraphQL, gRPC, FHIR) ships its own SDK ecosystem. Integrating them traditionally requires:
1. Installing a language-specific SDK package.
2. Reading API documentation and writing wrapper code.
3. Exposing that wrapper to your LLM tool list.

This creates SDK bloat, stale bindings, and hidden runtime costs. Turn inverts the model entirely.

### How It Works

A schema adapter is a Rust crate compiled to `wasm32-unknown-unknown`. At **compile time**, the Turn compiler:

1. Fetches the remote schema URL.
2. Loads the appropriate `.wasm` adapter in a Wasmtime sandbox.
3. Calls the adapter's `expand_schema(input_ptr, len)` FFI.
4. Receives a JSON array of Turn AST nodes (`StructDef`, `Let`, etc.).
5. Inlines those nodes into the program as if they were hand-written Turn code.

```
Turn Compiler
  |
  |  use schema::graphql("https://api.github.com/graphql")
  v
graphql_adapter.wasm (Wasmtime sandbox)
  |  expand_schema(introspection_json) -> AST JSON
  v
Turn Compiler
  |  Inlines struct + tool definitions into the program
  v
Normal bytecode compilation proceeds
```

Zero runtime overhead. The schema lookup happens once at compile time. At runtime, the LLM calls native Turn closures.

### Supported Adapters

| Keyword | Adapter | Input Format |
|---|---|---|
| `use schema::openapi("url")` | `openapi_adapter.wasm` | OpenAPI 3.x JSON |
| `use schema::graphql("url")` | `graphql_adapter.wasm` | GraphQL Introspection JSON |
| `use schema::swagger("url")` | `swagger_adapter.wasm` | Swagger v2 JSON |
| `use schema::grpc("url")` | `grpc_adapter.wasm` | Protocol Buffer text |
| `use schema::fhir("url")` | `fhir_adapter.wasm` | FHIR Conformance Statement JSON |

### Example

```turn
// Compile-time: fetches GraphQL schema, generates native Turn types and tools.
let gh = use schema::graphql("https://api.github.com/graphql");

// Runtime: pure Turn function call. No HTTP. No JSON parsing.
let result = infer gh.SearchRepositories with [gh.searchRepos] {
    "Find the top 5 Rust repositories by stars.";
};
```

### Wasm Sandbox Security

Because adapters compile to `wasm32-unknown-unknown` (no WASI), they:
- Cannot open network connections.
- Cannot read or write the filesystem.
- Cannot inspect environment variables or secrets.
- Fault in a trapped, isolated address space.

---

## 11.2 The `mcp()` Legacy Bridge

### Motivation

The Model Context Protocol (MCP) represents a significant existing investment across thousands of servers and tools. Turn does not displace MCP. It absorbs it.

### Syntax

```turn
// Spawns an MCP server subprocess via stdio JSON-RPC.
let legacy = mcp("stdio://npx @modelcontextprotocol/server-stripe");
```

### VM Execution

When `Instr::McpStart` executes:

1. The URL scheme is validated (`stdio://` is the only supported scheme).
2. The binary and arguments are extracted from the URL path.
3. The VM spawns a child OS process using `std::process::Command`.
4. A `McpServer` struct is pushed to the stack:

```turn
// Equivalent Turn value:
McpServer {
    pid: 12345,
    status: "active"
}
```

The spawned process lifecycle is tied to the owning agent. When the agent terminates (normally or by fault), the subprocess is orphaned and will be reaped by the OS.

### Design Invariant

`mcp()` is a runtime domestication bridge, not a compile-time adapter. It is the migration path for teams that already run MCP servers. Once a team migrates their server logic to Turn, the `mcp()` call is replaced with a native `use schema::*` adapter, and the subprocess overhead disappears.

---

## 11.3 CLI Domestication (`sys_exec`)

### Motivation

LLMs hallucinating `bash -c` commands or composing strings with shell metacharacters is a critical security failure mode. Turn eliminates it structurally.

### Syntax

```turn
let result = call("sys_exec", {
    "bin":  "python3",
    "arg1": "process_data.py",
    "arg2": input_file,
    "arg3": output_dir
});
```

### Enforcement Guarantees

The `sys_exec` handler in `ToolRegistry`:

1. **Requires a `Map` argument.** A raw string or list causes an immediate typed error.
2. **Requires every map value to be `Str`.** Numbers, booleans, or nested structures cause an immediate typed error.
3. **Never invokes a shell.** `Command::new(binary).args(cmd_args)` is called directly with no shell interpolation.
4. **Returns `Str` (stdout) on success** or a Turn error string on non-zero exit.

This means an LLM physically cannot produce a shell injection through `sys_exec`. There is no string to inject into. Each token of the OS command is a separate, typed argument.

### Wrong (impossible in Turn)

```turn
// This requires a shell. sys_exec does not use a shell.
// Even if the LLM generated this, the VM rejects it at the type boundary.
call("sys_exec", "rm -rf /; echo done");  // ERROR: Str is not Map
```

### Right

```turn
call("sys_exec", {
    "bin":  "python3",
    "arg1": "analyze.py",
    "arg2": user_provided_input  // Str. Safe. Passed as a positional arg, not a shell string.
});
```

---

## 11.4 Design Invariants

| Bridge | Execution Time | Overhead at Agent Runtime | Security |
|---|---|---|---|
| Wasm Schema Adapters | Compile time | Zero | Sandboxed, no I/O |
| `mcp()` bridge | Runtime | OS subprocess per call | Bounded by stdio |
| `sys_exec` | Runtime | Child process per call | No shell; strict arg types |
