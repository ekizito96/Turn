# Turn 2.0 Decision Models Preview

## Positioning

LLMs generate. Decision models judge. Turn coordinates both as durable effects.

Turn 2.0 introduces `decide(state, questions)`, a provider-neutral effect for bounded probabilistic decisions. TypeSafe Jev is the first supported decision-model backend. Generative `infer` remains separate.

## Reproducible demo

No API key:

```bash
git clone https://github.com/ekizito96/Turn.git
cd Turn
TURN_DECISION_PROVIDER=mock cargo run -- run examples/jev_triage.tn
```

Live Jev:

```bash
export TURN_DECISION_PROVIDER=typesafe
export TYPESAFE_API_KEY=your-key
cargo run -- doctor
cargo run -- run examples/jev_triage.tn
```

## X post

Turn now treats generative inference and bounded AI decisions as different runtime effects.

`infer` is for synthesis.
`decide` is for Choice, Score, and Noul judgments.
Turn owns the durable state, control flow, tools, actors, and recovery around both.

TypeSafe Jev is the first `decide` backend, delivered as a replaceable WASM provider.

LLMs generate. Decision models judge. Turn coordinates both.

[demo video]
https://github.com/ekizito96/Turn

## Suggested thread

1. Models are specializing. Asking one general-purpose LLM to generate, classify, score, route, verify, and orchestrate everything is no longer the only architecture.
2. Turn 2.0 separates open-ended `infer` from bounded `decide`. They are distinct durable effects and can use different providers.
3. A single `decide` call can evaluate Choice, Score, and Noul questions against shared state. Turn then handles confidence gates, tools, human escalation, actors, checkpoints, and replay.
4. Jev is the first decision backend, not a dependency baked into the language. Official providers ship as WASM adapters and project-local adapters can override them.
5. The demo runs without an API key through Turn's deterministic mock, then switches to live Jev through environment configuration only.

## Demo recording sequence

```bash
cargo install --git https://github.com/ekizito96/Turn turn
turn doctor
TURN_DECISION_PROVIDER=mock turn run examples/jev_triage.tn
export TURN_DECISION_PROVIDER=typesafe
export TYPESAFE_API_KEY=your-key
turn doctor
turn run examples/jev_triage.tn
```

Show the source beside the terminal. Keep the recording under 45 seconds and focus on the unchanged Turn program when switching from mock to Jev.

## Claims to avoid

- Do not repeat TypeSafe's speed, cost, calibration, or accuracy numbers as Turn benchmarks.
- Do not say Jev is deterministic or cannot make incorrect decisions.
- Do not describe `decide` results as statically typed yet; the preview result type is currently `Any`.
- Do not claim OpenAPI adapters are production-ready.
- Do not claim distributed execution; current actor scheduling is local to the Turn runtime.

## Release checklist

- Run all implementation and provider tests.
- Run Clippy with warnings denied.
- Build all provider WASM modules.
- Package and install Turn outside the repository.
- Record the mock demo first, then the live Jev demo.
- Verify no API key or customer data appears in terminal output or media.
- Tag `v2.0.0` only after the live Jev request succeeds.
