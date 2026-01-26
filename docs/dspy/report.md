# How DSPy, MIPRO, GEPA, and RLMs fit together

## Executive picture

These four things are best understood as **a stack**:

* **DSPy** is the *system-building layer*: you write an LM application as a modular “program” (multiple LM calls, tool calls, control flow), not a single prompt. DSPy then gives you a way to *compile/optimize* that program. ([DSPy][1])
* **MIPRO (today: MIPROv2 in DSPy)** and **GEPA** are *optimizers/compilers*: algorithms that automatically tune the text parts of your DSPy program (instructions, few-shot demos, sometimes more) to maximize a metric you define. ([DSPy][2])
* **RLMs (Recursive Language Models)** are an *inference-time strategy* for handling **very long context**: instead of stuffing the whole context into the prompt, the model treats the context as an external environment, inspects it programmatically, and can recursively call itself on pieces. ([arXiv][3])
* As of mid‑January 2026, **DSPy added an `dspy.RLM` module**, so RLM-style inference can be used as a first-class DSPy building block—meaning it can sit inside a DSPy program *and be optimized like other modules*. ([GitHub][4])

So: **DSPy is the “programming model.” MIPRO/GEPA are the “compilers/optimizers.” RLMs are a powerful “runtime/inference module/strategy,” now integrated into DSPy.**

---

## 1) What each component does (and what “knob” it turns)

### DSPy: the programming substrate

DSPy is a declarative framework for building **modular AI software**. You define components by their input/output signatures and compose them into pipelines/agents; DSPy can then compile that program into better prompts (and sometimes weights) for your chosen LM(s). ([DSPy][1])

Key implication: in DSPy, you’re not optimizing *one prompt*—you’re optimizing a **graph of LM calls** (plus tools and control flow).

---

### MIPRO (MIPROv2): Bayesian search over instructions + demos

In DSPy, `MIPROv2` is an optimizer designed to **jointly optimize**:

* natural-language **instructions**, and
* **few-shot examples (demos)**

…by (1) bootstrapping candidate demos, (2) proposing instruction candidates grounded in task/program signals, then (3) using **Bayesian Optimization** to search for the best combination across the whole program. ([DSPy][5])

A useful mental model: **MIPROv2 is a structured, sample-efficient “prompt search engine”** for multi-module pipelines.

---

### GEPA: reflective evolutionary optimization (often multi-objective)

GEPA (Genetic‑Pareto) is a reflective optimizer that **evolves** textual components (e.g., prompts/instructions) using:

* **full execution traces**,
* **natural-language reflection** on what went wrong/right, and
* **Pareto-frontier selection** (useful when you care about multiple tradeoffs like quality vs. cost/latency). ([DSPy][6])

DSPy’s GEPA interface also supports returning **text feedback** along with a score, letting you guide optimization with richer signals than a scalar metric alone. ([DSPy][6])

The GEPA paper reports strong empirical results vs RL baselines (e.g., GRPO) and also vs MIPROv2 in their studied settings—worth reading as “reported in-paper,” not a universal guarantee. ([arXiv][7])

---

### RLMs: a runtime strategy for unbounded/huge context

The RLM paper frames Recursive Language Models as a **general inference strategy**: treat the long prompt/context as an **external environment**, and let the LM:

* examine/decompose the context,
* and **recursively call itself** on snippets to solve subproblems. ([arXiv][3])

One concrete instantiation described by the authors uses a **REPL (e.g., Python) environment** where the huge context is stored in variables; the root LM writes code to “peek/grep/chunk,” and can trigger sub-calls over extracted slices. ([Alex L. Zhang][8])

---

## 2) How they “fit”: compile-time optimization vs inference-time scaling

A clean way to connect them is to separate **two kinds of compute**:

### Inference-time compute (runtime behavior)

This is what happens *per user query*.

* **RLMs live here**: they decide how to traverse/partition/inspect giant context during the run, potentially making many controlled sub-calls. ([arXiv][3])
* In DSPy terms, this is implemented as a **module/strategy** you can call inside your program (now explicitly: `dspy.RLM`). ([GitHub][4])

### Pre-inference compute (compile/optimization budget)

This is what you do *before deployment* (or periodically offline) to make the program better.

* **MIPROv2 and GEPA live here**: they run your DSPy program on training inputs, measure performance via your metric, and then tune the program’s textual parameters (instructions/demos/etc.) to improve the metric. ([DSPy][2])

**Key connection:** once RLM is a DSPy module, its behavior is partly governed by *textual instructions/tool schemas/decision prompts*—which means **MIPROv2 and GEPA can optimize the RLM-containing system**, not just a simple Q→A prompt. ([GitHub][4])

---

## 3) The “together” architecture: an end-to-end pattern

Here’s a common way all four come together in a real system (long-context QA over a massive corpus, repo, or log history):

### Step A — Build a DSPy program with an RLM front-end

**Goal:** answer a question over huge context without stuffing it into one prompt.

* **Module 1: `dspy.RLM` (exploration layer)**
  Takes `(context, query)` and returns “relevant snippets / structured notes / extracted facts”.
* **Module 2: Answerer (synthesis layer)**
  A `dspy.Predict` / `dspy.ChainOfThought` module that turns those snippets into the final answer.

Because DSPy programs are just Python control flow over modules, you can also add:

* retrieval tools, validators, citation checkers, etc.,
* and iterate (multi-hop) if needed. ([DSPy][9])

### Step B — Choose a metric that reflects what you actually want

Examples:

* exact match / F1 on a labeled set,
* citation correctness,
* “must include evidence” constraints,
* cost/latency penalties.

DSPy optimizers are explicitly designed to tune a program to **maximize your metric**. ([DSPy][2])

### Step C — Compile with MIPROv2 (good default baseline)

Use MIPROv2 when you want systematic improvements by optimizing:

* instructions,
* and few-shot demos,
  across **multiple predictors** in your pipeline via bootstrapping + Bayesian optimization. ([DSPy][5])

In this combined system, MIPROv2 often ends up optimizing:

* the RLM’s “how to explore the environment” instruction,
* the answerer’s “how to synthesize + format” instruction,
* and any intermediate reasoning/verification steps.

### Step D — Compile with GEPA (when reflection + Pareto tradeoffs matter)

Use GEPA when:

* you have **multiple objectives** (quality *and* cost *and* tool-call count),
* you want to use **trace-level reflection** (“the tool was misused here”, “the decomposition missed the key section”),
* or you can provide **text feedback** beyond a scalar score. ([DSPy][6])

GEPA also explicitly works by capturing traces and proposing instruction updates for the parts of the system that caused failures—this aligns naturally with complex agent/RLM-style pipelines where *how* the model arrived matters. ([DSPy][6])

---

## 4) Practical “division of labor” between MIPRO and GEPA

They overlap (both optimize text components), but they have different strengths:

### When MIPROv2 shines

* You want a strong, fairly general-purpose optimizer for DSPy programs.
* You want **joint instruction + few-shot** optimization with a structured search process.
* You have a clear scalar metric and enough examples to evaluate candidates reliably. ([DSPy][5])

### When GEPA shines

* You care about **tradeoffs** (accuracy vs cost/latency/tool calls): Pareto selection is natural. ([DSPy][6])
* You want **reflection-driven updates** informed by traces (especially for multi-module systems). ([DSPy][6])
* You want to inject **human-style text feedback** into the optimizer loop. ([DSPy][6])
* You’re optimizing “agentic” behavior (including tool descriptions/schemas in some setups). ([GitHub][4])

A pragmatic workflow that many teams use in DSPy-land:

1. start with **MIPROv2** to get a strong baseline quickly,
2. then run **GEPA** to push further—especially if multi-objective or trace-level issues remain. ([DSPy][2])

---

## 5) The unifying theme: “systems that learn better behavior without retraining the base LM”

All four sit in a coherent philosophy:

* **DSPy**: treat an LM app as a *program* with learnable parameters. ([DSPy][1])
* **MIPROv2 / GEPA**: improve that program by *compiling* it—searching/evolving prompt components to maximize metrics. ([DSPy][5])
* **RLMs**: expand what the program can do at inference-time by giving it a *structured environment* for long context and enabling recursive self-calls. ([arXiv][3])
* **DSPy + RLM** (recent): bring RLM into the same modular/optimizable ecosystem, so you can treat “long-context handling strategy” as just another programmable/optimizable component. ([GitHub][4])

---

## 6) If you want a crisp one-liner

**DSPy is the framework that turns LM applications into optimizable programs; MIPRO and GEPA are two different compilers/optimizers for those programs; RLMs are a powerful inference-time module/strategy for long context that DSPy now exposes as `dspy.RLM`, making it something you can compose and then optimize with MIPRO/GEPA.** ([DSPy][1])

---

## 7) OpenAgents: why we care and how we use this stack

OpenAgents implements DSPy as the **`dsrs`** crate (Rust DSPy). We care because it
turns agent behavior into **typed, auditable, optimizable programs** that can be
compiled into **policy bundles** (target) and verified with real-world outcomes.

### Why it matters here

- **Policy vs execution split:** DSPy (`crates/dsrs/`) decides *what to do*; the
  runtime enforces *how* to do it (schema validation, retries, receipts).
- **Outcome-coupled learning:** decision labels target `step_utility` and
  `verification_delta`, so optimizers improve behavior without retraining models.
- **Portability:** signatures + adapters let us swap providers and lanes without
  rewriting decision logic.
- **Auditability:** compiled module manifests and replay/receipt artifacts are
  the traceability target for decisions and outcomes.

### Where it shows up today

- **Adjutant decision pipelines** (complexity, delegation, RLM trigger) and
  training data collection (`crates/adjutant/docs/DSPY-INTEGRATION.md`).
- **Autopilot planning/execution/verification flow** (DSPy signatures drive the
  loop; see `crates/autopilot-core/docs/EXECUTION_FLOW.md`).
- **Effuse UI plan** for signature-driven rendering (`docs/dsrs-effuse-ui-plan.md`).
- **Artifacts and replay targets**: Verified Patch Bundle specs in
  `crates/dsrs/docs/ARTIFACTS.md` and `crates/dsrs/docs/REPLAY.md`.

### Near-term integration targets

- Merge per-step decisions into **ToolCallSignature** and add
  **ToolResultSignature** for step-level learning signals (spec-only today; see
  `crates/dsrs/docs/SIGNATURES.md` and `ROADMAP.md`).
- Unify PlanIR across Adjutant and Autopilot to avoid split training data.
- Finish REPLAY.jsonl emission and policy bundle pin/rollback paths.

For a deeper OpenAgents-specific walkthrough, see
`docs/dspy/openagents-usage.md` and the root docs:
`GLOSSARY.md`, `SYNTHESIS_EXECUTION.md`, and `ROADMAP.md`.

[1]: https://dspy.ai/ "DSPy"
[2]: https://dspy.ai/learn/optimization/optimizers/ "Optimizers - DSPy"
[3]: https://arxiv.org/abs/2512.24601 "[2512.24601] Recursive Language Models"
[4]: https://github.com/stanfordnlp/dspy/releases "Releases · stanfordnlp/dspy · GitHub"
[5]: https://dspy.ai/api/optimizers/MIPROv2/ "MIPROv2 - DSPy"
[6]: https://dspy.ai/api/optimizers/GEPA/overview/ "1. GEPA Overview - DSPy"
[7]: https://arxiv.org/abs/2507.19457 "[2507.19457] GEPA: Reflective Prompt Evolution Can Outperform Reinforcement Learning"
[8]: https://alexzhang13.github.io/blog/2025/rlm/ "



      Recursive Language Models | Alex L. Zhang


"
[9]: https://dspy.ai/learn/programming/modules/?utm_source=chatgpt.com "Modules"
