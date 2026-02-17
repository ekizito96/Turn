# Research: Syntax

What we’re unpacking: **lexing**, **parsing**, **grammar** (BNF and variants), **AST design**, and what the best language creators did so we don’t repeat their mistakes and we steal their good ideas.

---

## Open questions (resolved in spec)

- [x] **Syntax style:** Conventional keyword/block, not S-expr. See [spec/02-grammar.md](../spec/02-grammar.md) §5 Rationale.
- [x] **Grammar for turn and context:** Locked in spec §1–2 (turn Block, context.append, remember, recall, call).
- [x] **Layers:** Program = Stmt*; Expr vs Stmt. One grammar. See spec/02-grammar.md.

---

## 1. Grammar and specification

### BNF and context-free grammars

- **Context-free grammar (CFG):** production rules of the form A → α; A is a single nonterminal, α is a sequence of terminals and/or nonterminals. No context dependency.
- **BNF (Backus–Naur form):** standard notation for CFGs. Four components:
  1. **Terminal symbols** — tokens (keywords, literals, operators).
  2. **Nonterminal symbols** — categories (e.g. `Expr`, `Stmt`, `Program`).
  3. **Start symbol** — root nonterminal (e.g. `Program`).
  4. **Rewrite rules** — LHS → RHS; `|` for alternatives.
- **ABNF (RFC 5234):** augmented BNF with repetition, optional sequences, value ranges; used in protocol specs.

**Takeaway for Turn:** We need a clear BNF (or equivalent) for the language. Formal grammar first; parser can be generated or hand-written from it.

### References

- Wikipedia: Context-free grammar, Backus–Naur form.
- RFC 5234: Augmented BNF for Syntax Specifications.

---

## 2. What the best creators did

### Niklaus Wirth (Pascal, Modula, Oberon)

- **Principle:** “Simplicity must be achieved by transparence and clarity of its features and by a **regular structure**, rather than by utmost conciseness and unwanted generality.”
- **Practice:** Small, regular syntax; few constructs; structure mirrors program structure. No magic.
- **For Turn:** Prefer regular, predictable syntax for turns, memory ops, and context. Avoid “clever” shorthand that obscures meaning.

### John McCarthy (Lisp)

- **S-expressions:** Syntax = data. Atoms or `( . )` pairs; lists as nested pairs terminated by NIL. Programs and data share the same representation (homoiconicity).
- **Minimalism:** “Five elementary functions and predicates” plus composition, conditionals, recursion. Syntax is trivial (parentheses and atoms); power is in semantics.
- **For Turn:** If we want “code as data” (e.g. manipulable plans, tools as values), S-expr–style is a proven path. If we want readability for non-Lispers, we need a different surface syntax but might keep a simple core representation.

### Ken Iverson (APL)

- **Notation first:** Language grew from mathematical notation (Iverson notation). **Uniformity** (few, simple rules), **generality** (one function applies to many cases), **familiarity** (known symbols), **brevity**.
- **Arrays + functions + operators:** Small set of concepts; syntax is terse and consistent.
- **For Turn:** We could define a small set of “agentic” constructs (turn, remember, recall, context, goal, tool) and make their syntax uniform and minimal.

### Scheme (Sussman & Steele)

- **“Exceptionally clear and simple semantics and few different ways to form expressions.”**
- **Design by removal:** “Programming languages should be designed not by piling feature on top of feature, but by **removing the weaknesses and restrictions** that make additional features appear necessary.”
- **For Turn:** Start with the smallest set of expression forms that can express turns, context, and memory; add syntax only when the absence creates real pain.

---

## 3. Lexical analysis (lexing)

- **Role:** Source text → stream of **tokens** (identifiers, keywords, literals, operators, punctuation).
- **Separates:** “What are the words?” from “How do they combine?” (parsing).
- **Best practice:** Use a formal or explicit token specification (e.g. regex or simple rules). Avoid ad-hoc character-by-character hacks.

**For Turn:** Decide tokens for: turn boundaries, memory ops (`remember`, `recall`, `forget`), context ops, goal/tool keywords, and standard expression tokens (idents, numbers, strings, parens).

---

## 4. Parsing and AST

- **Role:** Token stream → **Abstract Syntax Tree (AST)**. AST is the first representation that reflects program structure (no source-level trivia).
- **Common approaches:** Recursive descent (hand-written), table-driven (LR, LALR, etc.), parser combinators. Generated parsers often start from a grammar (e.g. BNF).
- **AST design:** Nodes for each grammatical category (expr, stmt, decl, etc.). Clear parent/child relationships. No redundant or ambiguous information.

**For Turn:** AST must have clear nodes for: turn (or “step”), context update, memory op, goal, tool call, and ordinary expressions/statements. Document the AST shape in the spec.

---

## 5. Lessons to apply

| Lesson | Application to Turn |
|--------|----------------------|
| Regular structure (Wirth) | Turn, memory, context should have consistent, predictable syntax. |
| Minimal expression forms (Scheme) | Few ways to form a “turn” and to form memory/context ops. |
| Grammar first (BNF) | Write grammar before coding the parser. |
| Syntax can be data (McCarthy) | Consider whether plans or tool descriptions should be representable in the language as values. |
| Uniformity and brevity (Iverson) | Small set of agentic primitives with uniform syntax. |

---

## 6. Citations and further reading

- Wirth: “Design and implementation of Modula,” Oberon material; “simplicity, clarity, regular structure.”
- McCarthy: “Recursive Functions of Symbolic Expressions and Their Computation by Machine”; S-expressions.
- Scheme: Revised Reports (R3RS, R4RS, R7RS); “clear and simple semantics.”
- Iverson: “A Programming Language”; “The Design of APL” (simplicity, uniformity, generality).
- Dragon Book / Crafting Interpreters: lexing and parsing chapters.

*(Add exact citations and links as we go.)*
