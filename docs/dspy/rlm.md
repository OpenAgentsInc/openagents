# DSPy + RLM Integration

## Implementation Status

The conceptual integration described in this document is now implemented in the `rlm` crate.

| Component | Status | Location |
|-----------|--------|----------|
| DSRs integration | **Implemented** | `crates/rlm/Cargo.toml` (feature: `dspy`) |
| DSPy bridge module | **Implemented** | `crates/rlm/src/dspy_bridge.rs` |
| Typed signatures | **Implemented** | `crates/rlm/src/dspy_orchestrator.rs` |
| Router module | **Implemented** | `RouterSignature` |
| Extractor module | **Implemented** | `ExtractorSignature` (with CoT) |
| Reducer module | **Implemented** | `ReducerSignature` |
| Verifier module | **Implemented** | `VerifierSignature` |
| Prompt optimization | Planned | Using COPRO/MIPROv2 |

See [crates/rlm/docs/DSPY.md](../../crates/rlm/docs/DSPY.md) for usage documentation.

---

## Conceptual Background

DSPy and RLMs fit together almost *too* cleanly, because they're both "LLM systems as programs," just attacking different pain points:

* **RLM (paper):** how to *handle arbitrarily long inputs* by moving the prompt into an external environment (REPL) and letting the model slice/peek/compute + recurse.
* **DSPy:** how to *build reliable multi-step LM programs* and then *optimize* (compile/teleprompt) them against metrics instead of hand-tuning prompts.

Here are the main intersections.

## 1) RLM is a runtime; DSPy is the programming model

Think of RLM as the **execution substrate**:

* persistent state (variables, buffers)
* tools (regex, parsers, indices)
* “subcalls” to LMs on chosen snippets

DSPy is the **structured way to define the steps**:

* typed-ish “Signatures” (inputs → outputs)
* Modules (ChainOfThought, ReAct-like, Router, etc.)
* a compiler/optimizer to improve prompts and module wiring based on evals

So: **RLM gives you out-of-core memory + recursion; DSPy gives you composable modules + automatic prompt optimization.**

## 2) DSPy can replace the “hand-wavy policy” inside RLM

In the paper, the root LM decides (via a system prompt) things like:

* how to chunk
* what to search for
* when to subcall
* how to verify
* how to stitch outputs

Those are exactly the parts DSPy is good at turning into explicit modules and then optimizing.

Concrete mapping:

* **RLM “peek/filter” → DSPy Selector module**

  * input: query + cheap observations (lengths, sample lines)
  * output: a plan: which regions/chunks to inspect

* **RLM “subcall per chunk” → DSPy Map/Batch module**

  * signature: (query, chunk) → extracted facts / partial answers
  * optimized prompt to maximize faithfulness on that chunk type

* **RLM “aggregate” → DSPy Aggregator module**

  * signature: (query, partials[]) → final answer
  * optimized to be consistent + cite evidence handles/IDs

* **RLM “verification” → DSPy Critic/Verifier module**

  * signature: (query, answer, evidence summaries) → pass/fail + fixes
  * optimized to reduce the “Qwen keeps re-verifying forever” problem the paper shows

Now instead of “hope the root LM behaves,” you *train/compile* these policies against a benchmark.

## 3) RLM recursion becomes a DSPy “program graph”

RLM’s recursion pattern (root → subcalls) is basically a dynamic call graph.

DSPy already thinks in graphs of modules. So you can represent “depth-1 recursion” as:

* Router (choose chunks)
* Map (run Extract on chunks)
* Reduce (aggregate)
* Optional Verify loop (bounded)

If you later go “deeper recursion,” DSPy can still represent that as nested modules or iterative controllers—just keep it bounded/typed.

## 4) DSPy gives you what the RLM paper says is missing: **training**

The paper explicitly suggests models aren’t trained to be efficient RLM decision-makers and wastes calls / output tokens.

DSPy’s sweet spot is: **optimize prompts (and sometimes few-shot traces) to improve reliability and efficiency under a metric.**

So you can define metrics like:

* exact match / F1 (task score)
* **cost** (tokens, #subcalls)
* latency
* “evidence coverage” (did we actually read the needed chunk IDs?)
  …and compile toward a Pareto frontier (quality vs cost).

This is directly aimed at the paper’s “high variance trajectories” and “redundant verification” observations.

## 5) Practical integration patterns (how you’d actually build it)

### A) DSPy *inside* an RLM REPL

Your REPL environment hosts:

* the giant `context` object (string / list of docs)
* helper functions (search, chunk, sampling)
* a `llm_query` wrapper that DSPy uses

Then DSPy modules run normally, but whenever they need text, they request slices/handles from the REPL rather than stuffing everything into the model context.

### B) RLM as the “tool layer” for DSPy

DSPy orchestrates; RLM provides tools:

* `get_chunk(i)`, `bm25(query)`, `regex(pattern)`, `summarize(chunk)`
* `subcall(signature, chunk)` with batching

This is like giving DSPy a *memory hierarchy* that’s not lossy by default.

### C) Compile a “long-context strategy” per task family

In the paper, task complexity matters (needle vs linear vs quadratic).
With DSPy you can compile different strategies:

* NIAH-like: retrieval-first
* OOLONG-like: map-reduce semantic transforms
* OOLONG-pairs: build structured intermediate representations + programmatic pairing

## 6) The “why it matters” in one line

**RLM solves the context window; DSPy solves the brittleness.**
Together, you get long-context agents that are both *scalable* and *tunable with evals* instead of prompt-vibes.

If you want, I can sketch a minimal architecture (modules + signatures + where the REPL hooks in) specifically for the kinds of tasks you care about (repo understanding, deep research corpora, or your Autopilot trajectories).

---

> We are using Rust not DSPy, what should we implement for it

Here’s a clean, conceptual background on **inference-time compute**, where it comes from, and why it matters now.

---

## What “inference-time compute” means

**Inference-time compute** is all computation you do **after a model is trained**, at the moment you ask it to solve a specific problem.

Traditionally, ML progress focused on:

* **Training-time compute** → bigger models, more data, longer training
* **Inference** → a single forward pass, as cheap and fast as possible

Inference-time compute breaks that assumption. It says:

> *We can spend more computation per query to get better answers from the same trained model.*

This includes:

* multiple forward passes
* intermediate reasoning steps
* tool use
* recursion
* verification
* search
* program execution
* memory access

RLMs, agents, chain-of-thought, tree search, and self-verification are all inference-time compute strategies.

---

## The historical roots (pre-LLMs)

### 1. Classical algorithms

Long before ML:

* **Time–space tradeoffs**: spend more time to use less memory (or vice versa)
* **Search algorithms**: DFS, BFS, A*, branch-and-bound
* **Anytime algorithms**: better answers the longer you run them

Key idea: *computation is a controllable resource*.

---

### 2. Game AI (a huge influence)

Game-playing systems normalized inference-time compute.

* Minimax, alpha-beta pruning
* Monte Carlo Tree Search (MCTS)
* Rollouts, evaluation functions, heuristics

Training produces an **evaluation function**.
Inference does **search + simulation**.

This is a direct ancestor of:

* tree-of-thought
* self-consistency
* recursive LLM calls

---

### 3. Pre-LLM ML examples

* **Ensembles**: run many models, aggregate
* **Test-time augmentation** (vision): multiple crops/views
* **Beam search** (NLP): explore multiple outputs
* **Reranking pipelines**: cheap model → expensive model

All of these trade *latency + cost* for *quality* at inference.

---

## Inference-time compute in LLMs

### Phase 1: Single-pass LMs

Early GPT-style usage:

* One prompt
* One forward pass
* No state, no tools

Performance ceiling is baked into the model weights.

---

### Phase 2: Prompt-level inference compute

People discovered you could “buy intelligence” with more tokens:

* Chain-of-thought
* Scratchpads
* Few-shot examples
* Self-consistency (sample many answers, vote)

Same model, more inference compute → better reasoning.

---

### Phase 3: Agentic inference

Now inference becomes a **process**, not a call:

* Multiple LM calls
* Tool use (search, code, DBs)
* Memory
* Iterative refinement
* Planning + acting loops

The LM becomes a **controller**, not just a predictor.

This is where:

* ReAct
* CodeAct
* tool-using agents
* AutoGPT-style systems
* DSPy programs
  all live.

---

## Why inference-time compute matters economically

Training compute:

* upfront
* fixed
* capital-intensive
* slow iteration

Inference-time compute:

* per-query
* elastic
* tunable
* can be user- or task-specific

This lets you:

* deploy one model
* vary intelligence by budget
* specialize behavior without retraining

It’s **pay-as-you-think**.

---

## The long-context problem reframed

Context limits expose the limits of *pure training-time solutions*.

Two options:

1. Train bigger context windows (expensive, slow, brittle)
2. Use inference-time compute to **manage context externally**

RLMs explicitly choose option 2.

They treat:

* the prompt as data
* the LM as a reasoning engine
* code + recursion as memory management

That is inference-time compute applied to **memory**, not just reasoning.

---

## Key mental model shift

Old view:

> Intelligence = model size × training data

Modern view:

> Intelligence = model × inference-time computation × tools × memory

The model is no longer the whole system — it’s one component in a runtime.

---

## Why this direction is accelerating now

Three reasons:

1. **Models are “good enough”**

   * Marginal gains from training are expensive
   * Inference scaffolding yields large gains cheaply

2. **Cost asymmetry**

   * One model can serve many intelligence levels
   * High-value queries justify high inference spend

3. **Tasks are long-horizon**

   * Research
   * Codebases
   * Planning
   * Agents operating over hours/days

These are fundamentally *inference problems*, not training problems.

---

## How RLMs fit perfectly into this arc

RLMs are a **pure inference-time scaling technique**:

* no retraining
* model-agnostic
* explicit compute/memory tradeoffs
* recursive reasoning
* externalized context

They’re to context windows what MCTS was to game evaluation functions.

---

## One-sentence takeaway

**Inference-time compute is the idea that intelligence is not fixed at training time, but can be scaled, shaped, and controlled by spending computation at runtime — and modern LLM systems are rapidly becoming inference engines rather than static predictors.**

If you want, next we can:

* map inference-time compute to your Autopilot / agent network vision
* compare inference-time compute vs training-time scaling laws
* or formalize it as a systems architecture (scheduler, budgeter, optimizer)
