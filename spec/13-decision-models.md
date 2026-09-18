# Decision model effects

**Status:** Preview. This document defines the implemented provider-neutral boundary for bounded probabilistic decisions. The runtime effect and TypeSafe driver are operational; statically derived answer types remain future work.

## Purpose

`infer` and `decide` represent different effects:

- `infer Type { prompt; }` requests open-ended synthesis constrained to a Turn type.
- `decide(state, questions)` requests bounded judgments whose possible outputs are defined in advance.

The distinction lets the runtime route generative and decision workloads to different model classes while preserving Turn's durable effect semantics.

## Syntax

```turn
let result = decide(state, questions);
```

`state` may be any serializable Turn value. `questions` is a map keyed by application-defined question identifiers. Question schemas are provider capabilities rather than language keywords; this keeps the Turn core independent of any vendor or model taxonomy.

## Runtime semantics

Evaluation proceeds as follows:

1. Evaluate `state` and `questions` from left to right.
2. Suspend with the effect name `ai_decide`.
3. Pass `{ state, questions, context }` to the configured decision provider.
4. Persist the continuation and pending decision request before external execution, using the same durability boundary as other tools.
5. Resume with the provider-normalized structured result.

If execution restarts while the request is pending, Turn replays it with at-least-once delivery. Decision requests are read-only in the normal case, but surrounding side effects must still be designed for replay.

The default decision provider is selected with `TURN_DECISION_PROVIDER`. Provider-specific credentials, model names, request translation, and response normalization belong in WASM drivers, not in the compiler or VM. Official drivers are bundled with Turn; a matching file in `.turn_modules/` overrides the bundled version.

## TypeSafe driver

The `typesafe` driver maps the generic effect to TypeSafe's System One endpoint. It supports mixed `choice`, `score`, and `noul` questions in one request and preserves answer probabilities, confidence values, model metadata, and token usage.

```bash
export TURN_DECISION_PROVIDER=typesafe
export TYPESAFE_API_KEY=your-key
turn doctor
turn run examples/jev_triage.tn
```

The driver defaults to `jev-latest`. Set `TYPESAFE_MODEL` to select another compatible System One model.

## Current typing boundary

The alpha surface types the result of `decide` as `Any` because one call may contain heterogeneous question types. A later typed surface may derive a result type from statically declared question definitions. This limitation is explicit: the runtime preserves structured results today without pretending that dynamic question maps have compile-time field guarantees.