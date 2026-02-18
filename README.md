# Turn

**A systems language for agentic computation.** Turn is a compiled language whose primitives encode the physical constraints of agentic software: finite context capacity, stochastic inference, durable state, and suspension/resumption across effects.

**Quick Links:**
- [Vision](VISION.md) — Single source of truth for language vision and roadmap
- [Whitepaper](WHITEPAPER.md) — Publishable model and semantics overview
- [Spec](spec/) — Formal grammar and runtime model

## Status

**Spec locked for v1 minimal core.** Design mandate and primitives are fixed. **Implementation:** Rust bytecode VM from day one (see [spec/07-implementation-strategy.md](spec/07-implementation-strategy.md)). Not Python/TypeScript—those languages' overhead contradicts Turn's goals (fast, cost-efficient).

## Design mandate

Mission, design goals, and first-principles justification: [spec/00-design-mandate.md](spec/00-design-mandate.md).

High-level design and rationale: [spec/00-design-mandate.md](spec/00-design-mandate.md)

## Project layout

```
Turn/
├── README.md                    # This file
├── VISION.md                    # Vision and roadmap (public)
├── WHITEPAPER.md                # Publishable model + semantics overview
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

**New to Turn?** Start with [VISION.md](VISION.md) then read the [spec/](spec/) for formal semantics.

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
cd impl && cargo run -- run ../examples/hello_turn.tn
```

**Features (v0.4.0):**
- **Standard Library**: Built-in modules `std/fs`, `std/http`, `std/math`, `std/json`, `std/time`, `std/regex`.
- **Native Intelligence**: `infer Num { "Prompt" }` connected to real LLMs (OpenAI, Anthropic, Gemini, etc.).
- **Probabilistic Logic**: `confidence` operator and uncertainty propagation.
- **Concurrency**: Actor model with `spawn`, `send`, `receive`.
- **Vector Embeddings**: `vec[1,2,3]` and `~>` similarity operator.
- **Language Core**: Multi-arg methods (`math.max(10, 20)`), object-shaped syntax, comparison operators.
- **Structured Data**: Typed Generics `List<T>`, `Map<T>`.
- **Language Server**: `turn lsp` included.
- **Orthogonal Persistence**: `suspend;` primitive for durable checkpoint boundaries.

**Legacy Features (v0.2.0):**
- **Persistence**: Automatic state saving (`.turn_store`).
- **Server Mode**: Built-in `turn serve` command.
- **Error Handling**: `try/catch/throw`.

**First time?** Install Rust if needed: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

## Reading order

1. `VISION.md`
2. `WHITEPAPER.md`
3. `spec/00-design-mandate.md` → `spec/03-runtime-model.md`
4. `impl/` (reference implementation)
