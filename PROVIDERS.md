# Turn Inference Provider Protocol

Turn's `infer` primitive is **provider-agnostic**. The core VM never makes HTTP calls directly to LLM APIs. Instead, it embeds a secure WebAssembly (Wasm) runtime (`wasmtime`) to execute specialized **Wasm drivers**.

## Architecture

```
Turn VM (Host)
  │  (loads provider `.wasm` file into sandboxed memory)
  ▼
$TURN_INFER_PROVIDER (Wasm Driver)
  │  (transform_request: Turn AST -> HTTP Config)
  ▼
Turn VM (Host)
  │  (securely executes actual HTTPS request locally)
  ▼
$TURN_INFER_PROVIDER (Wasm Driver)
  │  (transform_response: HTTP Response -> Turn AST)
  ▼
Turn VM (Host)
```

The Wasm driver is purely computational and **cannot** access the network or filesystem. The dual-pass sandbox ensures absolute portability and security.

## Selecting a Provider

Set the `TURN_INFER_PROVIDER` environment variable to the absolute path of your compiled `.wasm` provider before running Turn:

```bash
# Use the official standard OpenAI provider
export TURN_INFER_PROVIDER=~/.turn/providers/turn_provider_openai.wasm
turn run script.tn
```

If unset, Turn defaults to searching for `turn_provider_openai` in its known module paths.

---

## Provider API Contract

A provider must be compiled as a `wasm32-unknown-unknown` module exporting three specific C-ABI functions.

### 1. Memory Management

```rust
#[no_mangle]
pub extern "C" fn alloc(len: u32) -> u32
```
Allocates `len` bytes of memory and returns a pointer. The Turn host uses this to write JSON strings into the Wasm module's space.

### 2. Request Transformation

```rust
#[no_mangle]
pub unsafe extern "C" fn transform_request(ptr: u32, len: u32) -> u64
```
Takes a Turn Inference Request JSON string and returns an HTTP Configuration JSON string. The 64-bit return value packs the memory pointer (upper 32 bits) and string length (lower 32 bits).

**Input JSON (Turn Request):**
```json
{
  "jsonrpc": "2.0",
  "method": "infer",
  "id": 1,
  "params": {
    "prompt": "user string",
    "schema": { ... },
    "context": ["episodic memory 1", "episodic memory 2"],
    "tools": []
  }
}
```

**Output JSON (HTTP Config):**
```json
{
  "url": "https://api.openai.com/v1/chat/completions",
  "method": "POST",
  "headers": {
    "Authorization": "Bearer $env:OPENAI_API_KEY",
    "Content-Type": "application/json"
  },
  "body": { ... }
}
```
*Note: The Host automatically resolves `$env:VARIABLE_NAME` references from the real environment before making the request, keeping API keys out of compiled Wasm.*

### 3. Response Transformation

```rust
#[no_mangle]
pub unsafe extern "C" fn transform_response(ptr: u32, len: u32) -> u64
```
Takes the result of the HTTP execution and converts it back into a standard Turn AST JSON-RPC response.

**Input JSON (Host Response):**
```json
{
  "status": 200,
  "headers": { ... },
  "body": "{\"choices\": [ ... ]}"
}
```

**Output JSON (Turn Result):**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": { ... structured data ... }
}
```

---

## Official Providers

All official providers found in `Turn/providers/` compile to `.wasm`:

| Crate | Provider | Required Env Vars |
|---|---|---|
| `turn-provider-openai` | Standard OpenAI (`api.openai.com`) | `OPENAI_API_KEY`, `OPENAI_MODEL` |
| `turn-provider-azure-openai` | Azure OpenAI | `AZURE_OPENAI_ENDPOINT`, `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_DEPLOYMENT` |
| `turn-provider-azure-anthropic` | Anthropic via Azure AI Foundry | `AZURE_ANTHROPIC_ENDPOINT`, `AZURE_ANTHROPIC_API_KEY`, `AZURE_ANTHROPIC_MODEL` |
| `turn-provider-aws-anthropic` | Anthropic Claude via AWS Bedrock | *Work in Progress (SigV4 Wasm Timestamp Limitation)* |

---

## Security Notes

- **Absolute Sandbox**: Wasm drivers run in `wasmtime` without WASI. They literally cannot execute system calls, read files, or open sockets.
- **Provider Agnostic**: The architecture allows community members to build Inference drivers for local models (e.g., Llama.cpp servers, Ollama) by simply writing a request translator that compiles to Wasm.
- **Microsecond Cold Starts**: A 2MB `.wasm` driver can be instanced locally per `infer` instruction with zero noticeable overhead, compared to spawning OS subprocesses.
