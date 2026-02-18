# Error Handling (Alpha + Planned)

**Status:** Draft.

## 1. Philosophy: Robustness in Chaos

Agentic systems operate in a chaotic environment. Tools fail, APIs timeout, and LLMs hallucinate. A language for agents must treat **Failure as a First-Class Citizen**.

The "Physics of Failure" in Turn:
*   **Containment:** Errors should be trapped within a "blast radius" (the `try` block).
*   **Recovery:** Agents must have a mechanism to switch strategies upon failure.
*   **Propagation:** Unhandled errors bubble up, eventually crashing the agent if not caught (or notifying the orchestrator).

## 2. Syntax

We introduce `try`, `catch`, and `throw`.

```turn
try {
    let result = call("risky_tool", {});
    if result == null {
        throw "Tool returned null";
    }
    return result;
} catch (err) {
    call("log", "Recovering from error: " + err);
    return "default_value";
}
```

### `throw`
`throw <expr>` interrupts execution. The expression is evaluated to a `Value` (the error object).

### `try / catch`
*   `try` block is executed.
*   If successful, `catch` is skipped.
*   If an exception occurs (via `throw` or runtime error), execution jumps to `catch`.
*   The error value is bound to the variable named in `catch(...)`.

## 3. Semantics & VM Implementation

### The Exception Mechanism
We need a **Handler Stack**.

1.  **`Instr::PushHandler(offset)`**: Pushes a "Catch Handler" onto the current Frame's handler stack. The `offset` is the address of the `catch` block.
2.  **`Instr::PopHandler`**: Removes the top handler. Used when `try` completes successfully.
3.  **`Instr::Throw`**:
    *   Pops the top value from the data stack (the error).
    *   Walks up the `Frame` stack looking for an active Handler.
    *   **Unwinding:** If the current frame has no handlers, pop the frame (return) and check the caller.
    *   **Catching:** If a handler is found:
        *   Restore the VM state to that frame.
        *   Push the error value onto the stack.
        *   Jump to the handler address.

### Resilience & Persistence
The `handler_stack` must be part of `VmState` (serialized). If an agent crashes *physically* (machine dies), and we resume it, we must still be inside the `try` block logically.

## 4. Standard Library Integration
*   Runtime errors (e.g., "variable not found", "invalid index") should implicitly `throw`.
*   Currently, they return `null` or panic. We should upgrade them to `throw` eventually. For v1, `throw` is explicit.

## 5. Example: Retry Loop

```turn
let retries = 0;
while retries < 3 {
    try {
        let val = call("http_get", "https://api.com");
        return val;
    } catch (e) {
        retries = retries + 1;
        call("sleep", 1);
    }
}
throw "Max retries exceeded";
```
