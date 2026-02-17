# Research: Algorithms

What we’re unpacking: **parsing algorithms**, **evaluation/interpretation**, **compilation** (if any), and the **runtime pipeline** (front-end → AST → execution). We need to know the standard approaches so Turn’s implementation is sound and maintainable.

---

## Open questions

- [ ] Interpreter first or compiler to bytecode/IR first?
- [ ] How does the runtime represent “current turn,” “context,” and “memory” during execution?
- [ ] Parsing: hand-written recursive descent vs generated (e.g. from BNF)?
- [ ] Do we need a separate “plan” or “turn” execution loop distinct from expression evaluation?

---

## 1. Compiler / interpreter pipeline (front-end)

Standard stages:

1. **Lexical analysis (scanner)**  
   Source text → **token stream**. Handles whitespace, comments, token boundaries. Output: tokens (kind + value).

2. **Syntax analysis (parser)**  
   Token stream → **AST**. Checks grammatical structure. Errors: “unexpected token,” “missing closing brace,” etc.

3. **Semantic analysis (optional but recommended)**  
   AST → **annotated AST or IR**. Resolves names to bindings, checks types (if any), enforces context-sensitive rules (e.g. “tool call target must be a registered tool”). Errors: “undefined name,” “type mismatch.”

**Best practice (from “Implementing Programming Languages”):** Use a **formal grammar** as the single source of truth; generate lexer/parser from it where possible. Reduces bugs and keeps spec and implementation aligned.

**For Turn:** Same pipeline. Semantic pass should understand “turn,” “context,” “memory,” “tool”—e.g. tool names must be defined, context/memory ops have correct arity.

---

## 2. Parsing algorithms

| Approach | Pros | Cons |
|----------|------|------|
| **Recursive descent** | Hand-written; full control; good error messages; easy to extend. | Can be tedious; left recursion and precedence need care. |
| **LR / LALR** | Powerful; handles many grammars; generated from BNF. | Less control over error messages; grammar constraints (e.g. no ambiguity). |
| **Parser combinators** | Composable; embeddable in host language. | Performance and error reporting vary. |
| **PEG / packrat** | Unambiguous; “first match wins”; no separate lexer required (but often used with one). | Not context-free in the classic sense; can be tricky with left recursion. |

**References:** Dragon Book (LR, LALR); Crafting Interpreters (recursive descent, Pratt parsing for expressions).

**For Turn:** Recursive descent is a safe first choice: we can match our grammar exactly and produce clear errors. Expression precedence (if we have infix) can use Pratt or similar. Document the grammar (BNF) first; then implement or generate the parser.

---

## 3. Evaluation (interpreter)

- **Interpretation:** Walk the AST (or a lower-level IR); for each node, perform the corresponding action (look up variable, apply function, etc.). No separate “machine code” step.
- **Environment:** Map from names to values (and possibly types). Updated on bind (let, define, parameter).
- **Evaluation order:** Applicative vs normal order; strict vs lazy. Most languages are strict and applicative. Turn likely the same unless we have a reason for laziness.

**For Turn:** We need an evaluation loop that knows about:
- **Turn boundary:** When we “enter” a turn, we may load context, run a step, then save context and optionally yield.
- **Memory ops:** `remember` / `recall` / `forget` dispatch to a memory backend (in-memory, persistent, or pluggable).
- **Tool calls:** Suspend, call tool, get result, resume. Might be a special form or built-in.
- **Goals:** Possibly “set goal” and “check goal”; evaluation might branch or loop based on goal state.

So the “interpreter” may have **two levels**: (1) ordinary expression evaluation, (2) turn/step execution that uses (1) and orchestrates context, memory, and tools.

---

## 4. Compilation (optional for v1)

- **Bytecode / IR:** Compile AST to a linear or tree-shaped IR; then interpret the IR (or JIT/compile to machine code later). Enables optimization and a smaller, faster interpreter loop.
- **AOT compilation:** Compile to native code or to another language (e.g. C, JS). Better peak performance; more complex toolchain.

**For Turn:** Start with a **direct AST interpreter**. Add a bytecode or IR step only if we need performance or a clearer separation between “semantics” and “execution.” Document the AST and evaluation rules first.

---

## 5. Runtime representation

We need a clear picture of what exists at runtime:

- **Environment:** names → values (and possibly types).
- **Context:** the “current context” value (bounded buffer of messages or state). May be a first-class value or an implicit stack/object.
- **Memory:** backend for long-term/short-term memory (key-value, vector, or hybrid). Could be one object with methods `read`, `write`, `forget`, `summarize`.
- **Tool registry:** name → tool implementation (handler + schema). Handlers can be in-language functions or external (e.g. HTTP).
- **Turn state:** “current turn id,” “pending tool call,” “goal stack?” So the evaluator can implement “run one turn” and “resume after tool result.”

**For Turn:** Write a short “runtime model” doc (could live in `spec/` later) listing these and their lifetimes (e.g. context is per-session, memory may be persistent across sessions).

---

## 6. Key references

- **Crafting Interpreters** (Nystrom): recursive descent, Pratt parsing, tree-walk interpreter, bytecode VM. Very practical.
- **Dragon Book** (*Compilers: Principles, Techniques, and Tools*): lexing, parsing (LL, LR), semantic analysis, code gen.
- **Implementing Programming Languages** (PLT): grammar-driven development, theory and practice.
- **Engineering a Compiler**: full pipeline; optimization if we ever need it.

---

## 7. Lessons to apply

| Lesson | Application to Turn |
|--------|----------------------|
| Grammar-driven front-end | One BNF (or grammar doc); parser and AST derived from it. |
| Recursive descent for clarity | Hand-written parser unless we hit scaling limits. |
| Interpreter first | Direct AST interpretation; add IR/bytecode only if needed. |
| Explicit runtime model | Document environment, context, memory, tools, turn state. |
| Two-level execution | Expression evaluation + turn/step loop that uses it. |

*(Add exact citations and section numbers as we read the books.)*
