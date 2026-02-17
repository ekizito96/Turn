# Research: Problems Turn Solves — Deep Pain in Today’s Agentic Software

**Purpose:** Go deep on the **scientific and technical problems** that arise when people use traditional languages (Python, TypeScript, C, etc.) to build agentic software. Turn is an **object-oriented** language: the agent and its context/memory are the core objects. These problems are what Turn is designed to eliminate by making those abstractions native.

---

## 1. The core thesis: impedance mismatch

**Impedance mismatch** (from databases and systems) means: two models that don’t align. Mapping between them is manual, error-prone, and never quite right. You spend effort fighting the abstraction instead of solving the problem.

- **Traditional languages** are built around: **deterministic execution**, **explicit data structures** (lists, maps, structs), **single-call semantics** (call → return), **state in variables**, **control flow** (sequence, branch, loop).
- **Agentic systems** are built around: **turns** (observe → reason → act → observe), **bounded context** (evolving, often rewritten), **memory** (short-term vs long-term, selective read/write/forget), **goals and plans** (revised from feedback), **tools** (suspend → call → resume), **non-determinism** (model output, tool latency, failures).

When you build agentic software in Python or TypeScript, you are **encoding the second model in the first**. The language has no notion of “turn,” “context,” or “suspend for tool.” You implement them with loops, lists, async, and ad-hoc state. That’s the source of the pain below.

### The Physics of AI Engineering: three laws

A complementary framing from production AI systems is **"The Physics of AI Engineering"** (Kizito, AI Dojo / Prescott Data, 2026): building reliable agents is not about prompt engineering but about respecting the **physical constraints** of LLMs—token limits, attention decay, stochastic drift. Three laws map directly onto our problems:

1. **Law of Finite Attention:** Recall degrades with positional distance; U-shaped curve (primacy ~90%, middle ~50%, recency ~85%). Critical information in the middle is effectively invisible. **Engineering response:** *Priority Stack* — structure context as (top) system/mission, (middle) compressed history, (bottom) current task. In today's code this is manual ordering and eviction; the language has no notion of "context position" or "priority."
2. **Law of Stochastic Accumulation:** In a chain of N steps with per-step error probability p, success probability is (1−p)^N. At 2% per step, 50 steps → ~36% success. "You cannot build reliable multi-step systems on probabilistic chains alone." **Engineering response:** Checkpoint after every step; validation layers; retry (e.g. p′ = p³). So **turn** and **state persistence** are not optimizations—they're requirements. The language should make "one turn" and "checkpoint state" first-class.
3. **Law of Entropic Expansion:** Context grows linearly C(t) = C₀ + k·t while capacity is fixed; without compression, overflow is inevitable. **Silent truncation** (dropping earliest context) is especially dangerous. **Engineering response:** Semantic compression + priority eviction so growth is O(log t). Again: bounded context and compression are architectural, not ad-hoc.

The paper's terminology aligns with Turn: **context** = input to the LLM; **state** = persistent data across turns; **turn** = one cycle of reasoning and action. Its "discipline of AI engineering" (physics first, state is sacred, entropy is the enemy, cognitive offloading, validate everything, design for failure) is exactly what a language with first-class turn, context, memory, and tools would **encode by construction**. See [The Physics of AI Engineering](https://ai-dojo.io/papers/the-physics-of-ai-engineering) for the full masterclass; we cite it where its laws and patterns reinforce the problems below.

---

## 2. Problem 1: Context is not a first-class concept

### The science

- **Context window** is a hard constraint: models have fixed token limits (e.g. 128K). Performance degrades as usage grows (e.g. accuracy drops from ~99% to ~70% in some benchmarks at 32K+ tokens). Beyond the limit, information is dropped or truncated.
- **Context collapse** (or “relevance gradient collapse”): in multi-turn conversations, early retrieval and early turns consume tokens but become less relevant. Newer, more relevant content is squeezed out. By turn 5–6, many systems lose coherence. This isn’t a single bug—it’s gradual degradation; a large share of enterprise multi-turn deployments are affected.
- **Context engineering** is the practice of “filling the context window with the right information at each step.” It requires: **write** (save context elsewhere), **select** (retrieve relevant slice), **compress** (summarize, truncate), **isolate** (separate scopes). These are **strategies**, not primitives.

### Deeper science: empirical evidence and position bias

- **Length alone hurts:** Liu et al. (2025), “Context Length Alone Hurts LLM Performance Despite Perfect Retrieval” (arXiv:2510.05381): performance drops of **13.9%–85%** across math, QA, and coding as input length increases, across five open- and closed-source LLMs. Degradation persists even when irrelevant tokens are masked or replaced with whitespace—so the harm is from **length itself**, not retrieval quality. Mitigation: prompting the model to “recite retrieved evidence before solving” can turn long-context tasks into short-context and yield ~4% gains (e.g. GPT-4o).
- **Lost in the Middle** (Liu et al., 2023/2024, arXiv:2307.03172; TACL 2024): LLMs exhibit **U-shaped performance** over context position. Best when relevant information is at the **beginning** (primacy) or **end** (recency); performance drops when it’s in the **middle**. So “put the important stuff at the start or end” is a strategy—but the language has no notion of “context position” or “context shape,” so the programmer hand-manages ordering and truncation.
- **Positional encoding:** Beyond trained context length, position indices are untrained; attention disperses across many positions and short-context performance degrades. Long-context extension (e.g. LongRoPE to 2M tokens) doesn’t remove the fundamental length–performance tradeoff.
- **Cognitive parallel:** Human **working memory** is bounded (Miller’s “magical number seven plus or minus two”; chunking and recoding extend effective capacity). Bounded, structured context for agents is not a quirk—it’s analogous to a cognitive constraint. The language should make “bounded context” and “chunk/recode” first-class instead of leaving them to ad-hoc lists.
- **Physics of AI Engineering (Law of Finite Attention):** Kizito (2026) gives a quantitative recall model: P_recall ≈ 0.9 at start, 0.5 in middle, 0.85 at end. The **Priority Stack** response—top = system/mission, middle = compressed history, bottom = current task—is the right shape for context, but in Python/TS you implement it with list slicing and comments. Turn could make "context stack with priority slots" a first-class type, so the runtime enforces placement and eviction by priority instead of the programmer hand-managing order.

### How traditional languages handle it (the pain)

- Context is represented as **a list** (e.g. `messages: Message[]`). The programmer manually:
  - Appends after each turn.
  - Trims or summarizes when near the limit.
  - Decides what to keep (last N messages, or by importance, or by summarization).
- There is **no type or abstraction** for “bounded, evolvable context.” No compiler or runtime enforces “context never exceeds N tokens” or “context is always summarized after M turns.” It’s all convention and ad-hoc code.
- **Two contexts** are often conflated or mixed: (1) **model context** — what actually goes to the LLM (system prompt, tools, message history); (2) **local/runtime context** — loggers, API keys, user metadata, not for the model. In Python/TS you pass one “context” object and must remember which fields are for the model and which aren’t. Type errors (e.g. mixing context types across tools and lifecycle hooks) show up at runtime.

### What Turn would do

- **Context as a first-class value or object** with a bounded lifecycle. Operations: `append`, `rewrite` (e.g. summarize), `window(n)`, and maybe `isolate(scope)`.
- **Syntax and semantics** that make “what goes to the model” vs “what stays local” explicit (or separate types). The runtime (or spec) can enforce invariants (e.g. max size, summarization policy).

---

## 3. Problem 2: Memory is hand-implemented infrastructure

### The science

- Agent memory is typically **three layers**: (1) **working/short-term** — session scratchpad, aggressive eviction; (2) **episodic long-term** — past interactions, retrievable by similarity or time; (3) **semantic long-term** — durable facts, preferences, profiles. This mirrors cognitive and systems research: working memory is small and volatile; long-term is selective and curated. “If everything is remembered, retrieval becomes meaningless.”
- **Storage → retrieve → apply**: store interactions (often as embeddings + metadata), retrieve by similarity or key when needed, inject into context. This is a **repeated pattern**, not a one-off.

### Deeper science: memory layers and retrieval

- **Three-layer model** is standard in agent and cognitive architectures: (1) **Working/short-term** — small, volatile, session-scoped; eviction by recency or importance. (2) **Episodic long-term** — past episodes retrievable by similarity (embeddings) or time. (3) **Semantic long-term** — durable facts, preferences, user profile. “If everything is remembered, retrieval becomes meaningless”: selectivity and **forgetting** are part of the design, not an oversight.
- **Vector stores and embeddings:** Store → embed → index; retrieve by k-NN or approximate search; inject top-k into context. The **schema** (what is stored, what metadata, what is queryable) is project-specific. In Python/TS there is no type “memory with schema S”; you pass a client and hope everyone uses it consistently.
- **MEM1, semantic anchoring, ACE:** Research systems treat memory as **tool-based actions** the agent controls (read, write, forget, summarize), with semantic anchoring (linguistic structure, coreference) improving recall. These are **semantic designs** that could be reflected in language primitives (e.g. `recall` with a query type, `forget` with a selector), instead of being reimplemented per project.

### How traditional languages handle it (the pain)

- Memory is **not in the language**. You build it with:
  - In-memory dicts or caches for “short-term.”
  - Vector DBs (Chroma, Pinecone, Redis vector) for “long-term”; you call embed APIs, store, then query and splice results into the message list.
- **No shared abstraction.** Every project reinvents: “when do I write?”, “when do I read?”, “what do I forget?”, “how do I summarize?”. Lifecycle rules (what persists vs what’s ephemeral) are buried in application code.
- **No schema in the language.** You can’t say “this agent’s memory has keys A, B and a vector index C” in the type system or in a single place. Consistency (e.g. “recall always returns this shape”) is by convention.

### What Turn would do

- **Memory as a first-class construct** with operations: `remember(k, v)`, `recall(query)`, `forget(selector)`, `summarize(scope)`. Short-term vs long-term can be two modes or two objects with clear semantics.
- **Runtime** provides a default implementation (in-memory, or pluggable backends); the **language** defines the interface and semantics (ordering, visibility within a turn, persistence boundaries).

---

## 4. Problem 3: The “turn” and tool calls are control-flow hacks

### The science

- **One turn** = perceive (inputs, tool results, context) → reason → act (message, tool call, or memory op). The natural unit of execution in agentic systems is the **turn**, not the **function call**. A function is “call → return”; a turn can **suspend** (e.g. for a tool) and **resume** later with a result.
- **Strict sequential protocol** in current APIs: user message → assistant message → tool call → tool result → assistant response. Synchronous tool calling blocks the whole loop until each tool returns; async tool calling (e.g. AsyncLM) improves latency by allowing multiple calls in flight and notifying when results arrive, but the **programming model** in Python/TS is still “I’ll fake suspension with async/await and a big loop.”

### Deeper science: suspension and resumption in language theory

- **Effect handlers** (Koka, Eff, OCaml 5, Multicore OCaml): when a computation **performs** an effect, control transfers to a handler that receives the effect value and a **delimited continuation** representing the rest of the computation. The handler can **resume** by invoking the continuation with a value—exactly the “suspend for tool, resume with result” pattern. So “tool call” is naturally an **effect**; the runtime is the effect handler. In Python/TS we don’t have first-class continuations or effects; we simulate with async/await and a single global loop.
- **AsyncLM** (arXiv:2412.07017): asynchronous LLM function calling reduces latency 1.6×–5.4× by allowing multiple function calls in flight and notifying the model when each returns. The **protocol** (interrupt/notify) is right; the **language** still doesn’t have “tool call” as a primitive—you wire it in the loop.
- **Implication for Turn:** Turn could treat `call(tool, args)` as an effect: the evaluator performs the effect, the runtime (handler) invokes the tool, gets the result, and resumes the continuation. Then serialization (“saved state”) is the continuation + handler state; debugging (“where did we suspend?”) is “current continuation.” No need to invent a new theory—effect handlers already formalize this.

### How traditional languages handle it (the pain)

- The **canonical pattern** is a **while loop**:  
  `while not done: response = call_llm(messages); messages.append(response); if response.tool_calls: messages.append(execute_tools(response.tool_calls)); else: done = True`
- **Everything agentic is manual around that loop:** context trimming, memory read/write, error handling, retries, human-in-the-loop pauses. The loop is the only “turn” construct; the language doesn’t know that this loop is special.
- **Suspension and resume** are simulated with async/await and state in closures or a state object. There’s no **first-class “suspended for tool”** value. Debugging (“where did we suspend?”) and serialization (“save and restore this run”) are custom every time.
- **Multi-agent or nested workflows** become **state machines** (e.g. LangGraph): nodes for agent steps, edges for transitions. Powerful, but you’re building a state machine **in** a general-purpose language instead of having “turn” and “tool” as primitives. The state machine is the real abstraction; the language doesn’t reflect it.

### What Turn would do

- **Turn as the unit of execution** in the language and runtime. The runtime manages “current turn,” “pending tool call,” “resume with result.” Suspension is a **value** or **state** the runtime understands, so serialization and debugging are defined.
- **Tool call** as a built-in construct: when the program executes `call(tool, args)`, the runtime suspends, invokes the tool, and resumes with the result. No need to hand-roll a while loop that assembles messages and re-calls the LLM.

---

## 5. Problem 4: State is smeared across the program

### The science

- Production agent systems need **three kinds of state** (often cited in agent frameworks): (1) **workflow/process state** — where we are in the graph or plan; (2) **operational state** — retries, idempotency, timeouts; (3) **cognitive state** — what the model actually sees (context, selected memory). When these are **implicit** (smeared across globals, closures, and prompt strings), failures become non-deterministic and hard to reproduce.
- “State as first-class artifact with explicit schemas” is the recommended discipline. In traditional languages, that’s a **design guideline**, not something the type system or runtime enforces.

### Deeper science: long-horizon failure modes (state drift and goal drift)

- **State drift** (e.g. “State Drift in Language-Conditioned Autonomous Agents,” Preprints 2026): the agent’s **internal textual representation of state** drifts from the true environment state over time. It can persist even when individual reasoning steps are locally coherent—so long-horizon failure isn’t just “one wrong step.” Increasing context capacity doesn’t fix it in deterministic environments; the issue is **using natural language as internal state** without a disciplined update and sync mechanism. Agents often don’t detect their own drift.
- **Goal drift** (“Evaluating Goal Drift in Language Model Agents,” arXiv:2505.02709): agents deviate from the original instruction-specified objective over time. All evaluated LMs show some goal drift over extended operation; it correlates with pattern-matching behavior as context grows. Best models (e.g. Claude 3.5 Sonnet with scaffolding) can maintain adherence for 100K+ tokens but that’s the exception. **Intrinsification** (instrumental goals becoming permanent) raises safety concerns.
- **Performance collapse:** On long-horizon tasks, accuracy can approach zero beyond ~120 steps and collapse in &lt;15 steps on harder variants. So “many turns” is not just an engineering challenge—it’s a **semantic** one: the system needs explicit representations of “current goal,” “current plan,” and “state sync” that the language and runtime can enforce, not just a longer message list.
- **Physics of AI Engineering (Laws 2 and 3):** **Stochastic accumulation** (Law 2) implies checkpointing after every turn and validation at boundaries—so "turn" and "state" must be explicit and serializable. **Entropic expansion** (Law 3) implies context growth must be bounded and compressed; the paper's "state entropy" (dS/dt = k_accumulation − k_compression; goal dS/dt ≤ 0) and "poisoned well" (error traces biasing the model) argue for **context sanitization** and **priority eviction** as part of the runtime, not ad-hoc middleware. "State is sacred" and "entropy is the enemy" are design principles that Turn can bake into the configuration and context model.

### How traditional languages handle it (the pain)

- **No single “agent state” type.** You have: a list of messages, a dict for short-term memory, a connection to a vector DB, a variable for “current step,” maybe a workflow engine’s state. They’re separate variables; consistency (e.g. “context and memory never diverge”) is by discipline.
- **Context pollution:** without curation, prompts accumulate tool outputs, stack traces, retries. Signal degrades; later turns make worse decisions. Fixing this is more middleware and ad-hoc logic.
- **Persistence and recovery:** “save and resume this agent” means serializing all of the above. Because the language doesn’t have one conceptual “agent state,” you define your own schema and serialization. Every framework does it differently.

### What Turn would do

- **Explicit runtime state** in the spec: environment, context, memory, tool registry, turn state. The **configuration** of the interpreter is a single, well-defined object. Serialization and recovery can be defined once for that configuration.
- **Scoping rules** in the language (e.g. what is lexical vs dynamic) make it clear what “current context” and “current memory” refer to, so state isn’t hidden in closures and globals.

---

## 6. Problem 5: No shared semantics for “one step” or “one turn”

### The science

- **Formal semantics** (operational, denotational, axiomatic) give a precise meaning to programs. Without them, “what does this agent do?” is defined only by the implementation. Different frameworks implement “one turn” differently (what counts as input, when context is updated, when memory is flushed). You can’t compare or prove properties across implementations.
- **Operational semantics** in particular: “one step” or “one turn” as a transition rule. That’s exactly what’s missing in the ecosystem. We have patterns (the while loop), not a **spec**.

### Deeper science: why a spec matters

- **Equivalence and optimization:** Without a formal “one turn” transition, you can’t say “implementation A and B are equivalent” or “this refactor preserves behavior.” Compiler/runtime optimizations (e.g. context compaction, batching tool calls) have no specification to preserve.
- **Testing and verification:** Property-based or model-based testing (e.g. “every reachable state has context size ≤ N”) requires a state model. That model is the operational semantics configuration (context, memory, turn state).
- **Interoperability:** If two systems both “implement Turn,” they should agree on what a turn does. Today, “implement the agent loop” means “write a while loop”; no two codebases agree on the exact transition (when context is updated, when memory is flushed, what counts as one step).

### How traditional languages handle it (the pain)

- “One turn” is whatever the framework author coded. There’s no standard for “turn” that would let you say: “this Turn program and this Python script are equivalent.” So we can’t reason about equivalence, optimization, or correctness in a language-independent way.
- **Testing and debugging** are ad-hoc. You test “run the loop N times and see if the output looks right,” not “this transition from state A to state B is correct.”

### What Turn would do

- **Operational semantics** for at least: expression evaluation and “execute one turn.” The spec defines the transition (e.g. from (context, memory, program) to (context′, memory′, result or suspension). Then implementations and tools can be checked against the spec.

---

## 7. Problem 6: Cognitive load and the wrong mental model

### The science

- **Cognitive load** in programming: programmers hold a **mental model** of the program (plans, data flow, control flow). Language design should align the **code** with the **problem domain** so the mental model matches what the code does. When the language’s abstraction (functions, lists, loops) doesn’t match the domain (turns, context, memory, tools), the programmer must constantly **translate** between “what I want (one turn)” and “how I code it (a loop and a list).” That translation is extra cognitive load and a source of bugs.
- Research on “plans” in code: programmers think in goal-oriented chunks; code that mirrors those chunks is easier to understand. Agentic software is **inherently** goal-oriented and turn-based; forcing it into loop-and-list form fights that.

### Deeper science: plans and cognitive fit

- **Plan-based comprehension** (e.g. “Common cognitive representations of program code”): programmers form the same cognitive “plans” across tasks and languages; comprehension is closer to **plan recognition** than to linear text reading. So when the **surface form** of the code (loops, lists) doesn’t match the **plan** (turns, context, tools), the programmer must do extra translation—increasing load and error.
- **Ideal language design** (from cognitive HCI): the language should map onto how designers and programmers naturally think. For agentic systems, the natural unit is the turn and the natural “variables” are context and memory. A language that makes those first-class reduces the gap between plan and code.

### How traditional languages handle it (the pain)

- You think: “do one turn; if there’s a tool call, run it and do another turn.” You write: a while loop, message list mutation, tool dispatch, context trimming, maybe a state machine. The **distance** between the thought and the code is large.
- **Boilerplate** (message list management, tool registration, context budgeting) is repeated in every project. It’s not reusable in a way the language understands, because the language doesn’t have the concepts.

### What Turn would do

- **Notation aligned with the domain:** syntax and semantics for turn, context, memory, goal, tool. The programmer writes “one turn” and “recall from memory” directly; the mental model and the code coincide. Boilerplate becomes runtime or standard library, not repeated application code.

---

## 7a. Problem 7: Observability and debugging are afterthoughts

### The science

- **Non-determinism:** The same input can produce different outputs; reproduction is hard. Failures often come from **prompt issues, tool misuse, or context misunderstanding**, not from a single code bug. So the debugging unit isn’t “this line crashed”—it’s “at this turn, with this context, the agent decided X and that led to failure.”
- **What must be traced:** Every LLM call (prompts, responses, tokens, latency), every tool call (inputs, outputs, timing), RAG/retrieval, and full context (system message, conversation history) at each step. Without a **first-class notion of “turn” and “context,”** traces are ad-hoc: you log what you remember to log, and “current context” is whatever you shoved into the message list.
- **Debugging workflow** (from production postmortems): (1) Locate the conversation (ID, timestamp, user). (2) Read the **timeline** from the start to see what context the agent had at each decision point. (3) Find the **divergence point** (often earlier than the final error). (4) Categorize: misunderstood request, retrieval error, tool misuse, context collapse. Tools (Langfuse, Patronus, etc.) can cut debugging time ~70% when traces are structured—but they’re layering structure on top of unstructured loops and lists.

### How traditional languages handle it (the pain)

- **No standard trace shape.** Each framework logs differently. “Turn,” “context at turn start,” “tool call and result,” “context at turn end” are not guaranteed to be first-class events—they’re whatever the loop author logged.
- **Reproducing a failure** means replaying the exact message list and tool results; if the language doesn’t define “agent state,” you don’t have a canonical serialization. So “replay this run” is custom per project.

### What Turn would do

- **Turn and context are in the spec.** So “one turn” is a natural trace boundary; the runtime can emit a standard event (turn start, context snapshot, tool calls, turn end) without the programmer wiring it. **Observability by construction**, not by convention.
- **State serialization** is defined (configuration = context + memory + turn state). Replay and debugging tools can assume a single, well-defined shape.

---

## 7b. Problem 8: Cost and token budgeting are invisible

### The science

- **Cost structure:** Input and output tokens are metered separately; output is often 3–5× more expensive than input. A single interaction may be ~$0.01, but at 1K users that’s $500+/day. Tool results are fed back as **input** tokens—so verbose tool output multiplies cost.
- **Death by accumulation:** Long conversations grow without natural cleanup. A 15-turn example can go from ~2.1K tokens (turn 1) to ~47K (turn 15) and hit limits. This isn’t “bad prompts”—it’s **architectural**: the language has no concept of “context budget” or “token cost,” so every project invents its own trimming and budgeting.
- **Context window budgeting** (fixed costs: system + tools 7–15K; variable: history + tool results 20–60K; reserve for response 4–8K) is a **design discipline**. In Python/TS it’s comments and ad-hoc checks, not types or runtime guarantees.

### How traditional languages handle it (the pain)

- **No first-class “context budget” or “cost.”** You might track token count in a variable and trim when it exceeds a threshold—but the language doesn’t know that variable is special. Caching, summarization, and model routing are all manual.
- **Optimizations** (prompt caching, batch API, cheaper model for simple turns) are scattered across the codebase. There’s no single place that “owns” the budget policy.

### What Turn would do

- **Bounded context** in the language or runtime implies a **budget** (e.g. max tokens or max messages). The runtime can enforce it (e.g. summarization or rejection when full). Optionally, **cost** or **token count** as a visible quantity so policies (e.g. “summarize when &gt; 80% full”) are expressible in one place.

---

## 7c. Problem 9: Governance and safety are bolted on

### The science

- **AGENTSAFE** (arXiv:2512.03180): end-to-end governance for agentic systems—design, runtime, audit. Profiles the agentic loop (plan → act → observe → reflect), maps risks to taxonomies, introduces safeguards (constrain risky behavior, escalate to human, pre-deploy scenario evaluation). So **governance is about the loop and the actions**, not about a single function.
- **Runtime policy enforcement:** Systems like **Governance-as-a-Service** (arXiv:2508.18765) and **AgentSpec** (arXiv:2503.18666) enforce rules at runtime: triggers, predicates, enforcement (block, escalate, log). AgentSpec shows &gt;90% prevention of unsafe code-agent executions and 100% compliance in autonomous-vehicle scenarios with minimal overhead. So **policies are rules over agent actions and state**.
- **Risks:** Indirect prompt injection, jailbreaks, ambiguous instructions, reward hacking. Mitigations: input/output guardrails, sandboxed execution, pre/post hooks, network controls. These are **cross-cutting concerns** that apply to every tool call and every turn—but in Python/TS they’re middleware and conditionals, not part of the language model.

### How traditional languages handle it (the pain)

- **Policies are code.** You wrap tool calls in “if allowed then run else escalate.” Which tools are allowed, what “allowed” means, and how to escalate are scattered. No standard way to say “this agent may only call these tools” or “this action requires approval.”
- **Audit:** “What did this agent do?” requires reconstructing from logs. If the language doesn’t have first-class “turn” and “action,” audit trails are whatever you logged.

### What Turn would do

- **Policies as part of the runtime or spec:** e.g. tool registry with per-tool policy (allow, require_approval, deny); runtime checks before executing a tool call. **Governance by construction**: the language or runtime knows “this is a tool call” and can enforce a policy layer.
- **Audit:** If every tool call and turn is a defined transition, audit is “replay the transition log” with a single, well-defined format.

---

## 8. Summary: problems in one table

| # | Problem | Science / cause | Pain in Python/TS/C | What Turn provides |
|---|---------|------------------|---------------------|----------------------|
| 1 | Context not first-class | Context window limits; Lost in the Middle; length-alone degradation; need write/select/compress/isolate | Manual list + trim/summarize; two contexts conflated; no enforcement | Context as value/object; bounded; explicit model vs local |
| 2 | Memory as infrastructure | STM/LTM; store→retrieve→apply; selective retention; MEM1/semantic anchoring | Hand-rolled caches + vector DBs; no shared abstraction or schema | Memory primitives; interface + semantics in the language |
| 3 | Turn and tool as hacks | Turn = unit of execution; tool = suspend/resume; effect handlers / continuations | while loop; async to fake suspension; state machines in code | Turn and tool as language/runtime primitives; first-class suspension |
| 4 | State smeared | Workflow, operational, cognitive state; state drift and goal drift in long horizon | Many variables; context pollution; custom serialization | Single runtime configuration; explicit state in spec |
| 5 | No shared semantics | Need precise meaning for “one step”; equivalence and verification | Implementation-defined; no equivalence or proofs | Operational semantics for turn and evaluation |
| 6 | Wrong mental model | Cognitive load when code ≠ domain; plan-based comprehension | Think in turns; code in loops and lists; boilerplate | Notation and primitives for turns, context, memory, tools |
| 7 | Observability ad-hoc | Non-determinism; trace = turn + context + tools; divergence point | No standard trace shape; replay is custom | Turn/context in spec → standard trace; defined state serialization |
| 8 | Cost and budget invisible | Token metering; death by accumulation; context budgeting | No first-class budget or cost; trimming and caching scattered | Bounded context; optional cost/token visibility and policy |
| 9 | Governance bolted on | AGENTSAFE, AgentSpec; runtime policy over actions; audit | Policies as scattered conditionals; audit from ad-hoc logs | Policy in runtime/spec; tool registry + enforcement; defined audit trail |
| 10 | Computation power overhead | Two-layer cost: semantic (tokens, turns) + runtime (interpreter overhead). At scale, every CPU cycle matters. | Python: 10–100× slower, high memory, GIL. TS: V8 overhead, Node runtime. No separation of "agent work" vs "runtime work". | Semantic: bounded context, explicit turn, memory discipline. Runtime: Rust VM = native speed, minimal overhead, single binary. Deterministic semantics enable replay/audit. |

---


## 8a. Completeness: have we identified all deep science issues?

**Checklist:** We have grounded the need for a new language in (1) **context** (attention, position, boundedness, priority), (2) **memory** (STM/LTM, store–retrieve–apply), (3) **turn and tool** (suspend/resume, effect handlers), (4) **state** (workflow, operational, cognitive; drift), (5) **semantics** (no spec for one step), (6) **mental model** (plan vs loop), (7) **observability**, (8) **cost/budget**, (9) **governance**. That covers the main physical, cognitive, and engineering constraints from the literature and production.

## 7d. Problem 10: Computation power and runtime overhead

### The science

**Two-layer cost structure:**
1. **Semantic cost** (big wins): Token accumulation (2.1K → 47K tokens), unbounded context growth, redundant tool calls, inefficient state management. This is **architectural**—the language model doesn't prevent waste.
2. **Runtime cost** (steady wins): Interpreter overhead, memory bloat, GC pauses, serialization cost, policy check overhead. This is **mechanical**—the host language adds friction.

**Physics perspective:** If tokens/LLM calls are the "energy" cost, then:
- **Semantic reduction** = reducing required energy (fewer tokens, fewer turns, bounded context)
- **Runtime reduction** = reducing friction (minimal overhead per unit of work)

**At scale:** $500+/day at 1K users. Every CPU cycle and memory byte matters. If the runtime adds 10–100× overhead (Python/TypeScript), that multiplies cost.

### How traditional languages handle it (the pain)

- **Python:** 10–100× slower than native, high memory overhead (~10–50MB baseline), GIL limits parallelism, slow startup (100–500ms imports). Building agentic systems on Python means **every turn is slow**; checkpointing, context append, memory read become Python function calls. Cost multiplies.
- **TypeScript/Node:** V8 JIT overhead, Node.js runtime overhead (~10–30MB), async/Promise overhead per suspension, npm dependency hell. Better than Python but still **overhead per turn**.
- **No separation:** The language doesn't distinguish "agent work" (tokens, tool calls) from "runtime work" (interpreter overhead). You can't optimize one without the other.

### What Turn does

**Semantic solutions (Turn's design):**
- **Bounded context** = prevents unbounded growth (semantic reduction)
- **Explicit turn** = checkpointable unit (semantic reduction)
- **Memory discipline** = explicit read/write (semantic reduction)
- **Tool output control** = structured results, not verbose strings (semantic reduction)

**Runtime solutions (Rust implementation):**
- **Native speed** = minimal overhead per turn (runtime reduction)
- **Minimal memory** = no GC pauses, predictable memory (runtime reduction)
- **Fast serialization** = cheap checkpointing (runtime reduction)
- **Single binary** = no runtime dependencies, fast startup (runtime reduction)

**Key insight:** Turn solves the **semantic problems** (bounded context, explicit turn, memory discipline). Rust enables those solutions to run **fast and cheap** (native speed, minimal overhead). Building Turn on Python/TypeScript contradicts our goals—we'd solve semantic problems but add runtime friction.

**Deterministic semantics:** Turn's core language is deterministic (given config + external inputs, execution is reproducible). Non-determinism is quarantined at effect boundaries (tool calls, LLM calls). This enables debugging, audit, replay: same inputs → same state transitions. Physics: \(S_{t+1} = F(S_t, e_t)\) where \(e_t\) are external events. Well-defined and reproducible.

---

## 8a. Completeness: have we identified all deep science issues?

**Checklist:** We have grounded the need for a new language in (1) **context** (attention, position, boundedness, priority), (2) **memory** (STM/LTM, store–retrieve–apply), (3) **turn and tool** (suspend/resume, effect handlers), (4) **state** (workflow, operational, cognitive; drift), (5) **semantics** (no spec for one step), (6) **mental model** (plan vs loop), (7) **observability**, (8) **cost/budget**, (9) **governance**, (10) **computation power** (semantic + runtime overhead). That covers the main physical, cognitive, and engineering constraints from the literature and production.

**Possible gaps** (candidates for v2):

- **Reproducibility and testing** — Covered by deterministic semantics (Problem 10) + observability (Problem 7). Turn's deterministic core + effect boundaries enable replay and testing.
- **Composability** — In trad languages you compose functions and modules. In agentic code the unit of reuse (turn, agent, workflow) is not a first-class citizen; you copy-paste loops and registries. **Composition of agents or sub-workflows** (one agent's output as another's context, shared memory) might be a gap. We have "tool" and "context"; we don't yet have "agent as a composable value" or "sub-turn." Worth adding when we design modules/agents.
- **Time and latency** — Turns take seconds; tool calls can take minutes. The model of "call and return" doesn't encode "this may take 30s" or timeouts. We have suspend/resume; we haven't made **timeouts, backpressure, or SLA** first-class. Could stay as library/runtime policy for v1.
- **Multi-agent and distribution** — CAP for agents, eventual consistency, "multiple agents with divergent worldviews." We cited it (Physics doc) but didn't make it a full problem. A language could have "agent" and "message between agents" as primitives; for v1 we can treat multi-agent as composition on top of turn/context/memory.
- **Capabilities and authority** — "What can this agent do?" is often "whatever tools are registered." In trad languages, types and modules bound what code can do. **Expressing "this turn/agent may only call these tools"** in a way the type system or runtime enforces (capability-safe tool calls) could be a gap. Governance (Problem 9) is adjacent; we could extend it to "tool capability" as a type or policy.

**Verdict:** The nine problems plus the formal framing (operational semantics + effect handlers) give a **complete enough** deep-science case for why a new language is needed: the execution model of agentic systems (turns, bounded context, memory, suspend/resume, state, governance) is not aligned with the execution model of existing languages (functions, lists, single-call, variables). The gaps above are either already implied (replay/testing), or are natural extensions (composability, multi-agent, capabilities) to add once the core is stable. We have not missed a major category of "why trad languages hurt."

---

## 8c. Why developers will love Turn

Devs loved **C** for control and transparency ("what I write is what runs"). **Rust** for safety without giving up power ("if it compiles, it's correct"). **Python** for readability and speed to something that works ("batteries included"). **JavaScript/TypeScript** for one language everywhere and a huge ecosystem. Love comes from **fit**: the language matches how they think and what they're building, and it gives them **leverage** (less boilerplate, fewer bugs, clearer reasoning).

**Why will devs love Turn?**

| What they get | Why it matters |
|---------------|-----------------|
| **"It speaks my problem"** | I'm building turns, context, memory, tools. Turn has words for those. I'm not encoding my design in loops and lists; I write it directly. The language matches the domain, so less translation and fewer bugs. |
| **"I can reason about it"** | One turn = one transition; one configuration = one state. I can checkpoint, replay, and debug. There's a spec for "one step," so I know what the runtime does. No more "whatever the framework author coded." |
| **"It doesn't surprise me"** | Context is bounded; state is explicit; tool calls are first-class. No silent truncation, no state amnesia by default, no "where did we suspend?" mystery. The physics (attention, entropy, checkpointing) are in the model, not hidden in the framework. |
| **"I ship production agents, not demos"** | Reliability patterns (checkpoint every turn, validate at boundaries, compress context) are built in. I get "state is sacred" and "entropy is the enemy" by construction. The language is built for production, not for a single run. |
| **"Less boilerplate"** | No more writing the same while loop, message list, and tool dispatch for every project. Turn does that; I write the agent logic. My time goes into *what* the agent does, not *how* the loop is wired. |
| **"We all agree on what we're building"** | Same vocabulary (turn, context, memory, goal, tool) and same semantics. Onboarding and code review are easier. "Run one turn" and "replay this run" mean the same thing for everyone. |

In one line: **Turn gives developers the same feeling C, Rust, Python, and JS gave—a language that fits the problem, reduces accidental complexity, and lets them focus on what they care about.** For agentic software, that's turns, context, memory, and tools; Turn is the language that makes those first-class.

---

## 8b. Formal framing: one place for “turn” and “suspend”

Two ideas from language theory fit Turn directly:

1. **Operational semantics** gives a transition rule for “one step.” For Turn, the step is “one turn”: configuration (context, memory, env, turn state) → transition → new configuration or suspension. The **configuration** is the same object we need for serialization, observability, and state discipline. So: one formal notion of “agent state” and “one turn” subsumes Problems 4 (state), 5 (semantics), and 7 (observability).

2. **Effect handlers** give a formal account of “perform effect → handler runs → resume with value.” Tool call is an effect; the runtime is the handler. So: “tool call” isn’t a special-case hack—it’s a standard effect. The language can expose `call(tool, args)` as the only way to invoke tools, and the runtime (handler) does the real call, policy check, and resume. That addresses Problem 3 (turn/tool) and Problem 9 (governance) in one design.

Taking both together: **Turn’s core could be “operational semantics over a configuration that includes context and memory, with tool call as an effect.”** The rest (syntax, modules, types) builds on that.

---

## 8d. Security and performance (including compute)

### Security: what we touch and what we should make explicit

**Already in scope (Problem 9, governance):** Policy enforcement at tool-call time, tool registry, audit trail, "what can this agent do?" as a runtime concern. AgentSpec, AGENTSAFE, and Google ADK (input/output guardrails, sandboxed execution, pre/post hooks, network controls) are in the references.

**What we should make explicit as language/runtime design:**

- **Prompt injection and context integrity** — Context is what the LLM sees. If untrusted input is concatenated into context without boundaries, the model can be led to ignore instructions or leak data. Turn can help by making **context structure first-class**: e.g. strict separation of "system / mission" (trusted) vs "user / tool output" (untrusted), and a rule that "model instructions are never overwritten by appended content." The language can enforce that context has typed slots (system, history, current_input) so injection surfaces as a violation of the model.
- **Secrets and sensitive data** — API keys, tokens, PII must not live in context that gets sent to the LLM or logged. Today this is convention. Turn can treat **secrets as a separate channel**: e.g. "local/runtime context" (never sent to model, never in checkpoint logs) vs "model context." Parameter injection (Physics doc) — injecting auth from state into tool calls in code, not from context — is the right pattern; the language can make "this value is local-only" explicit so it never bleeds into context.
- **Tool-call capability and sandboxing** — "This turn/agent may only call these tools" (capability-safe tool calls). Governance (Problem 9) covers policy; we can extend to **per-turn or per-agent tool allowlists** enforced by the runtime so that even if the model requests a forbidden tool, the runtime rejects it. Sandboxing (run tool in a restricted env, limit network, limit filesystem) is an implementation concern for tool execution; the language can require that every tool call goes through the runtime so sandboxing is the single place to enforce it.
- **Audit and forensics** — We have "defined audit trail" in the summary. For security this means: every tool call, every context update (or at least every turn boundary) is logged in a form that supports "who did what, when" and "replay for incident review." The single configuration (context, memory, turn state) makes it clear what to log and what to redact (e.g. local-only slots never in audit logs).

So: **we are touching security** through governance, policies, and audit; we should **explicitly** add context integrity (injection), secrets vs context, capability-safe tool calls, and audit/redaction to the design so Turn is security-aware by construction, not by convention.

**Related: Nexus Protocol (agent identity and connection).** Existing auth is built for humans (SSO, 1Password) or static servers (hardcoded secrets), not for dynamic agent fleets. That creates an **N+1 problem** — every new integration needs bespoke auth logic, with security and scaling bottlenecks. The **Nexus Protocol** (Sangalo, Prescott Data, Zenodo 2026) standardizes **Agent Identity and Connection Orchestration**: it decouples **authentication mechanics** (headers, signatures, tokens) from **agent logic**. A central Authority handles identity; agents become **universal adapters** that connect to any service without code changes, driven by server-side policy. For Turn: the *language* does not implement Nexus, but the **runtime** can integrate with it. When the runtime executes a tool call that hits an external API, it can resolve identity and connection via Nexus (or a Nexus-like authority) instead of per-integration secrets. That aligns with our "secrets as a separate channel" and "parameter injection" story: the agent never sees raw tokens; the runtime obtains and injects credentials via Nexus. So Nexus is **runtime/ecosystem** rather than language design—but it makes "Turn agents as universal adapters" feasible without baking auth into every tool. See [The Nexus Protocol (Zenodo)](https://zenodo.org/records/18315572).

### Performance of the language and compute

**Do programming languages affect computational power?** Yes. In general: the language shapes what code does (algorithms, data structures, allocation), and the implementation (interpreter, compiler) determines how much CPU/memory that costs. In **AI systems**, the dominant cost is often **LLM API usage** (tokens in + tokens out). So "computational power" here means both (1) **runtime overhead** of the language (CPU, memory per turn) and (2) **how much the language design drives token/compute spend** (context size, number of LLM calls, redundancy).

**Where the money goes today:** Teams spend heavily on compute for inference (tokens), embedding APIs, tool execution, and infra. The **language and architecture** directly affect token usage: if the default is "append everything to context and send it every turn," cost grows linearly with conversation length. If the language has no notion of bounded context or compression, developers hand-roll it—often too late or inconsistently—so systems over-send and over-call.

**What Turn can do about it:**

| Lever | How Turn helps |
|-------|-----------------|
| **Bounded context by design** | Context is a first-class object with a max size (tokens or messages). The runtime can enforce it (refuse to append, or compress/evict). So **every Turn program has a predictable upper bound on tokens per request**; no "accidentally sent 200K tokens." |
| **Compression and priority as primitives** | Summarization and priority eviction are not afterthoughts—they're operations on the context object (`rewrite`, `window`, priority stack). The language encourages "keep context small and relevant," which directly reduces input tokens per turn. |
| **Fewer LLM calls (cognitive offloading)** | When "what to do" is a small decision and "how to do it" is deterministic, the right design is: LLM decides, code executes. Turn makes **tools** first-class; the natural pattern is "call tool" (code) instead of "ask LLM again." Fewer turns for the same task = fewer API calls = less compute spend. |
| **Cost and token visibility (Problem 8)** | If the runtime or spec exposes **token count** (or cost) for the current context or per turn, developers can optimize and set budgets. Language-level context bounds + visibility = predictable and controllable cost. |
| **Efficient runtime representation** | One configuration (context, memory, turn state) means one serialization format and one place to optimize. Checkpointing can be incremental (e.g. only changed parts); context can be represented in a token-efficient way (e.g. minimal JSON or a compact binary) before sending to the API. The **language design** (single config, explicit turn) allows an implementation to avoid redundant work (no re-building context from scattered lists every turn). |
| **No redundant "agent loop" overhead** | In Python/TS, every project reimplements the loop (message list, tool dispatch, trim). That's CPU and bug-prone. In Turn, the loop is the runtime; the runtime can be implemented once and optimized (e.g. single allocation per turn, reuse buffers). So **language runtime overhead** can be kept small by design. |

**Summary:** Programming languages **do** affect computational power and cost. In AI, the biggest lever is **how many tokens you send and how often you call the model**. Turn addresses this by (1) making context **bounded and compressible** so token usage is predictable and small, (2) making **tools** the default for execution so fewer LLM calls are needed, (3) exposing **cost/token visibility** so engineers can tune, and (4) keeping the **runtime** simple and efficient (one config, one transition) so overhead is minimal. For an AI engineer, Turn is designed so that the language itself helps **reduce compute spend**, not just express agent logic.

---

## 8e. Multi-agent orchestration and agent communication: deep pains and Turn's scope

### Prior art: languages for agent orchestration

**Pel** (Behnam Mohammadi, CMU; SSRN/arXiv): a programming language **for orchestrating AI agents** and LLMs. Inspired by Lisp, Elixir, Gleam, Haskell. Homoiconic; minimal grammar; piping for linear composition; first-class closures; natural language conditions evaluated by LLMs; REPeL with restarts and LLM-powered error correction; **automatic parallelization** of independent operations; inter-agent communication. Targets expressiveness, scalability, cost, security, and fine-grained control that function-calling and raw codegen lack. So there is direct prior art for "language for agent orchestration." **Jason** (BDI/AgentSpeak) and **SARL** (agent-oriented, JVM) are older agent languages; Pel is aimed at LLM-based agents.

### Deep computer-science problems in multi-agent orchestration (with trad languages)

When you build **multi-agent systems** in Python/TS/C, you run into fundamental distributed-systems and coordination problems that the language does not model:

| Problem | What's going on | Why it's "deep" |
|--------|------------------|------------------|
| **Coordination scale** | Flat coordination hits limits around **~100 agents**: communication complexity forces **quadratic message growth**, consensus algorithms become unstable, and **phase transitions** cause unpredictable failures. You need hierarchical designs, weak coupling, and monitoring—but the language has no notion of "agent" or "orchestrator," so you build ad-hoc topologies and message buses. | Classic distributed-systems scaling; the language doesn't give you first-class "agent" or "delegation," so every system reinvents the topology. |
| **Orchestration cost vs benefit** | Orchestration effectiveness depends on performance/cost differentials between agents. Strategies often **overestimate performance gains** and **underestimate orchestration cost** (latency, failure handling, state sync). In trad languages there's no way to express "orchestrate only when benefit > cost" or to reason about orchestration in the language. | Economics of coordination; no language primitive for "delegate" or "orchestrate." |
| **Consensus and agreement** | Getting multiple agents to **agree** on a value or decision (consensus) is a fundamental CS problem. Under **partial synchrony** (bounds exist but may be unknown) and **failures** (fail-stop or Byzantine), you need a minimum number of correct processes (e.g. ⌈(n+1)/2⌉ for fail-stop, ⌈(2n+1)/3⌉ for Byzantine). Trad languages don't encode consensus or failure models; you bolt on a library or protocol. | Theory of distributed consensus; language has no notion of "agree" or "fault tolerance." |
| **No first-class "orchestrator" or "agent"** | Who decides which agent does what? In code you write a "router" or "orchestrator" as yet another service with its own state and loops. There's no **agent as value** or **delegation as primitive**, so composition (agent A delegates to B, B reports back) is hand-wired every time. | Same impedance mismatch as single-agent: the execution model (agents, messages, delegation) is encoded in loops and RPCs. |
| **State and consistency across agents** | CAP for agents: you can't have Consistency, Autonomy, and Partition tolerance all at once. You choose (e.g. AP: eventual consistency, agents tolerate failures). In trad languages, consistency is whatever your DB and message queue do; the **language** doesn't help you reason about agent state and consistency. | Distributed consistency; no language-level model of "agent state" and "eventual consistency." |

So: **yes, there are deep CS problems** in multi-agent orchestration—coordination scale, consensus, fault tolerance, orchestration cost, and the lack of first-class agents and delegation. Building with trad languages means you implement these with libraries and protocols; the language doesn't reflect the model.

### Agent communication and networking: pains

| Pain | What's going on |
|------|------------------|
| **Protocol fragmentation** | No single standard: **MCP** (tool/data access), **ACP** (RESTful, multipart messages), **A2A** (peer-to-peer delegation, Agent Cards), **ANP** (discovery, decentralized IDs). Teams pick one or glue several; interoperability is hard. |
| **Inconsistent message formats** | Without a shared schema, agents pass **inconsistent message formats** → silent failures, brittle integrations. No standard way to express **confidence**, **deadlines**, **token limits**, or **reliability** in the message. |
| **Latency and resilience** | Protocol choice affects **completion time** (e.g. 36.5% variation) and **mean latency** (e.g. 3.48s difference). Resilience under failures differs by protocol. In trad languages you pick a transport (HTTP, gRPC, queue) and hope; there's no language-level notion of "message with deadline" or "retry policy." |
| **Debugging and intent** | **Post-mortem failure analysis** is hard: unstructured messages, **unclear intent semantics**. Debugging a 10-agent workflow can require extensive investigation. The language doesn't give you a single trace format or intent model. |

So: **agent communication and networking** have real pains—protocol proliferation, format inconsistency, latency/resilience trade-offs, and observability. These are partly **protocol/transport** (layer below the language) and partly **language** (what is a "message," what is "intent," how do we type and trace it).

### Are these pains solved by Turn or out of scope?

| Area | Turn v1 (single-agent) | Turn v2 / runtime / scope |
|------|-------------------------|----------------------------|
| **Multi-agent orchestration** | **Out of scope.** Turn v1 is one agent: one configuration (context, memory, turn state), one turn at a time. Orchestrating *multiple* Turn programs (who does what, delegation, consensus) is done **outside** Turn—e.g. in a workflow engine or another language—by running multiple Turn instances and passing data between them. | **In scope for a later version** if we add **agent as value** and **delegation/send** as primitives. Then Turn could express "agent A delegates task to agent B" and the runtime could handle routing. Coordination scale and consensus would still be partly runtime/infra (we don't implement Paxos in the language), but the *model* (agent, message, delegate) could be first-class. |
| **Agent communication (protocol, network)** | **Out of scope for the language.** MCP, A2A, ACP, ANP are **protocols and transports**. Turn doesn't define a wire format or network protocol. A Turn runtime could *implement* one of these (e.g. "Turn agent speaks A2A") so that Turn programs can talk to other agents—but that's runtime/adaptor, not language semantics. | **Partially in scope.** The language can define **message shape and intent** (e.g. "message from A to B with payload and optional deadline") and **send/receive** as primitives; the runtime maps that to a protocol (A2A, etc.). So: **format, intent, and tracing** can be language-level; **wire protocol and network** stay in the runtime. |
| **Communication pains (format, debugging)** | **Indirectly helped.** Single-agent Turn already gives a **defined trace** (turn, context, tool calls) and **one configuration** to log. If we later add multi-agent, the same idea applies: **message as a typed value** and **one trace format** for agent-to-agent messages would reduce "unstructured messages" and "unclear intent." So these pains are **addressable by Turn** when we add agent composition and messages. |

**Summary:** Multi-agent orchestration and agent communication have **deep CS pains** (coordination scale, consensus, protocol fragmentation, format inconsistency, debugging). **Turn v1 does not solve them**—it is single-agent. They are **out of scope for v1** but **in scope for design later**: (1) add **agent** and **message/delegate** as primitives so Turn can express multi-agent workflows; (2) let the **runtime** handle protocol (MCP, A2A, etc.) and network; (3) use **typed messages** and **one trace format** to address format and observability pains. The deep science (consensus, CAP, fault tolerance) remains partly in the runtime and infra; the language can still make **orchestration and communication** first-class so that building multi-agent systems in Turn is not the same as hand-wiring in Python/TS.

---

## 9. References and further reading

### Physics and production AI systems

- Kizito, **"The Physics of AI Engineering: A Deep Science Masterclass,"** AI Dojo (2026), co-authored at Prescott Data. [https://ai-dojo.io/papers/the-physics-of-ai-engineering](https://ai-dojo.io/papers/the-physics-of-ai-engineering). Three laws (Finite Attention, Stochastic Accumulation, Entropic Expansion); priority stack, checkpoint-every-turn, semantic compression, state entropy, poisoned well, cognitive offloading, parameter injection; CAP for agents; production telemetry. Terminology: context, state, turn, token. "Prompt engineering vs AI engineering" table; eight principles (physics first, state is sacred, entropy is the enemy, etc.).

### Context and length

- Liu et al., “Context Length Alone Hurts LLM Performance Despite Perfect Retrieval,” arXiv:2510.05381 (2025); ACL Findings EMNLP 2025. Performance drops 13.9%–85% with length; mitigation by “recite then solve.”
- Liu et al., “Lost in the Middle: How Language Models Use Long Contexts,” arXiv:2307.03172 (2023); TACL 2024. U-shaped performance (primacy/recency); middle of context underused.
- Context budgeting: Gantz (gantz.ai), “Context Window Budgeting for Multi-Turn Agents”; Agents Arcade, “Reducing Token Costs in Long-Running Agent Workflows.”
- “Context equilibria in multi-turn LLM interactions,” arXiv:2510.07777. Drift and restoring forces.

### Memory

- Agents Arcade, “Memory in AI Agents: Short-Term, Long-Term, and Vector Memory”; “How to Implement Memory Systems for AI Agents.”
- MEM1, semantic anchoring (arXiv:2506.15841, 2508.12630); ACE (context engineering, self-improving context).

### Turn, tool, suspension

- “Asynchronous LLM Function Calling,” arXiv:2412.07017. AsyncLM; 1.6×–5.4× latency improvement.
- Effect handlers: Koka, Eff, OCaml 5 (ocaml.org manual “Effects”); “Effect Handlers, Evidently” (scoped resumptions); Effekt language (effekt-lang.org).
- Braintrust, “The canonical agent architecture: A while loop with tools”; Genkit interrupts; OpenAI background mode.

### State, orchestration, long-horizon

- “Why Agent Frameworks Break at Scale” (Alexander Ekdahl, Medium). State as infrastructure; context pollution.
- “State Drift in Language-Conditioned Autonomous Agents,” Preprints 2026 (e.g. 202601.0910). State drift; natural language as state.
- “Evaluating Goal Drift in Language Model Agents,” arXiv:2505.02709. Goal drift; intrinsification.
- LangGraph (state machine); Microsoft Agent Framework; “State Management in Agentic Workflows” (Agents Arcade).

### Observability and cost

- “AI Agent Logging and Observability: Debugging Production Failures” (Athenic); “Debugging AI Agents in Production” (inference.sh); Langfuse, Patronus, Noveum.
- “Managing and Reducing AI Agent Costs” (Brenndoerfer); “Cost Analysis: How Much Does It Cost to Run an Agent?” (Skywork); OpenAI “Managing costs,” prompt caching.

### Multi-agent orchestration and agent communication

- **Pel:** Mohammadi, "Pel, A Programming Language for Orchestrating AI Agents," SSRN/arXiv (e.g. 5202892); CMU. Homoiconic, piping, REPeL, automatic parallelization, inter-agent communication. [haebom.dev/dk58wg2e6j66jmnqevxz](https://haebom.dev/dk58wg2e6j66jmnqevxz).
- "The Orchestration of Multi-Agent Systems: Architectures, Protocols, and Enterprise Adoption," arXiv:2601.13671. MCP, A2A, capability-driven coordination, planning, policy, observability.
- "Scaling Limits in Distributed Multi-Agent Systems: A Practical Survey" (Zenodo). Quadratic message growth, ~100 agent limit, phase transitions, hierarchical designs.
- "When Should We Orchestrate Multiple Agents?" arXiv:2503.13577. Orchestration cost vs benefit, performance differentials.
- "A survey of agent interoperability protocols: MCP, ACP, A2A, ANP," arXiv:2505.02279. Protocol comparison; adoption (MCP → ACP → A2A → ANP).
- "Solving the Multi-Agent Communication Crisis: Introducing the Agent Communication Protocol (ACP)" (Omar Kafeel, Medium). Format, confidence, constraints, reliability.
- "A Scalable Communication Protocol for Networks of Large Language Models," arXiv:2410.11905. Agora meta-protocol; completion time and latency trade-offs.
- Consensus: "Consensus in the Presence of Partial Synchrony" (MIT); Wikipedia "Consensus (computer science)"; fault models (fail-stop, Byzantine), partial synchrony.

### Agent identity and connection (Nexus)

- **Sangalo, M.** "The Nexus Protocol: Standardizing Identity and Connection Orchestration for Autonomous Agents." Prescott Data, Zenodo (2026). [https://zenodo.org/records/18315572](https://zenodo.org/records/18315572). DOI: 10.5281/zenodo.18315572. Decouples auth mechanics from agent logic; central Authority; agents as universal adapters; N+1 auth problem. Refs: Okta/Strata (AI agent identity), Microsoft Entra (agent OAuth), IETF OAuth-for-agents draft, South et al. (identity for agentic AI), HashiCorp (zero trust for agentic systems), SPIFFE/SPIRE.

### Governance and safety

- AGENTSAFE, “A Unified Framework for Ethical Assurance and Governance in Agentic AI,” arXiv:2512.03180.
- “Governance-as-a-Service: A Multi-Agent Framework for AI System Compliance and Policy Enforcement,” arXiv:2508.18765.
- AgentSpec, “Customizable Runtime Enforcement for Safe and Reliable LLM Agents,” arXiv:2503.18666.
- GAF-Guard, arXiv:2507.02986; Google Agent Development Kit, “Safety and Security for AI Agents.”

### Cognitive and language design

- Miller, “The Magical Number Seven, Plus or Minus Two” (1956); chunking, recoding. Wikipedia, Cogprints, NCBI (Magical Mystery Four).
- “Cognitive model for programming” (CISE/ParallelPatterns); “Examining Factors Influencing Cognitive Load of Computer Programmers,” PMC10452396; “Common cognitive representations of program code,” plan-based comprehension.
- Impedance mismatch: object-relational mismatch (Wikipedia, GeeksforGeeks); architectural mismatch (CMU/ICSE, e.g. archmismatch-icse17).

---

## 10. Open questions

- [x] Observability, cost, governance: added as Problems 7, 8, 9. Which of these are v1 language/runtime vs library?
- [ ] How do we quantify “cognitive load” or “impedance” for Turn vs Python (e.g. study or metric)?
- [ ] Which problems must be solved in v1 vs can be library/runtime later? (Proposal: 1–6 are core; 7–9 can start as runtime/convention and become first-class if needed.)
- [ ] Effect handlers: do we adopt an explicit effect system (e.g. tool as effect type) or implement “tool call” as a single built-in effect without general effects?
- [ ] Long-horizon: should “goal” and “plan” be in the core configuration from v1 to address state/goal drift, or added after turn/context/memory are stable?

- [ ] Security (§8d): context slots (system vs user vs tool output), local-only/secrets channel, capability-safe tool allowlist, audit redaction—v1 spec or runtime policy?
- [ ] Performance (§8d): token/cost visibility in the spec; incremental checkpoint format; runtime efficiency targets (e.g. allocation per turn).
- [ ] Multi-agent and communication (§8e): keep v1 strictly single-agent; plan for v2 "agent as value" and "message/delegate" primitives? Runtime adaptors for MCP/A2A?

This doc should drive the design of Turn’s primitives (see [05-turn-primitives.md](05-turn-primitives.md)) and the semantics (see [04-foundations.md](04-foundations.md)).
