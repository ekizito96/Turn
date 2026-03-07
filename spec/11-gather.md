# 11. `gather` — Mass Concurrency Primitive

**Status:** Implemented in v1.0.0. Part of the core language.

---

## 1. Motivation

`spawn_each` delegates each item in a list to a concurrent actor and returns a `List<Pid>`. Without `gather`, collecting results requires a manual `receive` loop, which is non-deterministic in order, and requires explicit error handling for each actor.

`gather` is the language-native solution: a single keyword that blocks until all PIDs in a list have produced an `ExitSignal`, then returns their results in input order with `allSettled` semantics.

---

## 2. Syntax

```turn
let pids    = spawn_each(list, turn(item: T) { ... });
let results = gather pids;
```

`gather` is a prefix keyword expression. It takes any expression that evaluates to `List<Pid>`.

---

## 3. Semantics

When the VM executes `Instr::Gather`:

1. Pop a `List<Pid>` from the stack.
2. Scan the current process's mailbox for `ExitSignal` messages whose `pid` field matches any PID in the list.
3. Collect matches, leaving unrelated messages in the mailbox.
4. If all PIDs have resolved: assemble results in original PID order, push `List<Any>` to stack, continue.
5. If some PIDs are still running: restore any already-collected signals back to the mailbox, push the PID list back to the stack, decrement the instruction pointer by 1, increment gas by 1, and return `VmResult::Yielded`.

This yield-and-retry loop runs cooperatively on the scheduler. The gathering process yields CPU to all other processes until every child has reported.

---

## 4. Error Isolation (`allSettled`)

If a child actor throws an unhandled exception, the VM catches the error, wraps it as a string value, and delivers it as the `ExitSignal` result for that PID. The gathering process receives the error as a value in its result list rather than as a thrown exception. Every other actor's result is preserved.

```turn
let results = gather workers;
// results[i] is either the actor's return value or its error string
```

A single failed actor never aborts the entire batch.

---

## 5. Type System

| Expression | Inferred type |
|------------|---------------|
| `gather expr` | `List<Any>` |

The static analyzer checks that the operand expression resolves to `List<Pid>`. The result type is `List<Any>` because actor return types are not tracked statically in v1.0.0.

---

## 6. Comparison

| Feature | Manual `receive` loop | `gather` |
|---------|----------------------|---------|
| Code | Loop, PID tracking | One keyword |
| Order | Non-deterministic | Deterministic (matches input order) |
| Error handling | Explicit per message | `allSettled` built-in |
| Use case | Streaming / partial results | Complete batch collection |

---

## 7. Implementation

- **Lexer:** `Token::Gather` in `src/lexer.rs`
- **Parser:** `Expr::Gather { expr, span }` in `src/parser.rs`
- **AST:** `Expr::Gather` variant in `src/ast.rs`
- **Analysis:** `Expr::Gather` expects `List<Pid>` operand, returns `List<Any>` in `src/analysis.rs`
- **Bytecode:** `Instr::Gather` in `src/bytecode.rs`
- **Compiler:** Emits `Instr::Gather` after compiling the operand in `src/compiler.rs`
- **VM:** Full implementation in `src/vm.rs` under `Instr::Gather`
