# Empirical Analysis: The Physics of Agentic Software (2026)

**Status:** Research Note
**Date:** Feb 17, 2026
**Author:** Turn Research Group

## 1. Introduction

To validate the Design Mandate of Turn, we conducted a deep audit of production-grade agentic systems (a large-scale integration agent and an autonomous research agent) currently deployed in the field. The goal was to identify the "dark matter" of AI engineering—the invisible, heavy lifting that developers do to make agents work in Python/TypeScript.

Our findings confirm that the current state of agent engineering is akin to "pre-compiler" computing: developers are manually managing memory addresses (context windows), instruction cycles (turns), and interrupt vectors (tool calls).

## 2. The "Context Manager" Anomaly

In the integration agent codebase, we found a file named `ContextManager.py`. It is over 500 lines of code. Its sole purpose is to manage the text buffer fed to the LLM.

**What it does manually:**
1.  **Token Counting**: Imports `tiktoken` to count tokens byte-by-byte.
2.  **Priority Stacking**: Implements a complex heuristic to decide what to keep: `Mission > Plan > Scratchpad > Research > History`.
3.  **Manual Truncation**: Strings are sliced with `[:char_limit]` and `...[truncated]` markers.
4.  **Auto-Summarization**: An async loop triggers an LLM call to compress history when a threshold (0.8) is reached.
5.  **Sensitive Data Scrubbing**: Hardcoded lists of keys (`password`, `api_key`) are scrubbed from the string.

**The Turn Solution:**
In Turn, this entire 500-line class dissolves into the **Context Primitive**.
- **Bounded by Definition**: The runtime enforces `|context| <= N`.
- **Heap vs Stack**: The "Priority Stack" suggests that Turn's `Context` should not just be a flat list, but perhaps structured (e.g., `context.mission`, `context.working`, `context.history`). *Update to spec needed?*
- **Security**: Secure memory values should never be appendable to context without explicit declassification.

## 3. The "God Class" Kernel

The agent kernel file is 3,464 lines long. It implements the "Agent Loop."

**What it does manually:**
1.  **The Loop**: A `while` loop that checks for `MAX_TURNS`.
2.  **State Smearing**: State is spread across `RedisContextStore`, `IntegrationState` (an object passed everywhere), and local variables.
3.  **Tool Dispatch**: A massive switch/lookup mechanism to call tools and handle their outputs.
4.  **Fake Suspension**: Using `async/await` to simulate pausing for tool results, but actually blocking the Python event loop's logical flow for that agent.

**The Turn Solution:**
- **Turn Primitive**: The `turn { ... }` block replaces the loop.
- **Object Orientation**: State is encapsulated in the `Agent` instance, not a global `State` dictionary passed around.
- **True Suspension**: The VM suspends on tool calls, allowing the host to persist the agent to disk (or database) and resume days later.

## 4. The "Orchestrator" Pattern

In the autonomous research agent, we see `MissionOrchestrator` manually managing a list of `SubMission` objects.

**Observation:**
Agents often need to spawn "child" tasks. The current Python approach is to have a "Commander" agent write a JSON list of tasks, and then a Python `for` loop iterates over them to spawn "Scout" agents.

**The Turn Solution:**
Turn v1 is single-agent, but this finding validates the need for **Multi-Agent** primitives in v2. We need `spawn(AgentClass, args)` to be a first-class operation.

## 5. Conclusion & Course Correction

The empirical data validates the **Design Mandate**. The industry is suffering from "Retrofit Fatigue."

**Course Corrections for Turn:**
1.  **Structured Context**: The "Priority Stack" finding suggests that `context.append()` might be too simple. Real agents need "pinned" context (Mission) vs "sliding" context (History).
    *   *Action*: Consider `context.pin(val)` vs `context.append(val)`.
2.  **Secure Memory**: The "scrubbing" logic confirms that `Memory` needs a `Secret` type that cannot be implicitly stringified into the Context.
3.  **Goal Primitive**: Every agent observed has a "Mission" or "Objective". This is distinct from Context. We should elevate `Goal` to a primitive.
