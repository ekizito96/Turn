# Turn grammar and syntax design (v1)

**Status:** Locked for v1. Turn is object-oriented: the program is the behavior of one agent; `context` and `memory` are that agent's objects. This document gives the BNF, lexer rules, precedence, design rationale, and style. The syntax is designed for **power and delight**—enough to write real agents without friction, with a clean, consistent surface. See [01-minimal-core.md](01-minimal-core.md) for primitives and [03-runtime-model.md](03-runtime-model.md) for runtime.

---

## 1. Program and statements

```
Program  := Stmt*

Stmt     := Turn | LetStmt | ContextAppend | RememberStmt | CallStmt | ReturnStmt | IfStmt | WhileStmt | ExprStmt

Turn         := "turn" Block
LetStmt      := "let" Id "=" Expr ";"
ContextAppend:= "context" "." "append" "(" Expr ")" ";"
RememberStmt := "remember" "(" Expr "," Expr ")" ";"
CallStmt     := "call" "(" Expr "," Expr ")" ";"
ReturnStmt   := "return" Expr ";"
IfStmt       := "if" Expr Block ( "else" Block )?
WhileStmt    := "while" Expr Block
ExprStmt     := Expr ";"

Block    := "{" Stmt* "}"
```

---

## 2. Expressions (with precedence)

Expressions use a precedence-climbing grammar. Lower in the table = higher precedence (binds tighter).

```
Expr     := OrExpr

OrExpr   := AndExpr ( "or" AndExpr )*
AndExpr  := EqExpr ( "and" EqExpr )*
EqExpr   := AddExpr ( ( "==" | "!=" ) AddExpr )*
AddExpr  := Primary ( "+" Primary )*

Primary  := Literal | Id | RecallExpr | CallExpr | CallToolExpr | "(" Expr ")"

RecallExpr   := "recall" "(" Expr ")"
CallExpr     := Id "(" Expr* ")"
CallToolExpr := "call" "(" Expr "," Expr ")"
Literal      := Num | String | "true" | "false" | "null"
Id           := identifier
```

**Precedence (highest to lowest):** `+` > `==` `!=` > `and` > `or`

---

## 3. Terminals

- **Keywords:** `turn`, `let`, `context`, `append`, `remember`, `recall`, `call`, `return`, `if`, `else`, `while`, `and`, `or`, `true`, `false`, `null`.
- **Operators:** `+`, `==`, `!=`.
- **identifier:** non-keyword, letter or `_` then alphanumeric or `_`.
- **Num:** integer or decimal number.
- **String:** `"..."` with escapes (e.g. `\"`, `\n`, `\t`).
- **Punctuation:** `{`, `}`, `(`, `)`, `,`, `;`, `.`.

---

## 4. Lexer

The lexer produces a stream of tokens from source text. Rules:

1. **Whitespace:** Spaces, tabs, and newlines separate tokens and are otherwise ignored. No significant newlines (indentation is for style only).
2. **Keywords:** Reserved words above; matched as whole identifiers. `turn` is one token, not `t` + `urn`.
3. **Operators:** `+`, `==`, `!=`. For `==` and `!=`, match two characters as one token (so `==` is not `=` + `=`).
4. **identifier:** Longest match: start with letter or `_`, then zero or more letters, digits, or `_`. Must not be a keyword.
5. **Num:** Integer (`0`, `42`, `100`) or decimal (`3.14`). Longest match.
6. **String:** Open with `"`; consume until unescaped `"`. Escapes: `\\`, `\"`, `\n`, `\t`. No newline in string unless escaped.
7. **Comments:**
   - **Line comment:** `//` to end of line.
   - **Block comment:** `/*` to `*/`. No nesting.
8. **Punctuation:** `{`, `}`, `(`, `)`, `,`, `;`, `.`.

Token order: keywords before identifier; operators before punctuation. Longest match for numbers, identifiers, and `==`/`!=`.

---

## 5. Rationale (why this syntax)

**Design principle: power + delight.** The syntax must be **powerful enough** to write real agents without constant workarounds, and **delightful enough** that developers want to use it. Every addition earns its place.

**Conventional blocks, not S-expressions.** Readability for a broad audience. `turn { ... }` matches the mental model "do one turn: this block." Wirth-style regularity.

**`turn { body }`.** Domain-specific keyword; block delimiter `{ }` is sufficient. No extra parentheses.

**`context.append(expr);`** OOP: the agent's context object; dot notation. Rejected `context += expr` (suggests unbounded).

**`remember(key, value);` and `recall(key)`.** Verb-style; clear. `recall` returns `null` when key is missing (see [05-types-and-errors.md](05-types-and-errors.md)).

**`call(tool_name, arg)`.** Distinct primitive for suspend/resume. Explicit; no overloading with normal application.

**Semicolons.** Required. Unambiguous; no ASI footguns. One statement per line is style.

**Operators (properly designed):**
- **`+`** — String concatenation and number addition. `"Hello, " + name`; `1 + 2`. Mixed: string + number coerces number to string (e.g. `"x" + 1` → `"x1"`). Essential for readable agent code.
- **`==`, `!=`** — Equality. For conditions: `if x == "sunny"` instead of tool calls. Proper comparison, not workarounds.
- **`and`, `or`** — Logical, short-circuit. `if x and y`; `if a or b`. Readable conditions.

**Precedence.** Standard: `+` binds tightest (arithmetic/concatenation), then `==`/`!=`, then `and`, then `or`. Matches Python/JS intuition.

**`true`, `false`, `null`.** Boolean literals and null for "missing" (e.g. `recall` when key absent). No magic numbers or strings for booleans.

**`while cond { body }`.** Minimal iteration. Agents need loops (retry, iterate over results). One loop form; sufficient for v1.

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
  context.append("Hello, " + name);
  let out = call("echo", "Hello");
  return out;
}
```

---

## 7. Operator semantics and truthiness

**`+`:** String concatenation or number addition. If either operand is a string, both are coerced to string and concatenated. Otherwise numeric addition.

**`==`, `!=`:** Value equality. Same type and value. `null == null` is true. Type coercion: none (so `1 == "1"` is false).

**`and`, `or`:** Short-circuit. `a and b` → if `a` is falsy, return `a`; else return `b`. `a or b` → if `a` is truthy, return `a`; else return `b`.

**Truthiness (for `if` and `while`):** Falsy: `false`, `null`, `""`, `0`, `0.0`. Everything else is truthy (including non-empty strings, non-zero numbers, `true`).

---

## 8. Notes

- **CallStmt:** `call(tool_name, arg);` invokes the tool and discards the result.
- **CallToolExpr:** `call(tool_name, arg)` as an expression invokes the tool, suspends until the handler returns, then evaluates to the result.
- **CallExpr:** `name(arg1, ...)` is reserved for in-language functions (if added later).
- **Context/memory:** The agent's context object supports `.append(...)`; the agent's memory supports `remember` and `recall`. `recall(key)` returns `null` when key is missing.
