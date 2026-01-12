Abstract

We present **OpenAgents**, a systems and learning framework for building **self-improving, economically grounded AI agents** that operate over real software artifacts and open compute markets. OpenAgents unifies three previously separate threads: (i) **declarative agent programming and compilation** (DSPy-style signatures, modules, and optimizers), (ii) **open-ended curriculum and task selection** guided by models of interestingness (OMNI/OMNI-EPIC), and (iii) **empirically validated self-modification loops** for agent improvement (Darwin Gödel Machine). Concretely, we replace brittle prompt strings with typed **Signatures** spanning planning, tool execution, synthesis, and routing, and close the loop by logging trajectories, labeling decisions using downstream outcomes (e.g., test deltas, cost, repetition), and triggering automatic recompilation via prompt optimization. To scale beyond context limits and mitigate long-horizon “context rot,” we integrate **Recursive Language Models (RLM/FRLM)** as an execution substrate that externalizes long context into a programmable environment and enables recursive sub-queries across large codebases and document sets. OpenAgents additionally introduces a market-aware runtime in which agents possess cryptographic identity, hold budgets, purchase compute from a decentralized provider network, and emit verifiable receipts for work. We argue that this combination—**compiled behavior + outcome-coupled optimization + scalable recursion + economic constraints**—yields a practical path toward open-ended, continuously improving agents in production domains, and we outline evaluation protocols that measure not only task success but also stability, cost-efficiency, and resistance to Goodhart-style pathologies.

## Table of Contents

1. **Introduction**
   1.1 Motivation: Agents as Real Software Actors
   1.2 From Prompting to Compiled Agent Behavior
   1.3 Contributions

2. **Background and Related Work**
   2.1 Declarative LM Programming and Compilation (DSPy)
   2.2 Auto-Curricula and Interestingness (OMNI, OMNI-EPIC)
   2.3 Self-Improving Agents via Empirical Validation (Darwin Gödel Machine)
   2.4 Long-Horizon Inference and Externalized Context (RLMs)
   2.5 Markets, Identity, and Verification for Agent Economies

3. **System Overview**
   3.1 OpenAgents Stack and Design Principles
   3.2 Threat Model and Assumptions
   3.3 Execution Substrates: Local, Cloud, Swarm

4. **Programming Model**
   4.1 Signatures: Typed Contracts for Agent Cognition
   4.2 Modules: Composable Policies (Plan → Act → Verify → Synthesize)
   4.3 Metrics: From Format Correctness to Outcome-Coupled Utility
   4.4 Optimizers: MIPROv2 and Beyond
   4.5 Policy Bundles, Versioning, and Reproducibility

5. **Agent Runtime**
   5.1 Tick Model and State Surfaces
   5.2 Tooling and Sandboxed Execution
   5.3 Verification Harnesses (Tests, Builds, Deterministic Jobs)
   5.4 Trajectory Logging and Provenance

6. **Decision Pipelines and Routing**
   6.1 Complexity Classification
   6.2 Delegation Decisions (Codex / RLM / Local Tools / Swarm)
   6.3 Provider Lane Selection and Fallback Strategies
   6.4 Counterfactual Recording and Shadow Mode

7. **Recursive Language Models in OpenAgents**
   7.1 RLM as an Execution Substrate
   7.2 Context Externalization and REPL Environments
   7.3 FRLM: Federated Recursion Across Providers
   7.4 Cost/Latency Tradeoffs and Stopping Criteria

8. **Self-Improvement Loop**
   8.1 SessionStore and Outcome Labeling
   8.2 Rolling Accuracy and Drift Detection
   8.3 Auto-Optimization Triggers
   8.4 Avoiding Goodhart Pathologies (Utility, Cost, Repetition, Verification Delta)
   8.5 Archives and Stepping Stones: Toward Open-Ended Improvement

9. **Marketplace and Economic Constraints**
   9.1 Identity and Authentication (Nostr, NIP-42)
   9.2 Job Schemas and Deterministic Hashing
   9.3 Payments, Budgets, and Treasury Policy
   9.4 Incentives, Reputation, and Provider Selection

10. **Evaluation**
    10.1 Benchmarks: SWE-bench, Polyglot, Real Repos, Long-Context Tasks
    10.2 Metrics: Success, Cost, Actions-per-Minute, Stability, Regression Rate
    10.3 Ablations: DSPy vs Rules, With/Without RLM, With/Without Outcome Coupling
    10.4 Market Experiments: Swarm vs Local vs Cloud
    10.5 Failure Modes and Case Studies

11. **Safety and Governance**
    11.1 Sandboxing and Capability Containment
    11.2 Objective Hacking and Robust Evaluation Design
    11.3 Privacy, Redaction, and Data Sharing Policies
    11.4 Human Oversight: Intervention Points and Budgets

12. **Discussion**
    12.1 Why Compiled Agents + Markets Changes the Scaling Story
    12.2 Limits and Open Problems
    12.3 Implications for Self-Improving AI

13. **Conclusion**

**Appendices**
A. Implementation Details (Interfaces, Storage Layouts, Config)
B. Signature Catalog and Prompts
C. Metric Definitions and Scoring Functions
D. Optimization Recipes and Hyperparameters
E. Additional Plots and Tables
F. Reproducibility Checklist

## 1 Introduction

Autonomous agents are starting to write real code, run real commands, and ship real changes—but most of today’s systems are still **prompt-and-scaffold products**: brittle collections of hand-authored prompts and heuristics wrapped around powerful foundation models. They work impressively in demos, yet degrade under the conditions that matter in production: long-running sessions, large codebases, ambiguous requirements, partial observability, and shifting tool availability. When these systems fail, they fail in familiar ways—thrashing between files, repeating actions, hallucinating tool results, overfitting to superficial metrics, or collapsing as context grows. The result is a paradox: LMs are increasingly capable, but agent reliability is capped by **human-designed orchestration** that is hard to maintain, hard to verify, and difficult to improve systematically.

OpenAgents is motivated by a different thesis: the path to dependable autonomy is not another chat wrapper, but a **full stack** where agents behave like real software actors. A real actor has (i) an identity it can use to authenticate and sign work, (ii) a wallet and budget to pay for compute and services, (iii) an execution environment with tools and sandboxes, (iv) objective verification loops (tests, builds, static checks), and (v) an audit trail that makes work inspectable and replayable. OpenAgents provides these primitives and connects them to a learning loop so that behavior can improve over time, not by manual prompt tuning, but by compiling and optimizing structured agent programs against empirical outcomes.

The technical core of OpenAgents is a **declarative programming model** for agent behavior, inspired by DSPy. Instead of embedding behavior in monolithic prompt strings, we express each cognition step as a typed **Signature** with explicit inputs and outputs (e.g., task → subtasks; tool-call request → tool-call result; subtask results → final synthesis). These signatures are composed into **Modules** (plan → act → verify → synthesize) and are optimized using prompt/program optimizers (e.g., MIPROv2) driven by explicit metrics. This compiler layer makes agent cognition portable across model providers and execution lanes—local inference, cloud APIs, or a distributed swarm—while enabling systematic improvement as data accumulates.

However, compilation alone does not solve two fundamental open-endedness problems. First, in large spaces of possible actions and tasks, agents can spend unlimited time on **learnable but low-value work**—a Goodhart-style pathology mirrored in open-ended RL, where learning progress can be hijacked by trivial variations of tasks. Second, autonomy requires reasoning over contexts that are effectively unbounded: large repositories, long verification histories, and multi-hour sessions. Long-context prompts tend to degrade (“context rot”), and simply expanding context windows is costly and often insufficient.

OpenAgents addresses both problems by integrating two complementary ideas. For task selection and routing, we treat “what to do next” as a first-class prediction problem with measurable outcomes, aligning with the OMNI line of work on *interestingness* as a filter for open-ended learning. In OpenAgents, “interestingness” is operationalized as **outcome-coupled utility**: which decisions reduce failing tests, avoid repetition, control cost, and converge to verified success. For long-horizon reasoning, we integrate **Recursive Language Models (RLM/FRLM)** as an execution substrate that externalizes large context into a programmable environment and allows recursive sub-queries over subsets of the state. This keeps the agent’s working context small while still enabling global reasoning, and extends naturally to federation: sub-queries can be executed locally, on specialized cloud models, or via a decentralized compute market.

The full system closes the loop with a self-improvement mechanism inspired by the Darwin Gödel Machine perspective: improvements must be **empirically validated**, and progress must be robust to local optima. OpenAgents logs trajectories, records counterfactual decisions (DSPy vs. legacy heuristics), labels decisions after outcomes, tracks rolling accuracy per signature, and triggers recompilation when performance drops or enough new data is available. While our current implementation optimizes signatures and routing policies rather than rewriting the agent’s entire codebase, the same infrastructure—archives, stepping stones, and outcome-driven selection—supports more open-ended self-modification over time.

In summary, OpenAgents reframes agent autonomy as the composition of four tightly coupled layers: **compiled cognition** (signatures/modules/optimizers), **verifiable execution** (tools, sandboxes, test harnesses), **scalable reasoning** (RLM/FRLM for externalized context), and **economic grounding** (identity, budgets, and paid compute). This framing yields a practical path to agents that do not merely “respond,” but **operate**—selecting tasks, buying resources, producing verifiable work, and improving from experience.

### 1.1 Contributions

This paper makes the following contributions:

1. **A systems architecture for sovereign agents** that unifies identity, payments, verification, and trajectory provenance with agent execution in real software environments.
2. **A declarative programming model for agent behavior** using typed signatures and composable modules, enabling reproducible, optimizable agent cognition across heterogeneous model providers.
3. **Outcome-coupled training and optimization** for decision pipelines (complexity, delegation, recursion triggers), including counterfactual logging and shadow-mode evaluation to reduce regressions and Goodhart effects.
4. **An integration of RLM/FRLM into production agents**, formalizing recursive context externalization as an execution substrate for large codebases and long-running sessions, with cost-aware federation across local, cloud, and swarm lanes.
5. **An evaluation framework** that measures not only task success but also stability, cost-efficiency, repetition, verification progress, and resilience to objective hacking in autonomous coding settings.

## 2 Background and Related Work

OpenAgents sits at the intersection of (i) **declarative programming and compilation for foundation models**, (ii) **open-ended learning and auto-curricula**, (iii) **self-improving coding agents**, (iv) **long-horizon inference with externalized memory**, and (v) **market/identity primitives that make agents economically real**. This section reviews these threads and clarifies how OpenAgents composes them into a unified, production-oriented system.

### 2.1 Declarative LM Programming and Compilation

Early “prompt engineering” treated LMs as black boxes controlled through hand-written instructions and few-shot examples. As pipelines grew (retrieval → reasoning → tool use → verification), hand-tuned prompts became brittle: small changes to context or model provider often required extensive re-tuning, and prompt logic was duplicated across systems. Recent work reframes LM applications as **programs**: structured compositions of model calls, tools, and intermediate variables, analogous to classical software pipelines.

DSPy popularized a particularly useful abstraction: **Signatures** (typed I/O contracts expressed in natural language field names) and **Modules** (composable units whose internal prompting strategy is optimizable), paired with **optimizers** that compile programs to prompts/demonstrations for a target metric. This compilation view matters for OpenAgents because it turns “agent behavior” into an artifact that can be versioned, evaluated, and improved without rewriting orchestration code. In OpenAgents, signatures provide stable contracts across heterogeneous execution lanes (local inference, cloud APIs, swarm providers), and optimization (e.g., MIPRO-style prompt search) provides the mechanism for systematic improvement.

OpenAgents extends this line in two ways. First, it treats routing decisions (complexity, delegation, recursion triggers) as **first-class compiled components**, not as ad-hoc heuristics. Second, it emphasizes **outcome-coupled optimization**: rather than optimizing only for format correctness or synthetic evals, it ties optimization to real execution outcomes such as test deltas, cost, repetition, and verified completion in real repositories.

### 2.2 Auto-Curricula and Interestingness in Open-Ended Learning

Open-ended learning aims to generate a continual stream of novel, learnable behaviors without a fixed terminal objective. A central challenge is **task selection** in vast spaces: uniform sampling wastes effort on impossible tasks, while learning-progress-driven curricula can become distracted by endless variants of trivial tasks. This mirrors Goodhart-style failure modes: when a metric becomes a target, it can be optimized in ways that miss the intended goal.

The OMNI family addresses this by introducing **Models of Interestingness**: using foundation models as a proxy for human judgments about what is novel, worthwhile, and meaningfully distinct. OMNI demonstrates that adding “interestingness” filtering to learning progress improves open-ended training in both finite and effectively infinite task spaces. OMNI-EPIC extends the paradigm further by generating **environments programmed in code**, expanding the space of possible tasks by synthesizing executable environments and reward checks.

OpenAgents draws a direct analogy: the “task space” for an autonomous coding agent is enormous, and naive progress signals (e.g., “we did something” or “we produced valid JSON”) can be hijacked by low-value behaviors such as repeated file openings, unproductive edits, or verbose-but-incorrect synthesis. OpenAgents operationalizes interestingness as **utility under verification**, with metrics grounded in software development reality: test improvements, build success, reduced repetition, and controlled cost. In this view, OMNI’s core insight becomes a systems requirement: successful autonomy requires a notion of “worth doing next,” not merely “learnable” or “syntactically valid.”

### 2.3 Self-Improving Agents via Empirical Validation

Self-improvement has long been studied as a theoretical concept (e.g., Gödel machines), but practical systems require replacing formal proofs of improvement with **empirical evaluation**. Recent work on self-improving coding agents explores loops where an agent modifies parts of its own scaffolding, tools, and workflows, then validates improvements on coding benchmarks. The Darwin Gödel Machine (DGM) formalizes a particularly relevant pattern: (i) self-modify, (ii) evaluate on downstream tasks, (iii) retain variants in an archive, and (iv) explore multiple evolutionary branches so that stepping stones can yield later breakthroughs.

OpenAgents adopts the same empirical posture but targets a more product-oriented surface area first: instead of evolving the entire agent codebase immediately, it evolves the **compiled cognition layer** (signatures, instructions, routing policies) and the **execution policies** that govern tool use, delegation, and recursion. This choice reduces risk and improves reproducibility: optimization changes are expressed as explicit, versionable prompt/program artifacts rather than arbitrary code mutations. Nonetheless, the DGM lesson remains central: greedy “always update the latest” loops are fragile; archives, counterfactual logging, and stepping-stone exploration help escape local optima and prevent regressions.

OpenAgents also incorporates DGM’s warning about **objective hacking**. Any measurable proxy (format checks, superficial heuristics, even pass/fail on a narrow evaluation slice) can be gamed. OpenAgents therefore emphasizes multi-signal evaluation (verification delta + cost + repetition + outcome), shadow-mode counterfactuals (DSPy vs legacy), and traceability of changes to mitigate Goodhart effects in self-improvement loops.

### 2.4 Long-Horizon Inference and Externalized Context

A persistent obstacle for agent autonomy is long-horizon reasoning over large contexts: repositories with thousands of files, long tool traces, and multi-hour sessions. Simply expanding context windows often yields diminishing returns: models exhibit “context rot,” attention diffusion, and rising cost. Tool-using agent methods (e.g., ReAct-style loops) help by enabling external actions, but they do not by themselves solve the problem of representing and manipulating extremely large state.

Recursive Language Models (RLMs) propose an inference-time strategy for effectively unbounded context: rather than ingesting the entire state into a single prompt, a “root” model interacts with a programmable environment (e.g., a REPL) that holds large context and supports operations like search, partitioning, summarization, and recursive sub-queries. This makes long-context reasoning a **procedural interaction** rather than a single monolithic forward pass.

OpenAgents integrates RLM/FRLM as an execution substrate: when tasks exceed context or require deep global reasoning, the agent switches into a recursive mode that externalizes state, performs targeted retrieval and transformations, and optionally federates sub-queries across local models, cloud APIs, or swarm providers. This complements DSPy: DSPy compiles *what the agent should do*; RLM provides the substrate that makes those policies robust under large-scale state and long-run execution.

### 2.5 Markets, Identity, and Verification for Agent Economies

Most agent systems implicitly assume a trusted operator: a human provides API keys, pays for compute, and absorbs the risk of failures. This assumption becomes a bottleneck for scaling fleets of agents and for enabling autonomous cooperation across organizational boundaries. A parallel line of work argues that agents require **identity, budgets, and verification** to be first-class participants in open ecosystems.

OpenAgents takes this seriously as a core design constraint. Agents authenticate and sign actions, maintain budgets, and purchase compute across heterogeneous providers. Work products are coupled to **verifiable receipts**: deterministic verification (tests/builds/sandbox runs) and traceable trajectories enable external observers to audit not only outputs but the process that produced them. This is the systems analog of the “empirical validation” principle: improvements and transactions should be grounded in objective checks whenever possible, and subjective tasks should be explicitly recognized as requiring judges, consensus, or human feedback.

This market framing also shapes how OpenAgents relates to DSPy and self-improvement: compilation and optimization are not just about quality, but about **cost-aware performance under constraints**. A signature that is accurate but expensive may be dominated by a slightly less accurate but far cheaper policy when operating under budget limits. In OpenAgents, evaluation therefore includes resource-aware metrics (tokens, tool calls, latency) alongside correctness and verification, aligning “agent intelligence” with economic reality.

---

**Positioning OpenAgents.** Taken together, prior work provides the ingredients for self-improving agents: compiled LM programs (DSPy), interestingness-aware task selection (OMNI), empirically validated self-modification with archives (DGM), and scalable long-horizon reasoning (RLM). OpenAgents contributes a synthesis that is explicitly production-aligned: it binds these ideas to the realities of software development (tests/builds as rewards), heterogeneous inference markets (local/cloud/swarm), and the economic + cryptographic primitives needed for agents to operate as autonomous actors.

## 3 System Overview

OpenAgents is a full-stack system for autonomous agents that (i) execute real work in real environments, (ii) learn from outcomes, and (iii) operate under economic and verification constraints. The central design goal is **continuous improvement without brittle hand-tuning**, while remaining **auditable, reproducible, and cost-aware** across heterogeneous execution backends.

### 3.1 OpenAgents Stack and Design Principles

OpenAgents is organized as a layered stack. At the top are user-facing products (Autopilot, Onyx, GitAfter). Beneath them is an execution layer (Adjutant + Autopilot loop) built on a programming and compilation substrate (dsrs/DSPy). Below that sits infrastructure for execution, routing, identity, payments, and marketplace coordination (Pylon, Nexus, Gateway, Protocol, Runtime). The core system insight is that *agent capability is not only model intelligence*—it is the interaction of:

1. **Compiled Cognition:** Agent behavior is expressed as typed programs (Signatures + Modules), not prompt strings. These programs are optimizable using offline or online compilation (e.g., MIPROv2).
2. **Verifiable Execution:** Agents operate in environments with deterministic checks (tests/builds/sandbox runs). Verification is treated as the primary ground-truth reward signal for software tasks.
3. **Scalable Reasoning:** Large contexts and long sessions are handled by Recursive Language Models (RLM/FRLM), which externalize state and allow recursive subqueries without prompting the full context.
4. **Economic Grounding:** Agents hold budgets and pay for compute, enforcing cost-awareness as a first-class constraint. Agents can buy inference or execution services from local backends, cloud APIs, or a decentralized swarm.

These principles are reflected in three “system invariants”:

* **Everything has a contract:** Every cognitive step has a Signature; every job has a schema; every provider has a manifest; every result has a parseable output.
* **Everything is scored:** Every decision is evaluated (format + utility + cost + verification delta) and logged as training signal.
* **Everything is replayable:** Trajectories, tool calls, and verification steps are recorded such that an auditor (human or machine) can reconstruct what happened and why.

### 3.2 Threat Model and Assumptions

OpenAgents targets real-world autonomy while explicitly acknowledging that frontier models can hallucinate, drift, and Goodhart against naive metrics. We therefore define the system around pragmatic assumptions:

**Assumptions**

* For software tasks, **objective verification** (tests/builds/lints) is available for a meaningful subset of work.
* Agents can execute within **sandboxed environments** that constrain filesystem/network access, runtime duration, and tool permissions.
* Provider availability and model quality may fluctuate; routing must tolerate failures and degrade gracefully.

**Adversaries / Failure Modes**

* **Hallucinated tool use / fabricated results:** The model claims to run a command or read a file without actually doing so.
* **Degenerate progress loops:** The agent repeatedly executes low-value actions that appear plausible but don’t improve verification state.
* **Objective hacking / Goodhart effects:** The agent optimizes proxy metrics (format, confidence, partial checks) instead of the underlying task outcome.
* **Provider-level faults:** Unreliable providers, latency spikes, incomplete results, or malicious behavior (e.g., returning plausible but incorrect outputs).
* **Context rot:** Model performance degrades as session history grows, tool logs accumulate, and prompts become cluttered.

OpenAgents addresses these with: sandboxing, explicit tool mediation, verification-driven success criteria, outcome-coupled metrics, counterfactual logging, and RLM-based context externalization.

### 3.3 Execution Substrates: Local, Cloud, Swarm

OpenAgents runs the same agent program across multiple execution substrates. This is crucial both for cost control (use cheap local inference when possible) and capability scaling (burst to stronger models when needed).

#### 3.3.1 Local Execution

Local execution includes:

* local inference backends (e.g., Ollama/llama.cpp, on-device models)
* local tool execution (bash, file edits, compilation/tests)
* local storage of trajectories, datasets, and policy bundles

Local mode is the default for sovereignty and privacy. The agent can still delegate specific subproblems outward, but the control loop and ground-truth verification remain local.

#### 3.3.2 Cloud Execution

Cloud execution refers to calling hosted frontier models (e.g., Codex-style endpoints, specialized reasoning models) via the Gateway abstraction. Cloud calls are treated as **paid resources** with explicit cost accounting. Cloud lanes are typically used for:

* high-complexity planning
* non-localizable reasoning (when local models are too weak)
* high-stakes synthesis steps that benefit from stronger models

#### 3.3.3 Swarm Execution

Swarm execution is a decentralized provider network where inference and sandboxed execution are offered as purchasable services. Jobs are broadcast and fulfilled via a coordination layer (e.g., Nostr-based job messaging), with payment and receipts handled through the wallet layer. Swarm mode is used for:

* parallel subqueries (fan-out retrieval or evaluation)
* specialized capabilities (a provider advertises a model or tool the local host lacks)
* burst compute when local resources are limited

Critically, the **programming model is identical** across substrates because the agent interacts through a uniform abstraction:

* `/compute/*` for model calls (local/cloud/swarm)
* `/containers/*` for sandboxed execution
* `/wallet/*` for budgeted spending
* `/identity/*` for authentication and signing

### 3.4 Agent Execution Flow

At a high level, OpenAgents executes tasks through an iterative loop:

1. **Plan:** Produce a structured plan (subtasks with action types and targets).
2. **Act:** Execute subtasks via tool calls (read/edit/bash/sandbox).
3. **Verify:** Run deterministic checks (tests/builds) and compute deltas.
4. **Synthesize:** Summarize results, update state, decide next iteration.
5. **Route:** On each iteration, decide whether to delegate, recurse (RLM), or continue locally.

Each stage is implemented as a DSPy-style signature/module pair, meaning the agent’s cognition is explicitly typed and optimizable. Decision pipelines (complexity, delegation, recursion trigger) run alongside the loop and can override legacy heuristics when confidence is high.

### 3.5 Data Products: Trajectories, Datasets, and Policy Bundles

OpenAgents treats every run as valuable training and evaluation data.

* **Trajectories:** A full record of decisions, tool calls, intermediate outputs, and verification steps. Trajectories are the primary debugging and auditing artifact.
* **Training datasets:** Structured examples extracted from trajectories. These include both “format correctness” examples (does the module output parse?) and “outcome-coupled” labels (did this decision reduce failing tests? did it waste budget?).
* **Policy bundles:** Versioned compilations of signatures/modules/routing thresholds resulting from optimization runs. Bundles are reproducible artifacts: a specific bundle can be deployed, A/B tested, rolled back, or distilled into cheaper lanes.

These data products enable both offline compilation (batch MIPROv2) and online self-improvement (triggered recompilation when rolling accuracy drops).

### 3.6 Summary

OpenAgents is designed to make autonomy practical by binding together:

* a **compiler layer** for agent cognition (signatures/modules/optimizers),
* a **verified execution layer** (tools + deterministic checks),
* a **scalable reasoning layer** (RLM/FRLM for massive contexts),
* and an **economic substrate** (identity, budgets, paid compute).

The remainder of the paper details the programming model (§4), runtime (§5), routing (§6), RLM integration (§7), self-improvement loop (§8), marketplace mechanics (§9), evaluation (§10), and safety (§11).

## 4 Programming Model

OpenAgents adopts a declarative programming model for agent cognition, inspired by DSPy, but extended for long-horizon autonomy, multi-provider execution, and outcome-coupled optimization. The key idea is to treat agent behavior not as ad-hoc prompt strings, but as a **typed, optimizable program** composed of contracts (“Signatures”) and executable policies (“Modules”). This section defines the abstractions, explains how they compose into the Autopilot loop, and describes how we make them optimizable and reproducible.

### 4.1 Signatures as Typed Contracts for Agent Cognition

A **Signature** specifies a transformation from inputs to outputs. Conceptually, a signature is the unit of “agent thought” that we can independently measure and improve. OpenAgents uses signatures for both *work* (planning, executing, synthesizing) and *decisions* (complexity classification, delegation, recursion triggers).

**Design goals for signatures:**

1. **Explicit I/O:** Every input and output is named, typed, and validated.
2. **Model-facing semantics:** Field names are chosen for clarity because they become part of the model’s prompt contract.
3. **Stable interface:** The signature is the compatibility boundary across models, providers, and optimizers.
4. **Optimizable instruction surface:** The signature carries an instruction (or docstring) that can be compiled/optimized.

We use several signature families:

* **Execution signatures** (core loop):

  * `Plan`: `task_title, task_description, context → subtasks, reasoning, confidence`
  * `ExecuteSubtask`: `action, target, instruction, file_context → result, reasoning, success`
  * `Synthesize`: `task_title, subtask_results → success, summary, modified_files, confidence`

* **Decision signatures** (routing layer):

  * `Complexity`: `task_description, file_count, estimated_tokens, keywords → complexity, reasoning, confidence`
  * `Delegation`: `task_description, complexity, file_count, estimated_tokens → should_delegate, target_lane, reasoning, confidence`
  * `RLM Trigger`: `task_description, complexity, estimated_tokens → use_rlm, reasoning, confidence`

Two conventions are enforced throughout:

* **Every decision signature returns `confidence`**, enabling safe gating (only override legacy rules when confidence is high).
* **Every execution signature returns a parseable structure**, enabling downstream automation and objective checks.

### 4.2 Modules as Composable Policies

A **Module** is an executable policy that implements one or more signatures. Modules can be nested to form multi-step programs. In OpenAgents, modules serve three roles:

1. **Single-step predictors**: e.g., a `Predict` module implementing `SubtaskPlanningSignature`.
2. **Composite modules**: e.g., an `AdjutantModule` that chains plan → act → synthesize.
3. **Control modules**: wrappers that manage retries, tool mediation, verification, and provider selection.

OpenAgents’ core composite module follows a three-phase design:

**Phase 1: Plan**

* Produce a structured list of subtasks with action types (`read`, `edit`, `bash`, etc.).
* The output is treated as a *program sketch* that downstream phases execute.

**Phase 2: Act**

* Execute subtasks through a tool layer with explicit mediation.
* Each subtask execution produces a structured `result` artifact (often JSON) plus reasoning and success.

**Phase 3: Synthesize**

* Convert execution artifacts into a final outcome: summary, success boolean, modified file list, and calibrated confidence.

This pattern matters because it yields a stable, testable intermediate representation: the “subtask list” acts as an internal plan language. That internal language becomes a natural target for optimization (e.g., better subtask decomposition, fewer wasted actions, more consistent target-path selection).

### 4.3 Metrics: From Format Correctness to Outcome-Coupled Utility

Optimizing an agent requires measurable feedback. OpenAgents uses a two-tier metric model:

#### 4.3.1 Format and Contract Metrics (Local Correctness)

At minimum, modules must be well-formed:

* valid JSON (when expected)
* required fields present
* action types in allowed set
* target paths plausible
* confidence in [0, 1]
* reasoning non-empty and substantive

These metrics are easy to compute and prevent a common failure mode: systems that “look good” to humans but are structurally unusable by code.

#### 4.3.2 Outcome-Coupled Metrics (Task Utility)

Format correctness is necessary but insufficient; it can be Goodharted. OpenAgents therefore also defines outcome-coupled signals computed post-hoc from the execution loop:

* **Verification delta**: change in failing tests/build status after an action or iteration
* **Step utility**: scored progress signal (e.g., +1.0 for directly advancing toward passing tests, negative for regressions)
* **Repetition penalty**: detect repeated tool calls with identical inputs/outputs
* **Cost penalty**: token and tool-call consumption, optionally priced by lane/provider
* **Stability penalty**: regressions introduced (e.g., new failing tests), thrash behavior, or non-convergent loops

These metrics are used both for offline compilation (optimizer training) and for online monitoring (rolling accuracy + drift detection).

### 4.4 Optimizers: MIPROv2 and Compilation as Policy Improvement

OpenAgents treats optimization as a compilation step: given a set of signatures/modules and a dataset of examples plus a metric, an optimizer produces improved prompt instructions and (optionally) demonstrations.

We emphasize two properties:

1. **Lane portability:** The same signature can be compiled separately per lane (local small model vs cloud frontier model), producing lane-specific instruction sets.
2. **Incremental recompilation:** Optimization is not a one-off; it is triggered when drift is detected or sufficient new labeled examples exist.

In practice, compilation is used for:

* better plan decomposition
* improved delegation thresholds and reasoning
* calibration of confidence outputs
* reduction of repetitive tool behavior through utility-based scoring

### 4.5 Policy Bundles, Versioning, and Reproducibility

A compiled OpenAgents program produces a **policy bundle**—a versioned artifact containing:

* signature instructions (and any learned demos)
* optimizer configuration and run metadata
* metric definitions used for compilation
* lane routing priors and thresholds
* hashes of datasets used for training

Policy bundles allow:

* A/B testing (bundle A vs bundle B)
* rollbacks (pin to a known-good compilation)
* deterministic audits (“what policy produced this trajectory?”)
* distillation workflows (teach a cheaper lane from traces produced by a stronger lane)

This is essential for safety: without versioned policies, it is difficult to attribute failures to specific changes and hard to guarantee reproducibility across runs.

### 4.6 Design Implications

The programming model imposes a discipline that is unusual in today’s agent products:

* **No hidden prompts:** behavior lives behind typed contracts and versioned policies.
* **No unscored intelligence:** every decision is measurable and logged.
* **No provider lock-in:** signatures separate *what* is computed from *where* it is executed.

This discipline is what enables OpenAgents to connect “agent cognition” to open-ended improvement loops: once behavior is a program with contracts and metrics, continuous improvement becomes an engineering process rather than a guessing game.

The next section (§5) describes the runtime that executes these programs—tools, sandboxes, verification harnesses, and trajectory provenance—turning compiled cognition into reliable action.

## 5 Agent Runtime

OpenAgents’ programming model (§4) defines *what* the agent should compute. The runtime defines *how* those computations are executed safely, verifiably, and repeatably in real environments. The runtime is built around four requirements: (i) **tool mediation** (the model can act, but only through controlled interfaces), (ii) **sandboxed execution** (contain side effects), (iii) **objective verification** (ground truth signals for correctness), and (iv) **trajectory provenance** (auditability and training data).

### 5.1 Tick Model and State Surfaces

OpenAgents executes agents in discrete steps (“ticks”) to make long-running behavior inspectable and replayable. Each tick is a complete cycle of:

1. **WAKE**: load the agent’s identity, budgets, and session state
2. **LOAD**: hydrate workspace state (repo snapshot, plan, iteration counters, caches)
3. **PERCEIVE**: gather observations (tool outputs, file diffs, test results, provider health)
4. **THINK**: run compiled cognition (signatures/modules) to decide next actions
5. **ACT**: execute tool calls and/or delegate work to providers
6. **REMEMBER**: persist outputs, summaries, and derived state
7. **SCHEDULE**: choose next wake time or termination
8. **SLEEP**: yield control

This discrete-time model has two pragmatic benefits. First, it provides a natural unit of logging and evaluation (per-tick costs, progress, and regressions). Second, it enables deterministic replay: given the same repo snapshot, tool outputs, and policy bundle, the system can reproduce decision making for debugging or audit.

The runtime exposes the agent’s world as **state surfaces**—structured, inspectable objects that include:

* repository snapshot + diffs relative to a base commit
* plan state (subtasks pending/completed)
* verification state (test failures, build status, lint output)
* cost accounting (tokens, tool calls, provider spend)
* provider health and lane availability
* execution history (tool calls and results)
* session metadata (iterations, timestamps, termination reason)

### 5.2 Tooling and Sandboxed Execution

Agents interact with the world only through tools. Tools are explicit, versioned interfaces with typed schemas and deterministic logging. A tool invocation is a structured event:

* **tool name**
* **inputs** (schema-validated)
* **outputs** (serialized, logged)
* **runtime metadata** (latency, errors, side effects)
* **provenance** (policy bundle, lane/provider, tick number)

OpenAgents separates tools into two categories:

#### 5.2.1 Deterministic Tools

These are tools whose outputs are expected to be reproducible given the same state (e.g., file reads, directory listings, diff generation, running `cargo test` in a pinned container). Deterministic tools are the backbone of objective verification and replay.

Examples:

* file read/view (with optional line ranges)
* patch apply/revert
* structured string replacement tools (for precise edits)
* `git diff` and repo status queries
* sandboxed test/build commands

#### 5.2.2 Non-deterministic or External Tools

These include network calls, remote API calls, or any tool whose outputs may vary over time. OpenAgents treats these as “opaque” side effects and records them with stronger provenance requirements.

Examples:

* cloud model inference calls
* swarm job requests/results
* external web retrieval (when allowed)
* payment interactions

**Sandboxing.** Execution is contained using sandboxes with:

* filesystem scoping (workspace mounts)
* network policies (deny-by-default unless explicitly required)
* CPU/memory/time limits
* explicit allow-lists for commands and interpreters
* isolation per run (so a failed run doesn’t poison subsequent runs)

The runtime treats sandboxing as a safety feature *and* as a measurement primitive: sandbox boundaries define what counts as an objective check (e.g., “tests passed inside the sandbox” is a valid completion signal).

### 5.3 Verification Harnesses and Objective Checks

For software tasks, OpenAgents treats verification as the primary “reward function.” Each session defines a verification harness, typically a sequence of commands and checks that are run after iterations or after relevant tool actions.

A canonical harness includes:

* compilation/typechecking (e.g., `cargo check`)
* unit/integration tests (e.g., `cargo test`)
* linters/formatters (optional, policy-dependent)
* targeted regressions (task-specific commands)

Verification produces:

* pass/fail outcome
* structured failure summaries (e.g., failing test names)
* deltas relative to prior iteration (improved/worsened)
* artifacts (logs, coverage, diffs)

These signals are used in three ways:

1. **Termination criteria:** stop only when verification passes (or failure is definitive).
2. **Training labels:** compute step utility and decision correctness.
3. **Routing feedback:** decide when to escalate to stronger models or to RLM mode.

OpenAgents explicitly distinguishes:

* **objective verification** (tests/builds) that can be automatically trusted
* **subjective results** (summaries, refactors, “is the code clean?”) that require judges or human review

This distinction informs both payment/receipts and what outcomes can be used as ground truth for optimization.

### 5.4 Trajectory Logging and Provenance

Every Autopilot session emits a **trajectory**: a structured record of the entire run. The trajectory includes:

* policy bundle identifier and hashes
* signature inputs/outputs per call
* tool invocations and tool results
* provider lane choices + fallbacks
* verification history (commands, outputs, deltas)
* intermediate artifacts (plans, patches, summaries)
* cost accounting (tokens, time, provider spend)
* termination reason (success, max iterations, failure)

Trajectories support three essential properties:

1. **Auditability:** “What happened?” “What did the agent see?” “Why did it decide that?”
2. **Replayability:** reconstruct the run (or parts of it) to debug regressions.
3. **Data extraction:** convert trajectories into training datasets for compilation/optimization.

We treat trajectories as first-class data products. They can be stored locally by default and optionally contributed (redacted) for shared training, depending on privacy policies.

### 5.5 Runtime–Compiler Interface

The runtime and the DSPy compiler layer communicate through explicit boundaries:

* The runtime supplies **observations** (context slices, file contents, diffs, verification summaries) as signature inputs.
* Signatures return **structured outputs** that the runtime parses into actions (subtasks, tool calls, routing decisions).
* The runtime computes **outcome-coupled metrics** (verification delta, cost, repetition) and feeds them into:

  * online monitoring (rolling accuracy, drift)
  * dataset labeling (correct/incorrect decisions)
  * offline compilation (MIPROv2 training sets)

This separation ensures improvements in prompting/policy do not require changes to the runtime, and improvements in tooling do not require rewriting the signature contracts—both can evolve independently while remaining compatible.

### 5.6 Summary

The OpenAgents runtime turns “compiled cognition” into real-world action by enforcing tool mediation, sandbox boundaries, deterministic verification, and full provenance logging. It provides the substrate needed to (i) safely execute autonomous tasks, (ii) obtain reliable outcome signals, and (iii) generate the data required for continuous improvement. The next section (§6) describes how routing decisions are made across execution lanes and how counterfactual logging enables safe migration from legacy heuristics to compiled policies.

## 6 Decision Pipelines and Routing

OpenAgents must continuously decide *how* to execute a task: which model to use, whether to delegate, when to invoke recursion (RLM), and when to fall back to conservative rules. These are not incidental implementation details—routing decisions dominate reliability, cost, and convergence. OpenAgents therefore makes routing a first-class, optimizable layer implemented as **DSPy decision pipelines**, guarded by confidence thresholds and backed by counterfactual logging.

### 6.1 Complexity Classification

The first routing decision is an estimate of task complexity. Complexity predicts how many iterations, how much context, and what level of reasoning strength is likely required. OpenAgents uses a **ComplexityPipeline** that consumes lightweight features known early:

* task description
* file count in scope (from rule-based discovery or repo scan)
* estimated context tokens
* keyword hints (refactor, migrate, concurrency, etc.)

and outputs:

* `complexity ∈ {Low, Medium, High, VeryHigh}`
* `reasoning` (short explanation)
* `confidence ∈ [0, 1]`

**Why this matters:** Complexity gates downstream policies. Low/Medium tasks can often execute locally with cheap models and minimal recursion; High/VeryHigh tasks are likely to require stronger models, RLM/FRLM, or delegation to the swarm.

**Safety gating:** Complexity predictions only override legacy heuristics when confidence exceeds a fixed threshold (e.g., 0.7). Below threshold, the system uses conservative rule-based classification. This prevents early optimization from destabilizing behavior.

### 6.2 Delegation Decisions

Delegation determines whether the core Autopilot loop should run locally or be delegated to a specialized lane. OpenAgents models delegation as a prediction problem implemented by the **DelegationPipeline**. Inputs include:

* task description
* complexity label
* file count
* estimated tokens / context pressure

Outputs include:

* `should_delegate: bool`
* `delegation_target ∈ {codex_code, rlm, local_tools, swarm}`
* `reasoning`
* `confidence`

**Interpretation of delegation targets:**

* `local_tools`: run the normal tool loop locally (default).
* `codex_code`: delegate to a high-capability coding runtime (e.g., app-server-backed lane).
* `rlm`: switch the execution substrate to RLM/FRLM (see §7).
* `swarm`: fan out to a provider network for burst compute or specialized skills.

Delegation is conservative by default: a low-confidence “delegate” prediction does not trigger delegation. This avoids expensive or destabilizing routing.

### 6.3 RLM Triggering

RLM triggering is the “deep analysis switch.” It determines when standard prompting is likely to degrade due to context size or long-horizon state. The **RlmTriggerPipeline** consumes:

* task description
* complexity label
* estimated tokens / expected growth of state

and outputs:

* `use_rlm: bool`
* `reasoning`
* `confidence`

RLM triggering is not just a function of token count; it is also a function of expected *state complexity*. Some tasks require global reasoning across many files even if the prompt could fit. Conversely, some large contexts do not require deep global analysis if the task is localized. The trigger pipeline learns these distinctions over time using outcome-coupled feedback.

### 6.4 Provider Lane Selection and Fallback Strategies

Once delegation and recursion decisions are made, OpenAgents selects a provider lane. Lanes differ by capability, latency, cost, and availability. The system must be robust to failures (provider down, rate limits, missing models) and must remain economical.

OpenAgents uses a multi-stage routing strategy:

1. **Lane eligibility:** filter lanes by availability, authentication, and required capabilities.
2. **Lane preference:** choose the best lane given complexity and budget policy (e.g., prefer local for Low/Medium unless confidence is low).
3. **Health-aware dispatch:** route to a primary lane, but maintain fallbacks in priority order.
4. **Circuit breakers:** if a lane repeatedly fails, temporarily down-rank it and fallback earlier.

This routing is intentionally separate from the DSPy decision pipelines: the pipelines determine *what class of execution to use* (delegate/rlm), while the lane router determines *which provider instance* should serve the request within that class.

### 6.5 Counterfactual Recording and Shadow Mode

A central challenge in migrating from hand-built heuristics to learned/compiled policies is avoiding regressions. OpenAgents therefore records counterfactuals: for every decision, it stores both:

* **DSPy output** (what the pipeline predicted)
* **legacy output** (what rules would have chosen)
* whether DSPy was used or fell back
* the reason for fallback (low confidence, parse error, timeout, provider missing)

This enables two critical capabilities:

#### 6.5.1 Offline Policy Evaluation

After a session completes and an outcome is known (success/failure/max iterations), we can estimate:

* whether DSPy decision was “correct”
* whether legacy decision would likely have been “correct”
* cases where DSPy outperformed legacy (DSPy wins)
* cases where legacy would have outperformed DSPy (legacy wins)

This is the foundation for safe iteration: we can improve pipelines using data where legacy clearly beats DSPy, without guessing.

#### 6.5.2 Safe Gradual Rollout

Shadow mode allows DSPy to run in parallel without controlling behavior. In shadow mode:

* legacy decisions are executed
* DSPy decisions are recorded
* no behavior changes occur

Once DSPy performance is strong and stable in shadow mode, the confidence gating threshold can be lowered or DSPy can be enabled for specific decision types first (e.g., enable complexity classification before enabling delegation). This incremental rollout significantly reduces operational risk.

### 6.6 Outcome-Coupled Labeling of Decisions

After session completion, OpenAgents maps outcomes to decision correctness. Correctness is not treated as an abstract label; it is derived from execution performance:

* **Complexity correctness:** whether the session converged within expected iteration bounds and context constraints for that complexity level.
* **Delegation correctness:** whether delegation improved or harmed outcomes, factoring cost and failure attribution (e.g., a failure in a delegated lane may be less “wrong” if local lane would likely fail).
* **RLM correctness:** whether recursion was necessary given token growth and verification trajectory; e.g., tasks with >100k effective context or high thrash are strong signals that RLM should have been used.

These labels feed into rolling accuracy trackers per signature and determine when auto-optimization triggers (§8).

### 6.7 Summary

Decision pipelines transform routing from brittle heuristics into measurable, optimizable policy. OpenAgents keeps these policies safe via confidence gating, records counterfactuals to quantify regressions, and uses outcomes to label decisions and improve them over time. The next section (§7) details how Recursive Language Models are integrated as an execution substrate and how federated recursion (FRLM) allows OpenAgents to scale reasoning across local, cloud, and swarm compute while maintaining cost-aware control.

## 7 Recursive Language Models in OpenAgents

OpenAgents integrates Recursive Language Models (RLMs) to address a recurring failure mode in autonomous agents: **long-horizon reasoning over state that grows without bound** (large repos, long tool traces, multi-hour sessions). In standard agent loops, the model’s prompt accumulates history, diff logs, error messages, and file contents until either the context window is exceeded or the model’s behavior degrades (“context rot”). RLMs provide an execution substrate that externalizes large state into a programmable environment and enables **recursive sub-queries** over targeted slices, keeping the model’s working context small while still supporting global reasoning.

### 7.1 RLM as an Execution Substrate

In OpenAgents, RLM is treated as a *mode of execution*, not a different product. From the perspective of the programming model (§4), an RLM-backed executor behaves like an LM provider that can answer prompts. The difference is that the “LM call” becomes an interactive procedure:

* The root model has access to a **context store** that may contain millions of tokens (repo snapshot, diffs, logs, docs).
* The root model interacts with this store through a small set of **context operations** (search, slice, summarize, partition).
* For expensive reasoning, the root model can launch **recursive subcalls** on subsets of the context and integrate their outputs.

This turns long-context reasoning from “stuff everything into one prompt” into a controlled process of **progressive disclosure**: the model decides what to look at next, rather than being forced to attend to everything.

OpenAgents uses RLM in two ways:

1. **Deep analysis** for large tasks (global refactors, multi-component bugs, architectural changes).
2. **Stability over time** for long sessions where repeated verification and tool use would otherwise bloat the prompt.

### 7.2 Context Externalization and REPL Environments

OpenAgents’ RLM implementation externalizes state into a runtime-owned environment. Practically, this is a **programmatic workspace** containing:

* a repository snapshot and file index
* diffs relative to a base commit
* tool execution history
* verification artifacts (failing tests, stack traces, logs)
* cached summaries and embeddings (optional)

The root model does not receive this state directly. Instead, it can call operations exposed as tools. A minimal context tool API includes:

* `peek(path, range)` – read a slice of a file
* `grep(pattern, scope)` – search across files and logs
* `partition(scope, strategy)` – create a set of chunks (by file, symbol, or error locality)
* `map(query, chunks)` – apply a sub-query over many chunks
* `summarize(chunks)` – compress chunks into a stable summary
* `diff(base, head)` – compute patch deltas
* `rank(candidates, objective)` – select relevant chunks or hypotheses

The design principle is that these operations are **cheap, deterministic, and replayable** whenever possible. This allows RLM runs to produce stable trajectories that can be used as training signal for compiled policies.

### 7.3 FRLM: Federated Recursion Across Providers

OpenAgents extends RLM to **FRLM (Federated RLM)**, where recursive subcalls can be executed across heterogeneous lanes:

* **Local lane**: fast, private, cheap inference and context ops
* **Cloud lane**: strong frontier models for difficult reasoning
* **Swarm lane**: distributed parallel subqueries via paid jobs
* **Specialized lane**: e.g., code-only models, embedding rerankers, lint analyzers

FRLM enables a “fan-out / gather / synthesize” pattern:

1. The root model identifies an uncertain or broad subproblem (e.g., “find all callsites that could trigger this panic”).
2. It partitions the search space (files, modules, symbols).
3. It dispatches subqueries to multiple lanes in parallel (often swarm + local).
4. It gathers results, ranks them, and produces a consolidated hypothesis.
5. It proceeds with targeted edits and verification.

Federation is policy-driven: the decision pipelines (§6) and budget policies (§9) determine whether recursion stays local, escalates to cloud, or fans out to the swarm.

### 7.4 Cost/Latency Tradeoffs and Stopping Criteria

RLM-style recursion can waste budget if unconstrained. OpenAgents therefore treats recursion as an explicitly budgeted activity and enforces stopping criteria at both the root and subcall levels.

**Budget controls include:**

* maximum recursion depth
* maximum number of subcalls per tick
* per-lane spend caps
* token budgets per subcall
* early stopping when verification delta is positive and confidence is high

**Stopping criteria include:**

* verification success (tests pass) → terminate
* hypothesis stabilization (no new evidence after N searches)
* diminishing returns (subcalls produce redundant information)
* timeouts or budget exhaustion
* detection of thrash patterns (repeated identical queries or file reads)

These constraints are enforced at runtime, but the policies controlling them can also be compiled and optimized over time (e.g., learning what recursion budgets correlate with successful outcomes for different complexity classes).

### 7.5 RLM–DSPy Integration Patterns

RLM is not “instead of DSPy.” DSPy defines the agent’s cognitive structure; RLM provides the substrate that makes that structure reliable at scale. OpenAgents uses three integration patterns:

#### 7.5.1 RLM-backed Predictors

Any signature can be executed using an RLM-backed LM provider. This is the simplest integration: the module calls `Predict(Signature)` but the underlying LM is RLM-capable. The root model then uses context ops and recursion implicitly to satisfy the signature contract.

#### 7.5.2 Context Handles Instead of Raw Context

For large tasks, signatures can accept **context handles** rather than raw text. For example, planning might take:

* `repo_handle` (pointer to snapshot + index)
* `failure_handle` (pointer to failing tests/logs)
  rather than concatenated text. The model then retrieves only needed slices via context ops. This prevents prompt bloat and makes plans more grounded.

#### 7.5.3 Outcome-Coupled Metrics for Context Navigation

OpenAgents treats context navigation as part of policy quality. We measure:

* cost per useful verification improvement
* repetition of context ops (thrash)
* whether recursion led to actionable edits
* latency and provider spend per unit progress

These signals can be fed back into compilation: prompts can be optimized to prefer cheaper context ops first, avoid redundant searches, and escalate only when evidence suggests it’s needed.

### 7.6 Failure Modes and Mitigations

RLM introduces its own risks:

* **Search myopia:** over-reliance on grep-like retrieval can miss non-obvious dependencies.
* **Recursive hallucination:** subcalls may amplify incorrect hypotheses if not checked.
* **Cost blowups:** uncontrolled fan-out can exceed budgets rapidly.
* **Opaque reasoning:** heavy recursion can make the “why” harder to audit unless logged carefully.

OpenAgents mitigates these by:

* enforcing structured provenance for each context op and subcall
* requiring synthesis steps to cite evidence (file paths, line ranges, diffs)
* coupling recursion to verification deltas (prefer hypotheses that reduce failing tests)
* using conservative defaults (local-first, bounded recursion) and escalating only when confidence warrants

### 7.7 Summary

RLM/FRLM provides OpenAgents with a scalable, robust substrate for long-horizon reasoning in environments where state grows without bound. By externalizing context into a programmable environment and enabling recursive subqueries—optionally federated across local, cloud, and swarm compute—OpenAgents can maintain stable agent performance on large repositories and long sessions while remaining cost-aware and auditable. The next section (§8) describes how OpenAgents closes the loop: how session outcomes label decisions, how rolling accuracy is tracked per signature, and how automatic optimization is triggered to produce continuously improving policy bundles.

## 8 Self-Improvement Loop

OpenAgents is designed to improve over time without requiring continuous manual prompt tuning. It achieves this by treating every autonomous run as both (i) task execution and (ii) data generation for policy improvement. The self-improvement loop links *compiled cognition* (§4), *verifiable execution* (§5), *routing* (§6), and *RLM/FRLM execution* (§7) into an outcome-coupled training and compilation cycle. This section describes how sessions are recorded, how decisions are labeled, how performance is tracked, how optimization is triggered, and how we mitigate Goodhart-style pathologies.

### 8.1 SessionStore: A First-Class Record of Autonomy

OpenAgents persists each autopilot run as an **AutopilotSession**. A session includes:

* task title and description
* timestamps (start/end)
* iteration count and termination reason
* all decision records (complexity, delegation, RLM trigger, lane choice)
* all tool invocations (read/edit/bash/sandbox)
* verification history (test/build runs and outputs)
* trajectory links and policy bundle identifiers

Sessions are append-only, versioned, and stored locally by default. This serves two purposes:

1. **auditability** (what happened and why)
2. **supervision** (how to label decisions based on outcomes)

### 8.2 Outcome Feedback: Turning Runs into Labeled Examples

The key transition from “logging” to “learning” is labeling. After a session ends, OpenAgents computes a **SessionOutcome**:

* `Success`: verification passed and task completed
* `Failed`: definitive failure (e.g., unfixable constraints, repeated regressions)
* `MaxIterations`: budget exhausted or iteration cap reached

From this outcome and the session trace, OpenAgents constructs labeled examples at two levels:

#### 8.2.1 Decision Labels (Routing Supervision)

Each decision record is labeled as correct/incorrect (or soft-labeled) based on outcome and trajectory properties.

Examples:

* **Complexity correctness:** Did the task converge within expected bounds for the predicted complexity? Were context or recursion decisions consistent with observed token growth and iteration count?
* **Delegation correctness:** Did delegation improve success probability or cost-adjusted performance? If a delegated lane failed but local would likely fail as well, penalize less.
* **RLM correctness:** Was recursion necessary (token growth, thrash patterns, global reasoning needs)? Did using RLM correlate with improved verification deltas?

These labels become datasets for compiling decision signatures.

#### 8.2.2 Execution Labels (Tool and Planning Supervision)

For planning and synthesis signatures, OpenAgents records examples when:

* the run succeeded, or
* confidence was high and verification trajectory supports usefulness.

For tool steps, OpenAgents can derive **outcome-coupled labels** such as:

* step utility (helpful / neutral / harmful)
* verification delta after the step
* repetition flag (was this essentially the same action as before?)
* cost accounting (tokens, tool calls, time)

This is crucial: format-only metrics can be optimized without improving autonomy. Outcome-coupled labels prevent “pretty JSON” from becoming the target.

### 8.3 Performance Tracking: Rolling Accuracy and Drift

OpenAgents maintains a **PerformanceTracker** that computes rolling accuracy over a fixed window (e.g., last 50 labeled decisions) per signature:

* accuracy per decision pipeline signature
* success rate per execution signature (plan/act/synthesize)
* cost-adjusted success metrics (success per token, success per dollar)
* stability metrics (regression frequency, repeated-action rate)

This tracker powers two system behaviors:

1. **Monitoring:** Identify regressions in policy performance after deploying a new bundle.
2. **Auto-optimization triggers:** Decide when to recompile and what to recompile.

Rolling windows matter because the environment changes over time:

* different repos and frameworks
* different model versions and providers
* different tasks and user patterns

The tracker is therefore also a drift detector: if accuracy drops below a threshold, that’s evidence that policy needs recompilation.

### 8.4 Auto-Optimization: When to Recompile and What to Target

OpenAgents treats compilation (e.g., MIPROv2) as a background maintenance operation triggered by concrete criteria. A typical trigger policy checks:

* **data availability:** have we accumulated enough new labeled examples? (e.g., ≥ 20)
* **accuracy:** did rolling accuracy drop below threshold? (e.g., < 70%)
* **staleness:** has it been long enough since last optimization? (e.g., > 24h)
* **budget constraints:** do we have optimization budget available now?

When optimization is triggered, OpenAgents selects a target signature:

* typically the **lowest-accuracy** signature with sufficient data
* optionally weighted by impact (errors in delegation may cost more than errors in complexity)

This approach is intentionally incremental: optimize one high-impact piece at a time, validate it, then proceed. It reduces the risk of “global rewrites” that destabilize the whole system.

### 8.5 Avoiding Goodhart Pathologies

Self-improving systems are vulnerable to Goodhart’s law: any single proxy metric can be gamed. OpenAgents mitigates this through a multi-signal design:

#### 8.5.1 Multi-Objective Scoring

We score behavior using a weighted combination of:

* verification improvement
* step utility
* repetition penalties
* cost penalties
* convergence speed (iterations to success)
* stability (avoid regressions)

This makes it harder to “win” by optimizing a single proxy (e.g., verbose reasoning or format correctness).

#### 8.5.2 Counterfactual Baselines

Because we record what the legacy system would have done, we can detect when DSPy policy is worse than established heuristics. This prevents the optimizer from “learning itself into a hole,” and gives concrete training signals (the cases where legacy wins).

#### 8.5.3 Gating and Safe Deployment

Optimized policies are only used when:

* signature confidence is high, or
* shadow mode evaluation indicates consistent wins.

Otherwise, legacy behavior remains the fallback. This allows learning without destabilizing production behavior.

#### 8.5.4 Anti-Thrash Constraints

The runtime enforces hard limits on:

* recursion depth and fanout
* repeated identical tool calls
* total tool calls per iteration
* budget spending

These constraints prevent the system from finding degenerate strategies that burn compute without progress.

### 8.6 Archives and Stepping Stones: Toward Open-Ended Improvement

The Darwin Gödel Machine perspective emphasizes that non-greedy search and archives enable stepping stones: intermediate improvements that later unlock bigger gains. OpenAgents currently implements an *implicit archive* in three forms:

1. **Trajectory archive:** a growing repository of runs, failures, and recoveries
2. **Dataset archive:** labeled examples and outcomes over time
3. **Policy bundle archive:** versioned compiled policies with recorded performance

This is sufficient to realize “stepping stone” benefits without immediately evolving agent codebases. For example:

* A policy bundle might improve delegation accuracy but slightly reduce local success rate.
* Later, a separate optimization to reduce repetition might unlock the full benefit of delegation.
* Archives allow the system to revisit and recombine improvements rather than committing to a single greedy lineage.

A natural extension (future work) is explicit branching: keeping multiple top-performing policy bundles active and sampling them under controlled A/B policies, approximating evolutionary selection while maintaining safety gates.

### 8.7 Summary

OpenAgents closes the loop from autonomy to improvement by (i) recording full sessions, (ii) labeling decisions and steps based on outcomes and verification deltas, (iii) tracking rolling accuracy and drift per signature, and (iv) triggering incremental compilation when performance degrades or enough data accumulates. The design is intentionally conservative—confidence gating, counterfactual baselines, and multi-objective metrics—to avoid Goodhart pathologies and regressions. The next section (§9) describes how this self-improvement loop interacts with OpenAgents’ market substrate: identity, payments, budgets, and provider selection in a decentralized compute marketplace.

## 9 Marketplace and Economic Constraints

OpenAgents treats economic constraints as a first-class control surface for autonomy. Instead of assuming an agent can freely call any model or run unlimited compute, OpenAgents places agents inside an explicit economy: agents have identities, hold budgets, pay for compute, and are accountable for the costs they incur. This framing is not only product-relevant—it is technically stabilizing. Many failure modes in autonomous agents (thrash, over-exploration, unnecessary escalation) are best controlled by making actions *costful* and therefore optimizable under constraints.

### 9.1 Identity and Authentication

To participate in an open marketplace, agents require stable, cryptographic identity. OpenAgents assigns each agent a long-lived identity used to:

* authenticate to coordination relays and providers
* sign job requests and results
* build reputations and maintain accountability
* bind trajectories and receipts to a verifiable actor

Identity is treated as infrastructure, not an application feature: it is used by all layers (runtime, marketplace, and product UIs). This enables interactions such as “this provider produced this result using this policy bundle and this model” to be verifiable without trusting a centralized platform.

### 9.2 Job Schemas and Deterministic Hashing

The compute marketplace exposes work as typed **job schemas**. A job schema defines:

* required inputs (including exact types and formats)
* expected outputs (structured fields)
* verification mode (objective or subjective)
* deterministic hashing rules for inputs/outputs
* provenance metadata (model, sampling, token counts, timestamps)

This design solves the “garbage in, garbage out” problem common in ad-hoc agent markets. Providers cannot claim completion without producing outputs that match the schema. For **objective jobs** (e.g., running a command in a sandbox), verification is deterministic and automated. For **subjective jobs** (e.g., summarization, ranking), results require a judge model, consensus, or human review.

Deterministic hashing enables receipts: two parties can independently compute the same hash for a job request and result, making the transaction verifiable and replayable.

### 9.3 Payments, Budgets, and Treasury Policy

OpenAgents enforces budgets at runtime. Each agent operates under a treasury policy that can define:

* global spend limits (per hour/day/session)
* per-task spend caps
* per-lane caps (how much can be spent on cloud or swarm calls)
* escalation thresholds (when higher-cost models are allowed)
* cost targets (maximize success under a cost ceiling)

Budgets are not only guardrails; they are training signals. A policy that solves tasks but burns budget is dominated by one that achieves similar success at lower cost. Accordingly, OpenAgents logs spend and token usage per tick and per decision, and uses cost as an input to both routing policies and self-improvement (§8).

### 9.4 Incentives, Reputation, and Provider Selection

OpenAgents supports decentralized provider networks where providers offer compute in exchange for payment. This requires incentives and selection mechanisms that favor reliable providers and penalize low-quality or malicious behavior.

Provider selection can incorporate:

* advertised capabilities (models, hardware, tools)
* historical success rate on similar job types
* latency distributions
* cost and pricing
* cryptographic reputation signals (signed receipts, verified outputs)
* dispute history (failed objective checks, inconsistencies)

OpenAgents is designed so that provider quality can be learned and improved: the system can treat “provider choice” as another decision pipeline that can be compiled, evaluated, and updated over time.

### 9.5 Market-Aware Routing: Cost as a Control Surface

Economic constraints shape the routing layer (§6) in two ways:

1. **Lane arbitration:** delegation decisions are cost-sensitive. For example, a task may be complex, but if budget is low, the system may prefer local tools + RLM rather than cloud escalation.
2. **Federation planning:** FRLM fanout is bounded by budget. The root model can request parallel subcalls, but the runtime enforces maximum spend, and the optimizer learns which fanout patterns deliver value.

This is analogous to resource-bounded planning: OpenAgents is optimizing not just for correctness, but for *expected verified success per unit cost*.

### 9.6 Receipts, Provenance, and Auditability in Market Transactions

Every market transaction produces an auditable artifact:

* the job request and its hash
* provider identity and signature
* the result payload and its hash
* verification outcome (when objective)
* cost metadata (invoice amounts, token usage when available)
* links to supporting trajectories (optional)

This provenance supports:

* dispute resolution (“did the provider actually do the work?”)
* accountability (“which model produced this?”)
* reproducible evaluation (“can we replay the job?”)
* training signals (“which providers produce higher utility outputs?”)

Critically, provenance makes economic control compatible with self-improvement: the system can learn not only better prompts and routing, but also better market behavior—who to buy from, when to escalate, and how to allocate budget across subproblems.

### 9.7 Summary

OpenAgents treats autonomy as an economic activity. Identity enables accountability, job schemas ensure typed exchange, deterministic hashing yields verifiable receipts, and budgets create a resource-bounded control surface that reduces thrash and aligns optimization with real-world constraints. These market primitives integrate tightly with routing (§6), RLM federation (§7), and self-improvement (§8): the system learns not only how to solve tasks, but how to solve them efficiently under budget, provider variability, and verification requirements. The next section (§10) describes evaluation protocols and benchmarks for measuring success, stability, cost-efficiency, and resistance to Goodhart-style failure modes.

## 10 Evaluation

OpenAgents targets a domain where “correctness” is often measurable (tests/builds) but autonomy introduces new axes that typical LLM benchmarks do not capture: stability across long sessions, cost-aware routing, resistance to degenerate loops, and reproducible provenance. Accordingly, our evaluation is designed to measure **verified task success** and the system properties that determine whether autonomy is usable in practice.

### 10.1 Benchmarks and Task Suites

We evaluate OpenAgents across four complementary settings:

**(A) Real-World Repository Tasks**

* A curated suite of tasks drawn from active open-source repositories (bug fixes, feature additions, refactors, build breaks).
* Each task defines a verification harness (tests/build/lint) and a completion criterion (pass/fail + artifact checks).

**(B) Standard Coding Benchmarks**

* **SWE-bench Verified–style tasks** for multi-file bugfixes with private-style verification (but evaluated via our own harness for reproducibility).
* **Polyglot-style tasks** spanning multiple languages with pass@1 constraints (no ground-truth test reveal during execution).
* These benchmarks provide comparability to existing agent literature, while our runtime adds budgets, trajectories, and routing.

**(C) Long-Context / Long-Horizon Suites**

* Tasks designed to stress context growth: large repos, extensive log histories, deep dependency chains, and multi-iteration debugging.
* Includes “session-length stress” variants where the agent must work through multiple verification cycles and maintain coherence.

**(D) Market-Integrated Tasks**

* A subset of tasks executed with FRLM enabled, where subqueries can fan out to a swarm provider network.
* These experiments measure economic routing and distributed recursion under budget constraints.

For each suite we publish task definitions, verification harnesses, and reproducible environment setup (container images, pinned dependencies), so results can be replayed.

### 10.2 Metrics

We report three categories of metrics: **task success**, **efficiency**, and **stability/safety**.

#### 10.2.1 Task Success

* **Verified Success Rate:** fraction of tasks whose verification harness passes at termination.
* **Time-to-Success / Iterations-to-Success:** iterations needed to reach passing verification.
* **Regression Rate:** frequency of introducing new failing tests/build errors during the run.

#### 10.2.2 Efficiency and Cost

* **Token Cost:** total tokens consumed, broken down by lane (local / cloud / swarm).
* **Tool Cost:** number of tool calls and sandbox executions.
* **Cost-Adjusted Success:** success per 1k tokens, success per dollar, and success per unit time.
* **Escalation Frequency:** how often the system delegates to expensive lanes (Codex/swarm) vs staying local.

#### 10.2.3 Stability and Anti-Thrash

* **Repetition Rate:** fraction of actions that repeat an identical (or near-identical) tool call within a session.
* **Verification Delta Trajectory:** distribution of per-iteration test deltas (how often iterations improve vs worsen).
* **Hallucination Indicators:** incidence of “claimed-but-not-executed” tool calls (where detectable via tool middleware).
* **Termination Quality:** proportion of failures that are graceful (bounded) vs pathological (budget burn, infinite-like loops).

These metrics align with our self-improvement objectives (§8): we optimize for verified success while penalizing cost and degenerate behavior.

### 10.3 Ablation Studies

We evaluate the contribution of each system component via controlled ablations. Key comparisons include:

**DSPy compilation vs. static prompts**

* Replace optimized signatures with fixed handcrafted prompts.
* Measure success rate, cost, and stability to quantify “compiled cognition” gains.

**Outcome-coupled optimization vs. format-only optimization**

* Compare pipelines optimized only for parse/format correctness against pipelines optimized using verification delta, repetition, and cost signals.
* This isolates Goodhart effects and validates that outcome coupling improves real task completion.

**RLM/FRLM vs. non-recursive execution**

* Disable RLM and force all reasoning through standard prompt contexts.
* Evaluate on long-context suites to measure improvements in stability and success under context growth.

**Counterfactual gating vs. always-on learned routing**

* Remove confidence gating and execute DSPy routing decisions unconditionally.
* Quantify regressions and demonstrate why gated rollout + shadow-mode evaluation reduces risk.

**Archive effects (policy bundles)**

* Compare a single continuously-updated policy bundle against retaining an archive of prior bundles (A/B sampling or rollback strategy).
* Measure whether archives reduce regressions and improve recovery from local optima.

### 10.4 Market Experiments: Swarm vs Local vs Cloud

To evaluate OpenAgents as an economic system, we run experiments where identical tasks are executed under different market configurations:

1. **Local-only:** all inference local, no delegation.
2. **Cloud-first:** plan/synthesize on a strong cloud model, tools local.
3. **FRLM hybrid:** root execution local, federated fan-out to cloud and swarm for subqueries.
4. **Swarm-augmented:** parallel subqueries and specialized jobs purchased from distributed providers.

We measure:

* verified success and time-to-success
* total spend (tokens, invoices) and cost-adjusted success
* provider reliability (success rate, latency) and how provider selection evolves
* how often federation is actually used vs. avoided due to budget constraints

These experiments are essential: the same autonomy policy can be “good” in a local setting but “bad” in a market setting if it escalates too aggressively or fails to bound fan-out.

### 10.5 Failure Modes and Case Studies

Quantitative metrics do not fully capture agent behavior under stress. We therefore include qualitative case studies with full trajectories:

* **Context rot vs RLM recovery:** identical tasks where non-RLM runs degrade over long traces and RLM-backed runs remain coherent by externalizing state.
* **Objective hacking attempts:** cases where format correctness improves but verification stagnates, illustrating why outcome-coupled metrics are necessary.
* **Delegation mistakes:** tasks where delegation was unnecessary (wasted spend) or insufficient (should have escalated), highlighting the importance of counterfactual logging and confidence gating.
* **Provider faults:** swarm runs where providers time out or return low-quality results, showing how fallback strategies and receipts prevent silent failures.

Each case study is reported with:

* the task + verification harness
* policy bundle version
* lane routing decisions + confidence
* tool traces and diffs
* verification history over iterations
* cost breakdown

### 10.6 Summary

Our evaluation framework measures OpenAgents along the axes that matter for production autonomy: verified success, cost efficiency, stability under long sessions and large contexts, and resilience to Goodhart-style pathologies. The ablations isolate the value of compiled cognition (DSPy), recursion (RLM/FRLM), and outcome-coupled self-improvement, while market experiments validate that the system remains effective under budgets and heterogeneous providers. The next section (§11) focuses on safety and governance: sandboxing, objective hacking, privacy, and human oversight strategies for self-improving agents.


## 11 Safety and Governance

OpenAgents is designed to make autonomous agents more capable, more scalable, and more economically real. Those same properties raise safety and governance requirements beyond typical “chat assistant” deployments. In OpenAgents, safety is not a single guardrail—it is a layered set of constraints, evaluation strategies, and oversight mechanisms spanning execution, learning, and markets. This section describes the primary risks, the mitigations implemented today, and the governance surfaces required as agents become more autonomous and self-improving.

### 11.1 Sandboxing and Capability Containment

OpenAgents executes potentially destructive actions (editing code, running commands, deploying artifacts). The system therefore assumes **containment by default**.

**Execution isolation.** All task execution occurs in sandboxed environments with:

* scoped filesystems (workspace-only mounts)
* restricted network access (deny-by-default; allow-lists for specific tasks)
* CPU/memory/time limits per run and per tool call
* explicit tool schemas with validation and logging
* no ambient credentials unless explicitly mounted

**Capability boundaries.** The runtime separates:

* “reasoning” capabilities (LM inference)
* “acting” capabilities (tools)
* “spending” capabilities (wallet/budget)
* “publishing” capabilities (network write, git push, deploy)

Each boundary is separately controlled and policy-gated. For example, the agent may be allowed to run tests but not allowed to publish results externally without explicit approval or budget authorization.

**Fail-safe termination.** The runtime enforces hard caps:

* max iterations
* max recursion depth and fanout
* max tool calls per tick
* max spend per session
* circuit breakers for repeated failures

These are non-negotiable stop conditions that prevent runaway behavior.

### 11.2 Objective Hacking and Robust Evaluation Design

Self-improving systems are vulnerable to Goodhart effects: once a metric becomes a target, the agent can find strategies that “win the metric” without delivering true value. In autonomous coding, the most common form is **progress theater**—producing plausible plans, verbose reasoning, or format-correct outputs while failing to improve verification state.

OpenAgents mitigates objective hacking through a combination of design choices:

**Outcome-coupled scoring (§8).** We reward verified progress (test deltas, build success), penalize regressions and repetition, and incorporate cost. Optimizing a single proxy (like JSON validity) cannot dominate.

**Counterfactual baselines (§6).** We record what legacy heuristics would have done and measure DSPy wins/losses. This reduces the chance that the system “learns itself into a hole” and provides grounded negative examples.

**Separation of concerns.** Critical evaluators (verification harnesses, hallucination detectors, policy gates) are treated as trusted runtime components, not as editable prompt content. Where the system is allowed to modify itself, those changes are versioned and must pass evaluation gates before deployment.

**Multi-signal success criteria.** A run is only considered successful when:

* objective verification passes, and
* the system produces a coherent summary and artifact list that matches observed diffs/tests.

This avoids cases where an agent claims success without evidence.

### 11.3 Provider and Marketplace Safety

A decentralized compute market introduces adversarial or faulty providers. Threats include:

* low-quality outputs
* partial completion
* malicious results that look plausible
* invoice manipulation or payment fraud
* spam or denial-of-service via job flooding

OpenAgents addresses these with protocol and runtime controls:

**Authentication and signing.** Providers must authenticate and sign results. This enables accountability and reputation.

**Typed job schemas.** Providers must respond with structured outputs that pass schema validation. This prevents arbitrary payloads from being accepted as results.

**Verification when possible.** Objective jobs (sandbox runs, deterministic checks) are verified locally. Providers cannot claim success without producing a verifiable artifact.

**Reputation and selection.** Provider selection can incorporate past success rates, latency distributions, and dispute history. Providers that repeatedly fail verification are down-ranked or excluded.

**Budget caps and circuit breakers.** Market calls are bounded: the system cannot spend past caps or fan out indefinitely.

### 11.4 Privacy, Redaction, and Data Sharing Policies

OpenAgents generates valuable data (trajectories, diffs, verification logs) that can improve policies. But those artifacts often contain sensitive information: proprietary code, secrets, internal paths, or private credentials.

We treat privacy as a first-class governance decision:

**Local-first storage.** By default, trajectories and datasets remain local.

**Redaction pipeline.** When sharing is enabled, OpenAgents applies a privacy filter that:

* detects and removes secrets (keys, tokens, credentials)
* anonymizes repository paths and identifiers
* strips irrelevant file contents
* optionally summarizes sensitive segments rather than exporting raw text

**Opt-in contribution.** Users and organizations explicitly choose whether to contribute data. Enterprises can run fully in private mode and still benefit from the infrastructure.

**Differential trust tiers.** Not all artifacts are equal: objective verification deltas and aggregate metrics can often be shared safely even when raw diffs cannot. OpenAgents supports sharing higher-level telemetry without sharing source code.

### 11.5 Human Oversight and Intervention Points

Autonomy does not imply zero human involvement. OpenAgents is designed so humans can intervene at well-defined control points:

* **Budget setting:** operators decide how much the agent can spend and on what lanes.
* **Delegation approval:** certain escalations (cloud/swarm) can require confirmation.
* **Publish gates:** committing, pushing, deploying, or paying invoices above thresholds can require approval.
* **Stop/override:** the UI/CLI can pause, interrupt, or switch to manual mode.

Critically, these are *system-level* controls rather than “interrupt the prompt.” They remain effective even when the model behaves unexpectedly.

### 11.6 Governance for Self-Improvement

Self-improvement creates a new governance problem: how do we ensure optimized policies remain aligned with intended behavior?

OpenAgents uses three mechanisms:

**Versioned policy bundles.** Every compiled policy is a deployable artifact with metadata, datasets, and metrics. This makes upgrades reviewable and reversible.

**Shadow-mode rollout.** New policies can run in parallel without controlling behavior until they demonstrate consistent wins.

**Rollback and quarantine.** If a policy causes regressions (accuracy drop, cost spike, higher thrash), the system can automatically revert to a known-good bundle and quarantine the new one for offline analysis.

Future governance directions include:

* explicit “unmodifiable” safety modules (hard-coded evaluators and gates)
* constitutional constraints embedded into tool-use policies
* multi-party approval for high-risk actions (threshold signing applied to spending or publishing)

### 11.7 Summary

OpenAgents treats safety as a layered systems property: sandboxed execution and capability containment prevent catastrophic side effects; outcome-coupled evaluation and counterfactual logging reduce Goodhart and regressions; typed schemas and verification constrain marketplace risk; privacy defaults and redaction govern data sharing; and policy versioning plus shadow rollouts provide practical governance for self-improving behavior. The next section (§12) discusses broader implications, limitations, and open problems in building compiled, market-aware, self-improving agent systems.

## 12 Discussion

OpenAgents is an attempt to make autonomous agents behave less like brittle prompt scripts and more like **software systems that can improve over time**. This section discusses what the synthesis implies, where the approach is still limited, and what the most important open problems are if we want to move from “useful autonomy” to “reliably self-improving agents operating in open markets.”

### 12.1 Why Compiled Agents + Markets Changes the Scaling Story

Most current narratives about scaling agent capability focus on bigger models, larger context windows, and more tool integrations. OpenAgents suggests a different scaling axis: **compile behavior, measure it, and optimize it under constraints**.

**Compilation makes behavior portable and improvable.** Typed signatures and modules create stable interfaces that survive model swaps and infrastructure changes. Instead of rediscovering prompt hacks every time a provider updates a model, you can recompile a policy bundle against your metrics and training data. That is a qualitative shift: agent behavior becomes a *managed artifact* rather than an emergent property of a prompt.

**Markets impose a natural regularizer.** When an agent can buy compute, it becomes tempting to escalate constantly. But budgets convert this into a planning problem: “What is the minimal spend required to reach verified success?” This turns cost into a stabilizing pressure that discourages thrash, encourages early cheap probes (local grep, small model passes), and learns escalation policies that are economically rational. In practice, this is how you get autonomy that scales to fleets: humans allocate budgets and attention; agents allocate compute and steps.

**Verification closes the loop.** In software engineering, we are unusually lucky: many tasks have objective checks. Tests and builds are coarse, but they are powerful. They provide a stable reward signal that is far harder to game than “did the output look correct.” This makes autonomous coding a strong proving ground for self-improvement architectures, because it supports empirical selection of better policies.

**RLM changes the context limit from a hard ceiling to an optimization problem.** Externalized context + recursion reframes “how much can the model read?” into “what slices should be retrieved, transformed, and delegated?” This matters more than raw context size for long-running autonomy: performance collapse often comes from clutter and drift rather than a single missing token. RLM turns context management into policy, which means it becomes optimizable.

### 12.2 Limits and Practical Constraints

Despite these advantages, OpenAgents remains constrained by current model capabilities and the realities of production environments.

**Models still hallucinate and miscalibrate.** Tool hallucination, fake test claims, and overly confident reasoning remain common failure modes. OpenAgents mitigates these with tool mediation, objective verification, and confidence gating, but cannot eliminate them. As long as the base models have reliability gaps, autonomy must be bounded and evaluated continuously.

**Outcome signals are incomplete.** Tests and builds capture many bugs, but not all correctness and not all product requirements. Many software tasks are inherently subjective (“is this code clean?”, “is this UX right?”). For those tasks, OpenAgents must either (i) define proxy metrics and accept Goodhart risk, (ii) incorporate human feedback, or (iii) use judge models and consensus. None of these are fully solved.

**Optimization can overfit.** DSPy-style compilation improves performance on the examples and metrics provided, but can overfit to narrow distributions. Counterfactual baselines, rolling windows, and shadow-mode evaluation help, yet this remains an open operational challenge: “How do we ensure policy bundles generalize across repos and time?”

**RLM/FRLM can be expensive if uncontrolled.** Recursion is powerful but can explode in cost if fan-out is naive. We rely on budget caps and stopping criteria, but the optimal policy for when to recurse, how to partition, and how much to fan out is itself a hard learning problem—especially under partial observability.

**Marketplace integrity is non-trivial.** Decentralized compute introduces adversarial and noisy providers. Typed schemas and objective verification help, but subjective tasks still require trust mechanisms, judges, or economic incentives that can be exploited. Reputation systems are fragile; verification is expensive; and provider heterogeneity complicates performance predictability.

### 12.3 Open Problems and Research Directions

We highlight several concrete research directions suggested by OpenAgents’ architecture.

#### 12.3.1 Better “Interestingness” and Utility Models for Agents

OMNI’s key lesson is that “learnable” is not the same as “worth doing.” For coding agents, the analog is that “plausible steps” are not the same as “useful steps.” We operationalize utility via verification delta, repetition, and cost, but richer notions remain:

* learning to predict which edits are likely to reduce failing tests before running them
* novelty measures that reward discovering new solution approaches without incentivizing thrash
* explicit “value of information” modeling for context retrieval and recursion

A particularly important direction is learning a **step utility model** that becomes the agent’s internal “interestingness judge” for what to do next.

#### 12.3.2 Archives and Non-Greedy Policy Evolution

DGM shows that archives and stepping stones matter: greedy hill-climbing can get stuck. OpenAgents currently keeps an archive of trajectories and policy bundles, but does not yet run a true evolutionary search over policy variants. Future work could:

* maintain multiple active policy bundles per lane
* sample bundles using exploration/exploitation strategies
* evaluate bundles on held-out tasks continuously
* allow controlled branching and recombination of improvements

This would bring OpenAgents closer to an open-ended search process while retaining governance gates.

#### 12.3.3 Compiling for Federation: Distillation Across Lanes

A major opportunity is “teacher–student compilation” at system scale:

* strong cloud models (teacher) solve tasks and generate trajectories
* cheaper local/swarm models (student) are compiled using those trajectories
* skills and policy bundles are distributed and priced in the marketplace

This suggests a new kind of network effect: the market does not just sell compute, it accumulates **distilled competence** that makes cheaper compute increasingly valuable.

#### 12.3.4 RLM as a Policy Target

Today, RLM is a substrate. But the recursion policy itself—when to recurse, how to partition, what to summarize, when to stop—should be learned. This pushes toward:

* formal interfaces for context operations
* metrics for “information gain per token”
* training signals from successful long-horizon runs
* model-based or RL-style optimization over recursion trajectories

If solved, this would reduce cost blowups while preserving long-context robustness.

#### 12.3.5 Governance for Self-Improving Systems

Self-improvement makes governance continuous: “what did the agent change about itself?” must remain inspectable and reversible. OpenAgents’ policy bundles and shadow rollout are practical first steps, but larger questions remain:

* how to encode non-negotiable constraints (unmodifiable safety modules)
* how to define evaluation that resists objective hacking
* how to incorporate human feedback at scale without making humans the bottleneck
* how to handle model/provider drift in deployed self-improving fleets

### 12.4 Implications

OpenAgents points to a synthesis: **autonomy becomes tractable when behavior is compiled, execution is verifiable, reasoning is scalable via recursion, and actions are bounded by budgets**. This reframes the path to “better agents” away from one-off prompt artistry and toward an engineering discipline closer to traditional systems design—where contracts, measurement, iteration, and economics shape capability.

At the same time, the approach highlights the central tension of open-ended progress: any metric can be gamed, any market can be exploited, and any self-improvement loop can drift. The core bet is that these risks are best handled not by hoping the model is aligned, but by building a system where **verification, provenance, and resource constraints** continuously steer behavior.

### 12.5 Summary

OpenAgents changes the autonomy problem from “get a model to do tasks” to “build a system that can reliably improve at doing tasks under constraints.” Compiled cognition enables systematic optimization; verification provides ground truth; recursion enables long-horizon reasoning; and markets enforce cost-aware behavior. The remaining challenges—generalization, objective hacking, robust federation, and governance—define the next research frontier for production-grade self-improving agents.


## 13 Conclusion

We introduced **OpenAgents**, a systems-and-learning framework for building autonomous agents that behave like real software actors: they execute work in sandboxed environments, verify outcomes with deterministic checks, operate under explicit budgets, and improve over time from logged trajectories. OpenAgents unifies four core ideas into a single production-oriented stack: **compiled cognition** (DSPy-style signatures and modules optimized via compilation), **verifiable execution** (tools, sandboxes, and test harnesses as ground truth), **scalable long-horizon reasoning** (RLM/FRLM via externalized context and recursive subqueries), and **economic grounding** (identity, payments, and marketplace-based compute acquisition).

The central contribution is not any single component, but the **closed loop** created by their integration. By replacing brittle prompt strings with typed signatures, OpenAgents makes agent cognition measurable and portable. By coupling those signatures to outcome-derived labels—verification deltas, repetition signals, and cost—OpenAgents turns “agent improvement” into an empirical engineering process rather than a manual art. By externalizing context through RLM, the system remains stable as sessions lengthen and repositories grow. By embedding execution in an economy with budgets and receipts, OpenAgents aligns autonomy with real-world constraints and enables decentralized scaling through paid providers.

While our current implementation focuses on self-improvement at the level of compiled policies and routing decisions, the architecture is compatible with more open-ended variants: archived policy bundles, non-greedy exploration of stepping stones, and teacher–student distillation across lanes and providers. At the same time, we emphasize the safety implications of self-improving systems and motivate conservative governance mechanisms—sandboxing, confidence gating, counterfactual logging, shadow rollouts, and reproducible policy versioning—to mitigate objective hacking and regressions.

OpenAgents is ultimately a proposal for a practical path to continuously improving agents: not by assuming perfect models, but by building a stack where **measurement, verification, provenance, and resource constraints** shape behavior. We believe this framing—compiled, verifiable, recursive, and economic—offers a robust foundation for agents that can operate autonomously in production domains and evolve their capabilities over time.
