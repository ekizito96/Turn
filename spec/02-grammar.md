# Turn grammar and syntax design (v1)

**Status:** Locked for v1. Turn is object-oriented: the program is the behavior of one agent; `context` and `memory` are that agent's objects. This document gives the BNF, lexer rules, design rationale, and style. See [01-minimal-core.md](01-minimal-core.md) for primitives and [03-runtime-model.md](03-runtime-model.md) for runtime.

---

## 1. Program and statements

```
Program  := Stmt*

Stmt     := Turn | LetStmt | ContextAppend | RememberStmt | CallStmt | ReturnStmt | IfStmt | ExprStmt

Turn     := "turn" Block
LetStmt  := "let" Id "=" Expr ";"
ContextAppend := "context" "." "append" "(" Expr ")" ";"
RememberStmt  := "remember" "(" Expr "," Expr ")" ";"
CallStmt      := "call" "(" Expr "," Expr ")" ";"
ReturnStmt    := "return" Expr ";"
IfStmt   := "if" Expr Block ( "else" Block )?
ExprStmt := Expr ";"

Block    := "{" Stmt* "}"
```

---

## 2. Expressions

```
Expr         := Literal | Id | RecallExpr | CallExpr | CallToolExpr

RecallExpr   := "recall" "(" Expr ")"
CallExpr     := Id "(" Expr* ")"
CallToolExpr := "call" "(" Expr "," Expr ")"   // tool call as expression; evaluates to tool result after resume
Literal      := Num | String
Id           := identifier
```

---

## 3. Terminals

- **Keywords:** `turn`, `let`, `context`, `append`, `remember`, `recall`, `call`, `return`, `if`, `else`.
- **identifier:** non-keyword, letter or `_` then alphanumeric or `_`.
- **Num:** integer or decimal number.
- **String:** `"..."` with escapes as needed (e.g. `\"`, `\n`).
- **Punctuation:** `{`, `}`, `(`, `)`, `,`, `;`, `.`.

No infix operators in v1; no precedence table.

---

## 4. Lexer

The lexer produces a stream of tokens from source text. Rules:

1. **Whitespace:** Spaces, tabs, and newlines separate tokens and are otherwise ignored. No significant newlines (indentation is for style only; the grammar does not use it for parsing).
2. **Keywords:** Reserved words above; matched as whole identifiers. So `turn` is one token, not `t` + `urn`.
3. **identifier:** Longest match: start with letter or `_`, then zero or more letters, digits, or `_`. Must not be a keyword.
4. **Num:** Integer (`0`, `42`, `100`) or decimal (`3.14`). Longest match.
5. **String:** Open with `"`; consume until unescaped `"`. Escapes: `\\`, `\"`, `\n`, `\t` (implementation may support more). No newline in string unless escaped.
6. **Comments (v1):**
   - **Line comment:** `//` to end of line. The rest of the line is ignored.
   - **Block comment:** `/*` to `*/`. No nesting; first `*/` closes.
7. **Punctuation:** Single-character tokens `{`, `}`, `(`, `)`, `,`, `;`, `.`.

Token order: match keywords before identifier (so `turn` is keyword, `turn_id` is identifier). Longest match for numbers and identifiers.

---

## 5. Rationale (why this syntax)

**Conventional blocks, not S-expressions.** We want readability for a broad audience and a single, obvious way to write agent code. S-expr would be minimal and homoiconic but would force `(turn (block ...))` and require a Lisp mindset. Conventional keyword/block syntax (`turn { ... }`) matches the mental model "do one turn: this block." So: Wirth-style regularity over McCarthy-style minimalism for the surface form.

**`turn { body }`.** The keyword names the unit of execution and checkpointing. The block is the body. No extra parentheses; the block delimiter `{ }` is sufficient. Alternatives rejected: `(turn ...)` (S-expr), `do turn ... end` (more verbose), `step { }` (less domain-specific).

**`context.append(expr);`** The agent has a context *object*; we call a method on it. Dot notation makes "this is the agent's context" and "append is the operation" clear. Rejected: `append(context, expr)` (functional; we're OOP), `context += expr` (suggests unbounded; context is bounded).

**`remember(key, value);` and `recall(key)`.** The agent has a memory object; we don't use dot here because remember/recall are the only memory ops in v1 and they read as verbs. So `remember("x", 1);` and `recall("x")` are clear. Rejected: `memory.remember(...)` (redundant with only one object), `store`/`load` (too generic).

**`call(tool_name, arg)`.** Tool call is a distinct primitive (suspend → handler → resume). The word `call` makes the effect obvious. One name, one place; no overloading with normal application. So `call("echo", x)` is always "invoke tool"; `foo(x)` (CallExpr) is reserved for in-language functions later.

**Semicolons.** Statement terminator. Required so the grammar is unambiguous without significant newlines. One statement per line is style; the lexer doesn't rely on newlines.

**No infix operators in v1.** We have no `+`, `==`, etc. yet. That keeps the grammar and lexer minimal. We can add operators and a precedence table in a later version when we need expressions beyond literals, recall, and call.

---

## 6. Style (intended layout)

Not enforced by the grammar; recommended for readability and tooling (formatters, editors):

- **One statement per line** where possible. Semicolon terminates the statement.
- **Block indentation:** Indent the body of `turn { }` and `if ... { }` (e.g. 2 or 4 spaces). Align closing `}` with the keyword that opened the block.
- **Spacing:** One space after keywords (`turn {`, `if x {`); space after `,` in argument lists; no space before `;`.
- **Comments:** Use `//` for short notes, `/* */` for multi-line or section headers.

Example (from [04-hello-turn.md](04-hello-turn.md)):

```turn
turn {
  let name = "Turn";
  remember("user", name);
  context.append("Hello");
  let out = call("echo", "Hello");
  return out;
}
```

---

## 7. Notes

- **CallStmt:** `call(tool_name, arg);` invokes the tool and discards the result.
- **CallToolExpr:** `call(tool_name, arg)` as an expression invokes the tool, suspends until the handler returns, then evaluates to the result. So `let x = call("echo", "hi");` binds the tool result to `x`.
- **CallExpr:** `name(arg1, ...)` is reserved for in-language functions (if added later).
- **Context/memory:** The agent's context object supports `.append(...)`; the agent's memory supports `remember` and `recall`. No other methods in v1.
