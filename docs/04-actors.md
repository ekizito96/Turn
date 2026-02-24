# Actors and Multi-Agent Systems

Turn's concurrency model is Erlang-style actors. Every agent is an isolated process. Coordination is purely via message passing. No shared state.

## Creating Agents: `spawn`

```turn
let pid = spawn {
    let task = receive();
    return infer Summary { task; };
};
```

`spawn` creates a new concurrent process and returns its PID. The spawned agent runs immediately on the Tokio scheduler in parallel.

## Message Passing: `send` and `receive`

```turn
// Send a message to an agent's mailbox (non-blocking)
send(pid, "Analyze the Q4 report.");

// Wait for a reply in your own mailbox (suspends process)
let reply = receive();
```

## Supervision: `link` and `monitor`

```turn
// Bidirectional — if either process crashes, the other gets an EXIT message
link(pid);

// Unidirectional — if pid exits, this process gets a DOWN message (without crashing)
monitor(pid);
```

## A Real Multi-Agent Example

```turn
struct Vote { decision: Str, confidence: Num };

// Spawn three specialist agents
let cfo = spawn {
    return infer Vote { "CFO: evaluate financial risk of: " + receive(); };
};
let cto = spawn {
    return infer Vote { "CTO: evaluate technical feasibility of: " + receive(); };
};
let cmo = spawn {
    return infer Vote { "CMO: evaluate market opportunity of: " + receive(); };
};

// Distribute work
let proposal = "Acquire a 22-person AI startup for $12M.";
send(cfo, proposal);
send(cto, proposal);
send(cmo, proposal);

// Collect results
let cfo_vote = receive();
let cto_vote = receive();
let cmo_vote = receive();

call("echo", "CFO: " + cfo_vote.decision);
call("echo", "CTO: " + cto_vote.decision);
call("echo", "CMO: " + cmo_vote.decision);
```

## Remote Actors

```turn
// Spawn an agent on a remote node over TCP
let remote = spawn_remote("192.168.1.42:9001", {
    return infer HeavyAnalysis { receive(); };
});

send(remote, my_dataset);
let result = receive();
```

---

→ [Back to README](../README.md) | [Full Documentation](https://turn-lang.dev/docs)
