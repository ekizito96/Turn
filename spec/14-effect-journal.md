# Durable effect journal

**Status:** Implemented after the Turn 2.0 preview. This document defines effect identity, outcome persistence, and replay behavior.

## Record model

Every externally executed effect has a durable record:

```text
EffectRecord = {
  effect_id,
  tool_name,
  arg,
  outcome: Success(value, cost) | Failure(error),
  completed_at_ms
}
```

Effect IDs contain an execution ID, process ID, and monotonically increasing process-local sequence. Restored checkpoints retain the execution ID and pending effect ID. Starting a new run after completion creates a new execution ID, so prior journal records cannot be mistaken for current results.

## Commit order

For normal external effects the runner performs:

1. Persist the VM continuation with its pending effect request.
2. If the effect ID already has a completed record, skip execution and inject that outcome.
3. Otherwise execute the tool or model provider.
4. Atomically persist the outcome record.
5. Clear the pending marker and resume the VM with the recorded value or error.

If the process crashes after step 4, restart follows step 2 and does not invoke the effect again.

## Guarantee boundary

The journal prevents duplicate local invocation after a completed record is durable. It does not provide a distributed transaction with the external system. A crash after the external system accepts an operation but before step 4 may repeat the request. Non-idempotent external APIs must deduplicate using the Turn effect ID or an application-provided idempotency key.

Effect-aware host integrations can receive that ID directly without changing existing tool handlers:

```rust
tools.register_effectful(
  "open_pull_request",
  Box::new(|context, arg| {
    github.open_pull_request(arg, &context.effect_id)
  }),
);
```

`ToolRegistry::register` remains compatible with handlers that only need the Turn argument. `register_effectful` is opt-in for systems that support idempotency, tracing, or request correlation.

## FileStore layout

The default store writes:

```text
<store>/<agent>.json
<store>/<agent>.effects/<effect-id>.json
```

Agent and effect IDs are encoded as safe path components. Checkpoints and records are written to a temporary file, flushed, synchronized, and atomically renamed. Conflicting records for the same effect ID are rejected.

State checkpoints are removed on completed or uncaught-error runs. Effect records remain available for audit.

## Inspection

```bash
turn effects <agent-id>
turn effects <agent-id> --json
turn effects <agent-id> --json --show-payloads
```

Human and JSON output hide arguments and outcomes unless `--show-payloads` is supplied. Operators must protect journal storage because payload mode can expose prompts, source code, tool arguments, and model results.