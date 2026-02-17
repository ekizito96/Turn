# Turn grammar (v1)

**Status:** Locked for v1. BNF for the minimal core. See [01-minimal-core.md](01-minimal-core.md) for primitives and [03-runtime-model.md](03-runtime-model.md) for runtime.

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
- **String:** `"..."` with escapes as needed.

No infix operators in v1; no precedence table.

---

## 4. Notes

- **CallStmt:** `call(tool_name, arg);` invokes the tool and discards the result.
- **CallToolExpr:** `call(tool_name, arg)` as an expression invokes the tool, suspends until the handler returns, then evaluates to the result. So `let x = call("echo", "hi");` binds the tool result to `x`.
- **CallExpr:** `name(arg1, ...)` is reserved for in-language functions (if added later).
- **Context/memory:** Only the forms above; no other methods in v1.
