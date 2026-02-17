# Implementation Strategy: Building Turn

**Status:** Strategy document. Turn solves **real problems**: cost, performance, token efficiency. We cannot build on slow, high-overhead languages and claim to solve these problems.

---

## The Core Problem: Python/TypeScript Contradict Turn's Goals

Turn's mandate: **fast, cost-efficient, minimal tokens, performance**. 

If we build Turn's runtime on Python:
- **10–100× slower** → contradicts "fast"
- **High memory overhead** → contradicts "cost-efficient" 
- **Slow startup** → contradicts "performance"
- **GIL limits parallelism** → contradicts scalability

If we build on TypeScript/Node:
- **V8 JIT overhead** → contradicts "fast"
- **Node.js runtime overhead** → contradicts "cost-efficient"
- **Async/Promise overhead per suspension** → multiplies cost

**This is not acceptable.** We're solving **real cost problems** ($500+/day at scale, token accumulation). Building on slow languages is contradictory.

---

## The Real Problems We're Solving

From [00-problems-we-solve.md](../research/00-problems-we-solve.md):

1. **Cost:** $500+/day at 1K users. Token accumulation (2.1K → 47K tokens). **Every CPU cycle and memory byte matters.**
2. **Performance:** Agent loops need to be fast. Checkpointing, context append, memory read should be **minimal overhead**.
3. **Token efficiency:** Bounded context, minimal runtime overhead so more budget goes to **actual agent work**, not interpreter overhead.
4. **Deterministic semantics:** Turn's core language is deterministic (given config + external inputs, execution is reproducible). Non-determinism is quarantined at effect boundaries (tool calls, LLM calls). This enables debugging, audit, replay: same inputs → same state transitions. Physics: \(S_{t+1} = F(S_t, e_t)\) where \(e_t\) are external events.

**Python/TypeScript add overhead that directly fights these goals.** Rust enables Turn's semantic solutions to run **fast and cheap**.

---

## Two-Layer Solution: Semantic + Runtime

**Semantic reduction (Turn's design):**
- **Bounded context** = prevents unbounded growth
- **Explicit turn** = checkpointable unit
- **Memory discipline** = explicit read/write
- **Tool output control** = structured results
- **Deterministic semantics** = reproducible execution

**Runtime reduction (Rust implementation):**
- **Native speed** = minimal overhead per turn
- **Minimal memory** = no GC pauses, predictable memory
- **Fast serialization** = cheap checkpointing
- **Single binary** = no runtime dependencies, fast startup
- **True concurrency** = no GIL, parallel agent execution

**Key insight:** Turn solves the **semantic problems** (bounded context, explicit turn, memory discipline, deterministic execution). Rust enables those solutions to run **fast and cheap** (native speed, minimal overhead). Building Turn on Python/TypeScript contradicts our goals—we'd solve semantic problems but add runtime friction.

---

## Python Weaknesses (if we build Turn interpreter in Python)

### Performance

- **Interpreted overhead:** Every operation goes through Python's VM. A Turn `let` statement becomes Python function calls, dict lookups, object creation. **10–100× slower** than native code.
- **GIL (Global Interpreter Lock):** Single-threaded execution. Can't parallelize Turn agent execution even if we want to.
- **Dynamic typing overhead:** Every operation checks types at runtime. `context.append(expr)` → Python checks types, resolves methods, allocates memory.
- **Memory overhead:** Python objects have ~24–48 bytes overhead per object. A Turn value (number/string) becomes a Python object.

**Impact on Turn:** If Turn code runs on a Python interpreter, **every turn is slow**. Agent loops that should be fast (checkpointing, context append, memory read) become Python function calls. **Cost multiplies**: more CPU time = more cloud costs.

### Cost

- **Startup time:** Python imports are slow. Starting a Turn program means importing the interpreter, lexer, parser, runtime. **100–500ms** just to start.
- **Memory:** Python runtime uses ~10–50MB baseline. Each Turn agent instance adds Python objects. **High memory = higher cloud costs** (e.g., AWS Lambda charges by memory).
- **Dependency hell:** `pip install` pulls in hundreds of MB. Version conflicts. **Deployment complexity** = slower iteration.

**Impact on Turn:** Agentic systems run **many turns** (thousands per day). If each turn costs more CPU/memory because of Python overhead, **costs scale badly**.

### Ease

- **Dependency management:** `requirements.txt`, virtualenv, conda. Version conflicts. "Works on my machine."
- **Deployment:** Need Python runtime on server. Docker images are large (Python base image ~100MB+).
- **Debugging:** Python stack traces are deep (interpreter → lexer → parser → runtime → your code).

**Impact on Turn:** Harder to **deploy** and **debug** Turn programs if the interpreter is Python.

---

## TypeScript/JavaScript Weaknesses (if we build Turn interpreter in TypeScript)

### Performance

- **V8 JIT is good but not free:** First run is slow (JIT compilation). Hot paths are fast, but Turn's agent code (many small turns) may not "warm up" V8.
- **Async overhead:** Turn's suspension/resumption maps to JavaScript `Promise`/`async/await`. Each `call(...)` becomes a Promise chain. **Overhead per suspension**.
- **Memory:** Node.js baseline ~10–30MB. V8 heap management. Garbage collection pauses can affect Turn's "one turn" latency guarantees.

**Impact on Turn:** Better than Python, but still **overhead per turn**. Suspension/resumption adds Promise overhead.

### Cost

- **Node.js runtime:** Need Node.js on server. Lambda cold starts (V8 initialization) are ~100–300ms.
- **Bundle size:** If we bundle the interpreter, it's large (lexer + parser + runtime). **Slower downloads**, higher storage costs.
- **npm dependencies:** `package.json` with many deps. Version conflicts. Security vulnerabilities.

**Impact on Turn:** Deployment and runtime costs are higher than native code.

### Ease

- **TypeScript compilation:** Need `tsc` or bundler. Type errors at compile time, but runtime can still fail (Turn code errors).
- **npm ecosystem:** Dependency management, `node_modules` bloat.
- **Async complexity:** Turn's suspension maps to Promises. Debugging async Turn code means debugging Promise chains.

**Impact on Turn:** Easier than Python for some things, but **async mapping** adds complexity.

---

## The Solution: Build Fast from Day One

### Option 1: Rust Runtime + Bytecode VM (Recommended)

**Structure:**
- **Lexer/Parser:** Rust (or generate from grammar)
- **Compiler:** Turn → bytecode (Rust)
- **VM:** Execute bytecode (Rust)
- **Runtime:** Agent state, suspension, tool registry (Rust)

**Why Rust:**
- **Fast:** Native speed, zero-cost abstractions
- **Memory-efficient:** No GC pauses, predictable memory
- **No runtime overhead:** Single binary, no interpreter
- **Concurrent:** No GIL, true parallelism
- **Safe:** Memory safety without sacrificing performance

**Performance:** **10–100× faster** than Python, **comparable to native code**.

**Deployment:** Single binary (`turn` command). No runtime dependencies.

### Option 2: Go Runtime + Bytecode VM

**Structure:** Same as Rust, but Go instead.

**Why Go:**
- **Fast:** Native speed, good GC
- **Simple:** Easier to write than Rust
- **Concurrent:** Goroutines for parallelism
- **Single binary:** No runtime dependencies

**Performance:** **5–50× faster** than Python, slightly slower than Rust (GC overhead).

**Trade-off:** Simpler to write, slightly slower than Rust.

### Option 3: Direct Compilation (Long-term)

**Turn → Native Binary** (like Rust, Go compile to native)

- **Maximum performance:** No VM overhead
- **Hardest to build:** Need codegen, optimization passes
- **Future work:** After we have bytecode VM working

---

## Recommendation: Rust from Day One

**v1:** Build **bytecode VM in Rust**.

**Why:**
1. **Solves the problems:** Fast, cost-efficient, minimal overhead
2. **Single binary:** Easy deployment, no dependencies
3. **Future-proof:** Can optimize further (native codegen) without rewriting
4. **Real solution:** Not a "prototype" that contradicts our goals

**Structure:**
```
impl/
├── lexer.rs          # Tokenize Turn source
├── parser.rs         # Parse tokens → AST
├── ast.rs            # AST node definitions
├── compiler.rs       # AST → bytecode
├── bytecode.rs       # Instruction definitions
├── vm.rs             # Execute bytecode
├── runtime.rs        # Agent state, transition rules
└── tools.rs          # Tool registry, handlers
```

**Dependencies:** Minimal. Maybe `serde` for serialization, `clap` for CLI. **No heavy frameworks.**

**Performance target:** Native speed. Profile and optimize from day one.

**Deployment:** `cargo build --release` → single `turn` binary. Can distribute as static binary (no libc dependency if we use musl).

---

## What We Don't Inherit (Turn's Design Protects Us)

### Turn's Primitives Are Ours

- **`turn { }`** is not a Python function or JS async function—it's a Turn primitive. The interpreter implements it, but Turn code doesn't "feel like Python."
- **`context.append(...)`** is not a Python list append—it's bounded, runtime-enforced. The semantics are in the spec, not the host language.
- **`call(...)`** suspension is not Python's `await` or JS's `Promise`—it's Turn's effect system. The interpreter maps it to host language constructs, but Turn code doesn't see that.

### Turn's Runtime Model Is Independent

- **Agent state** (context, memory, turn_state) is defined in spec/03-runtime-model.md. Python/TS are just storage (dicts, objects). Turn's **semantics** (bounded context, checkpointing, suspension) are ours.
- **Cost model** (one config, one transition) is Turn's. Python overhead is **additive**, not **multiplicative**—we can measure and optimize it separately.

### Turn's Syntax Is Ours

- Turn code looks like Turn (see spec/02-grammar.md), not Python or TypeScript. The lexer/parser produce a Turn AST, not Python/TS AST.

---

## Implementation Plan: Rust from Day One

### v1: Rust Bytecode VM

**Structure:**
```
impl/
├── Cargo.toml        # Rust project config
├── src/
│   ├── main.rs      # CLI entry point
│   ├── lexer.rs     # Tokenize Turn source
│   ├── parser.rs    # Parse tokens → AST
│   ├── ast.rs       # AST node definitions
│   ├── compiler.rs  # AST → bytecode
│   ├── bytecode.rs  # Instruction definitions, serialization
│   ├── vm.rs        # Execute bytecode
│   ├── runtime.rs   # Agent state, transition rules (spec/03-runtime-model.md)
│   └── tools.rs     # Tool registry, handlers
└── tests/
    └── hello_turn.turn  # Test with spec/04-hello-turn.md
```

**Dependencies:** Minimal Rust stdlib. Maybe `serde` for bytecode serialization, `clap` for CLI.

**Performance target:** Native speed. Profile from day one. Target: <1ms per turn (excluding tool calls).

**Deployment:** 
- `cargo build --release` → `target/release/turn` binary
- Static binary (musl target): `cargo build --release --target x86_64-unknown-linux-musl`
- Single file, no dependencies, ~2–5MB binary

**Workflow:**
1. `turn compile agent.turn` → `agent.turnc` (bytecode)
2. `turn run agent.turnc` → execute VM
3. Or: `turn run agent.turn` → compile + run in one step

### Why Not Python/TypeScript?

**Python:**
- ❌ 10–100× slower → contradicts "fast"
- ❌ High memory overhead → contradicts "cost-efficient"
- ❌ GIL → contradicts scalability
- ❌ Dependency hell → contradicts "easy"

**TypeScript/Node:**
- ❌ V8 JIT overhead → contradicts "fast"
- ❌ Node.js runtime overhead → contradicts "cost-efficient"
- ❌ Async/Promise overhead → multiplies cost per suspension

**We're solving REAL problems. We need REAL performance.**

---

## Summary: Why Rust from Day One

| Turn Goal | Rust Solution | Python/TS Problem |
|-----------|---------------|-------------------|
| **Fast** | Native speed, zero-cost abstractions | 10–100× slower |
| **Cost-efficient** | Minimal memory, no GC pauses | High overhead, GC pauses |
| **Minimal tokens** | Fast runtime = more budget for tokens | Slow runtime wastes compute |
| **Performance** | Single binary, no interpreter overhead | Interpreter overhead per operation |
| **Easy deployment** | Single static binary | Runtime dependencies, version hell |

**Key insight:** We're solving **real cost and performance problems**. Building on slow languages contradicts our goals. Rust gives us:

1. **Native performance** from day one
2. **Minimal overhead** (more budget for actual agent work)
3. **Single binary** (easy deployment, no dependencies)
4. **Future-proof** (can optimize further without rewriting)

---

## Recommendation: Rust Bytecode VM (v1)

**Build Turn's runtime in Rust from the start.**

- **Lexer/Parser:** Rust (or generate from grammar with `lalrpop`/`pest`)
- **Compiler:** Turn → bytecode (Rust)
- **VM:** Execute bytecode (Rust)
- **Runtime:** Agent state, suspension, tool registry (Rust)

**Performance target:** Native speed. Profile from day one.

**Deployment:** Single `turn` binary. No runtime dependencies.

**This is not a "prototype." This is the real solution.**

Turn's spec (one config, one transition, clear semantics) is designed to be **compilable**. We implement it correctly the first time, in a language that matches our performance goals.

**Long-term:** Consider **self-hosting** (Turn compiler written in Turn) or **native codegen** for maximum performance, but Rust VM is already fast enough for production.
