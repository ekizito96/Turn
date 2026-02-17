# Research: Foundations

What we’re unpacking: **design principles** from the best language creators and **formal semantics** (operational, denotational, axiomatic) so Turn has a clear, precise foundation and we can reason about correctness and behavior.

---

## Open questions

- [ ] Which formal semantics style fits Turn best (operational likely)?
- [ ] Do we write a small-step or big-step operational semantics for “one turn”?
- [ ] How do we specify “context update” and “memory read/write” in the semantics?

---

## 1. Design principles from the best creators

### Niklaus Wirth

- **Simplicity** through “transparence and clarity of its features and by a regular structure,” not “utmost conciseness and unwanted generality.”
- **No magic:** “A good designer must rely on experience, on precise, logic thinking, and on pedantic exactness.”
- **Consequences of simplicity:** clarity of concepts, economy of features, efficiency and reliability of implementations.

**For Turn:** Every construct (turn, memory, context, tool) should be transparent and regular. No hidden behavior.

### John McCarthy (Lisp)

- **Minimalism:** A small set of primitive operations; everything else by composition, conditionals, recursion.
- **Program = data:** S-expressions for both; enables macros and meta-programming. Syntax is trivial; semantics carries the weight.
- **Mathematical foundation:** Recursive functions over symbolic expressions; equivalent in power to a universal Turing machine (e.g. via a universal `apply`).

**For Turn:** We could define a minimal core (e.g. “turn” as one primitive, “context” as one, “memory” as a few ops) and derive the rest. If we ever want to reason formally, a small core helps.

### Sussman & Steele (Scheme)

- **“Exceptionally clear and simple semantics.”** Few ways to form expressions.
- **Design by removal:** “Programming languages should be designed not by piling feature on top of feature, but by **removing the weaknesses and restrictions** that make additional features appear necessary.”
- **First-class abstractions:** Procedures (and in Scheme, continuations) as values. Enables powerful composition.

**For Turn:** Add features only when their absence forces ugly workarounds. Prefer a few, well-chosen primitives over many special cases.

### Alan Kay (Smalltalk)

- **Uniform metaphor:** Only objects and messages. No separate “data” and “code” in the sense of non-objects.
- **Biological metaphor:** “Protected universal cells interacting only through messages that could mimic any desired behavior.”
- **Object in control:** The receiver of a message decides how to respond.

**For Turn:** Tools and memory can be “objects” that receive messages; the agent sends “recall(query)” or “run(tool, args).” Keeps the model uniform.

### Ken Iverson (APL)

- **Simplicity and practicality.** Uniformity (few rules), generality (one function, many cases), familiarity (known notation), brevity.
- **Notation as language:** Language grew from mathematical notation. Syntax and semantics aligned with the domain (arrays, functions, operators).

**For Turn:** Our “domain” is turns, context, memory, goals, tools. Notation and syntax should align with these concepts; avoid borrowing unrelated syntax that doesn’t fit.

---

## 2. Formal semantics — why it matters

- **Precision:** No ambiguity about what a program does. Spec and implementation can be checked against the same definition.
- **Reasoning:** We can prove properties (e.g. “context is always bounded,” “memory op is atomic”) and justify optimizations.
- **Design:** Writing down semantics forces us to decide exactly what “one turn” or “context update” means.

---

## 3. Three main approaches

### Operational semantics

- **Meaning = execution.** We define **steps**: one small step (or one “big” step) of the machine. Program meaning is “what happens when we run it.”
- **Small-step:** One reduction at a time; we get a sequence of states. Good for concurrency and interleaving.
- **Big-step:** Whole phrase (e.g. expression, statement) reduces to a value in one shot. Simpler for sequential core.
- **State:** Usually we have a **configuration** (e.g. expression + environment + store). For Turn we need **context**, **memory**, and possibly **turn state** in the configuration.

**For Turn:** Operational semantics fits well. We can define:
- Expression evaluation (big-step or small-step).
- “One turn” as a transition: (current context, memory, goal) + program → (new context, new memory, result or suspension for tool call).

### Denotational semantics

- **Meaning = mathematical object.** Each phrase is mapped to a denotation (e.g. a function from environment to value, or a domain-theoretic value). Compositional: meaning of a compound phrase is a function of the meanings of its parts.
- **Good for:** Equivalence proofs, optimization (e.g. “this refactor preserves meaning”). Can be heavier to set up.

**For Turn:** Possibly later. If we add a type system or want to prove equivalences between “turn A then turn B” and some combined form, denotational semantics could help.

### Axiomatic semantics

- **Meaning = logical rules.** We give **pre- and postconditions** (e.g. Hoare triples {P} C {Q}). Used for program verification.
- **Good for:** Proving “this program satisfies this invariant” (e.g. “context size never exceeds N”).

**For Turn:** Useful for invariants: “after every turn, context is bounded,” “memory write is atomic.” We can adopt this after we have an operational spec.

---

## 4. What to specify formally (priority)

1. **Syntax:** BNF or equivalent (see [01-syntax.md](01-syntax.md)).
2. **Configuration:** What is the “state” of the interpreter? (environment, context, memory, tool registry, turn state.)
3. **Expression evaluation:** Rules for literals, variables, application, conditionals, etc. Standard.
4. **Turn execution:** One rule or a small set: “execute one turn” as a transition that may (a) only evaluate expressions, (b) perform memory/context ops, (c) suspend on a tool call.
5. **Context and memory:** At least informal semantics (e.g. “context.append(x) adds x to the end; context is truncated to max N items”). Later: formal rules.

---

## 5. Lessons to apply

| Lesson | Application to Turn |
|--------|----------------------|
| Simplicity and transparency (Wirth) | No hidden behavior; every construct has a clear rule. |
| Minimal core (McCarthy, Scheme) | Few primitives; build the rest. |
| Design by removal (Scheme) | Add syntax/semantics only when absence hurts. |
| Uniform metaphor (Kay) | Tools and memory as message receivers. |
| Notation aligned with domain (Iverson) | Turn, context, memory, goal, tool in the syntax and semantics. |
| Operational semantics first | Define “one step” and “one turn” precisely. |

---

## 6. Key references

- **Winskel:** *The Formal Semantics of Programming Languages* — operational (small-step, big-step), denotational, axiomatic.
- **Harper:** *Practical Foundations for Programming Languages* — type theory and semantics.
- **Pierce:** *Types and Programming Languages* — formal treatment of types and evaluation.
- **Scheme reports:** Informal but precise English semantics; good model for “clear and simple.”

*(Add exact citations and chapter/section numbers as we go.)*
