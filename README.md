# Turn

**A new, object-oriented programming language for agentic software.** Turn is not a retrofit. The primary abstraction is the **agent**—an object with context, memory, and the ability to run turns and call tools. We solve the pains of building agentic systems in Python, TypeScript, or C and optimize for minimal tokens, performance, security, less boilerplate, and clear observability.

**Quick Links:**
- 📖 [Complete Documentation Index](DOCUMENTATION_INDEX.md) — Guided reading paths
- 📋 [Project Summary](PROJECT_SUMMARY.md) — Executive overview for stakeholders
- 🎯 [First Program](spec/04-hello-turn.md) — Hello Turn example
- 🔬 [Why Turn?](research/00-problems-we-solve.md) — Deep science (10 problems, 50+ citations)

## Status

**Spec locked for v1 minimal core.** Design mandate and primitives are fixed. **Implementation:** Rust bytecode VM from day one (see [spec/07-implementation-strategy.md](spec/07-implementation-strategy.md)). Not Python/TypeScript—those languages' overhead contradicts Turn's goals (fast, cost-efficient).

## Design mandate

Mission, design goals, and first-principles justification: [spec/00-design-mandate.md](spec/00-design-mandate.md).

High-level design and rationale: [Internal Design Document](spec/00-design-mandate.md)

Empirical validation and industry pains analysis: [research/07-empirical-analysis.md](research/07-empirical-analysis.md)

## Project layout

```
Turn/
├── README.md                    # This file
├── DOCUMENTATION_INDEX.md       # Complete navigation guide
├── research/                    # Deep science and first-principles
│   ├── 00-problems-we-solve.md  # 10 pains in Python/TS/C agents
│   ├── 07-empirical-analysis.md # Production codebase validation
│   └── ...                      # Syntax, structure, algorithms, foundations
├── spec/                        # Language specification (locked for v1)
│   ├── 00-design-mandate.md     # Mission and design goals
│   ├── 01-minimal-core.md       # Turn, context, memory, tool primitives
│   ├── 02-grammar.md            # BNF, lexer, operators
│   ├── 03-runtime-model.md      # Configuration, transitions, semantics
│   ├── 04-hello-turn.md         # First program example
│   └── 07-implementation-strategy.md  # Rust VM architecture
└── impl/                        # Rust bytecode VM implementation
    ├── src/                     # Lexer, parser, compiler, VM, runtime
    └── tests/                   # Integration and suspension tests
```

**New to Turn?** Start with [DOCUMENTATION_INDEX.md](DOCUMENTATION_INDEX.md) for guided reading paths.

## Running

```bash
cd impl
./run.sh test      # run tests
./run.sh hello     # run hello_turn (prints "Hello")
./run.sh examples  # run all examples
./run.sh build     # build release binary
```

Or directly:
```bash
cd impl && cargo run -- run ../examples/hello_turn.turn
```

## Deployment (Server Mode)

Turn includes a built-in HTTP server for deploying agents as microservices.

```bash
cd impl && cargo run -- serve --port 3000
```

Then run an agent via API:

```bash
curl -X POST http://localhost:3000/run \
  -H "Content-Type: application/json" \
  -d '{
    "id": "my-agent",
    "source": "turn { return \"Hello from API!\"; }"
  }'
```

**Features (v1):**
- **Structured Data:** Lists `[1, 2]` and Maps `{"a": 1}`.
- **Persistence:** Automatic state saving on tool calls (`.turn_store`).
- **Standard Library:** `http_get`, `http_post`, `json_parse`, `llm_generate` (requires `OPENAI_API_KEY`).
- **Server Mode:** Built-in `turn serve` command.

**First time?** Install Rust if needed: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

## Research first

Before writing grammar or code, we are:

0. **Problems we solve** — Deep pain when using Python, TypeScript, C, etc. for agentic software: context, memory, turn/tool as hacks, state smeared, no shared semantics, wrong mental model. Science and references in [research/00-problems-we-solve.md](research/00-problems-we-solve.md).
1. **Syntax** — What great designers did (BNF, lexing, parsing, AST). Lessons from Wirth, McCarthy, Iverson.
2. **Structure** — Modules, scoping, types. Lessons from Scheme, Smalltalk, modern languages.
3. **Algorithms** — Parsing, evaluation, compilation. Front-end and runtime pipeline.
4. **Foundations** — Principles from the best creators; formal semantics (operational, denotational, axiomatic).
5. **Turn-specific** — How agentic primitives (turn, memory, context, goal, tool) map onto these foundations.

See [research/README.md](research/README.md) for the full research plan and index.
