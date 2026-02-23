# Turn Inference Provider Protocol

Turn's `infer` primitive is **provider-agnostic**. The core VM never makes HTTP calls directly. Instead it spawns an external *provider binary* over `stdio` using JSON-RPC 2.0. This document defines the protocol contract.

## Architecture

```
Turn VM
  │  (spawns on first infer call)
  ▼
$TURN_INFER_PROVIDER  ← any binary in PATH
  │  (writes JSON-RPC requests to stdin)
  │  (reads JSON-RPC responses from stdout)
  ▼
LLM API   ← the provider owns ALL HTTP
```

The provider binary is responsible for:
- Reading one JSON-RPC request per line from `stdin`
- Making any necessary HTTP calls to the LLM API
- Writing one JSON-RPC response per line to `stdout`

## Selecting a Provider

Set the `TURN_INFER_PROVIDER` environment variable to the name or absolute path of your provider binary before running Turn:

```bash
# Use the official standard OpenAI provider
TURN_INFER_PROVIDER=turn-provider-openai turn run script.tn

# Use a custom in-house provider
TURN_INFER_PROVIDER=/opt/my-org/turn-provider-grok turn run script.tn
```

If unset, Turn defaults to `turn-provider-openai`.

---

## JSON-RPC Protocol

### Request (VM → Provider)

The Turn VM sends one JSON-RPC 2.0 request per `infer` evaluation:

```json
{
  "jsonrpc": "2.0",
  "method": "infer",
  "id": 1,
  "params": {
    "prompt":  "string — the full assembled prompt sent to the LLM",
    "schema":  { ... } | null,
    "tools":   [ ... ] | [],
    "context": [ "string", ... ] | []
  }
}
```

| Field | Type | Description |
|---|---|---|
| `prompt` | `string` | The assembled user message / task description |
| `schema` | `object \| null` | JSON Schema the LLM response must conform to. `null` means free-form text. |
| `tools` | `array` | Tool definitions (OpenAI tool format). Empty if no native tools are used. |
| `context` | `array<string>` | Ordered episodic memory context strings from `remember` / `recall` |

### Response: Success (Provider → VM)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": <value>
}
```

- If `schema` was an object型, `result` **must** be a JSON object matching the schema.
- If `schema` was `null`, `result` should be a JSON string.

### Response: Error (Provider → VM)

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": "Human-readable error message"
}
```

Turn will surface the error message to the running script as a `Result::Err`.

### Response: Tool Call (Provider → VM)

If the LLM requests a native tool call, the provider should emit a `tool_call` method back to the VM:

```json
{
  "jsonrpc": "2.0",
  "method": "tool_call",
  "id": 1,
  "params": {
    "name": "tool_name",
    "arguments": "{\"key\": \"value\"}"
  }
}
```

> **Note:** Tool call looping (provider receives the result and re-prompts) is not yet fully implemented in the VM. Providers may emit a single tool_call and then terminate the exchange.

---

## Official Providers

| Binary | Provider | Required Env Vars |
|---|---|---|
| `turn-provider-openai` | Standard OpenAI (`api.openai.com`) | `OPENAI_API_KEY`, `OPENAI_MODEL` (default: `gpt-4o`) |
| `turn-provider-azure-openai` | Azure OpenAI | `AZURE_OPENAI_ENDPOINT`, `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_DEPLOYMENT` (default: `gpt-4o`) |
| `turn-provider-azure-anthropic` | Anthropic via Azure AI Foundry | `AZURE_ANTHROPIC_ENDPOINT`, `AZURE_ANTHROPIC_API_KEY`, `AZURE_ANTHROPIC_MODEL` (default: `claude-3-5-sonnet`) |
| `turn-provider-aws-anthropic` | Anthropic Claude via AWS Bedrock | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION` (default: `us-east-1`), `BEDROCK_MODEL_ID` (default: `anthropic.claude-3-5-sonnet-20241022-v2:0`) |

---

## Writing Your Own Provider

Any binary that follows the protocol above is a valid Turn inference provider. You can write one in any language.

**Minimal Python example:**

```python
#!/usr/bin/env python3
import sys, json

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    if req.get("method") == "infer":
        prompt = req["params"]["prompt"]
        result = your_llm_call(prompt)  # implement this
        print(json.dumps({"jsonrpc": "2.0", "id": req["id"], "result": result}), flush=True)
```

Then use it:

```bash
TURN_INFER_PROVIDER=./my_provider.py turn run script.tn
```

---

## Security Notes

- Providers run as a child process of the Turn VM. They inherit the same OS user and environment variables.
- Never log or persist user prompts or API keys inside a provider binary.
- For cloud/Playground deployments, run providers inside an ephemeral, network-isolated container.
