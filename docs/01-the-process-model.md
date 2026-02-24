# The Process Model

In Turn, an agent is a **process** — not a loop, not an API endpoint, not a function. It has bounded context, persistent memory, a mailbox, and a lifecycle.

## What Is a Turn?

A Turn program executes as a stateful process. Every execution unit is a "turn" — an atomic, durable unit of agentic work. At any point in time, the full state of the agent is:

```
Config = (env, context, memory, mailbox, tool_registry, turn_state, program)
```

- **env** — lexically-scoped variable bindings (`let x = ...`)
- **context** — token-budgeted working knowledge (`context.append(...)`)
- **memory** — persistent semantic key-value store (`remember` / `recall`)
- **mailbox** — message queue for actor communication (`send` / `receive`)

## Suspension and Resumption

When the agent calls a tool or runs `infer`, the VM **suspends** — serializing the entire state tuple to durable storage. The tool runs. When the result arrives, the VM **resumes** from the saved state, injecting the result.

```
1. Run until: Complete, Error, or Suspend(tool_name, arg, continuation)
2. If Suspend:
   a. Persist the continuation
   b. Execute the tool (async, external, or human-in-the-loop)
   c. Resume: load continuation, inject result → goto 1
3. If Complete: return final value
```

This means an agent never blocks a thread. It is either running or suspended in state — scalable to thousands of concurrent agents on modest hardware.

## Everything Is an Actor

Every Turn process is an actor: isolated state, isolated context, isolated memory, isolated mailbox. Coordination happens only through message passing. No shared state.

```
spawn { ... }     → creates a child actor, returns its PID
send(pid, value)  → puts a value in pid's mailbox
receive()         → blocks (suspends) until a message arrives
link(pid)         → bidirectional crash propagation
monitor(pid)      → unidirectional crash observation
```

→ Next: [Inference](02-inference.md)
