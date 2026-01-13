Abstract

> **Implementation Status:** This paper describes the OpenAgents system architecture. Some components are implemented and production-ready; others are specified but not yet wired. For current implementation status, see [SYNTHESIS_EXECUTION.md](./SYNTHESIS_EXECUTION.md). For canonical terminology, see [GLOSSARY.md](./GLOSSARY.md). For protocol details, see [docs/PROTOCOL_SURFACE.md](./docs/PROTOCOL_SURFACE.md).

We present **OpenAgents**, a systems and learning framework for building **self-improving, economically grounded AI agents** that operate over real software artifacts and open compute markets. OpenAgents unifies three previously separate threads: (i) **declarative agent programming and compilation** (DSPy-style signatures, modules, and optimizers), (ii) **open-ended curriculum and task selection** guided by models of interestingness (OMNI/OMNI-EPIC), and (iii) **empirically validated self-modification loops** for agent improvement (Darwin Gödel Machine). Concretely, we replace brittle prompt strings with typed **Signatures** spanning planning, tool execution, synthesis, and routing, and close the loop by logging trajectories, labeling decisions using downstream outcomes (e.g., test deltas, cost, repetition), and triggering automatic recompilation via prompt optimization. To scale beyond context limits and mitigate long-horizon “context rot,” we integrate **Recursive Language Models (RLM/FRLM)** as an execution substrate that externalizes long context into a programmable environment and enables recursive sub-queries across large codebases and document sets. OpenAgents additionally introduces a market-aware runtime in which agents possess cryptographic identity, hold budgets, purchase compute from a decentralized provider network, and emit verifiable receipts for work. We argue that this combination—**compiled behavior + outcome-coupled optimization + scalable recursion + economic constraints**—yields a practical path toward open-ended, continuously improving agents in production domains, and we outline evaluation protocols that measure not only task success but also stability, cost-efficiency, and resistance to Goodhart-style pathologies.

## Table of Contents

1. **Introduction**
   1.1 Motivation: Agents as Real Software Actors
   1.2 From Prompting to Compiled Agent Behavior
   1.3 Contributions

2. **Background and Related Work**
   2.1 Declarative LM Programming and Compilation
   2.2 Auto-Curricula and Interestingness in Open-Ended Learning
   2.3 Self-Improving Agents via Empirical Validation
   2.4 Long-Horizon Inference and Externalized Context
   2.5 Markets, Identity, and Verification for Agent Economies
   2.6 Crypto Identity and Threshold Signing
   2.7 Decentralized Job Markets and DVMs
   2.8 Pay-per-Call and Lightning-Native Payments
   2.9 Agent Wallets, Receipts, and Verifiable Work

3. **System Overview**
   3.1 OpenAgents Stack and Design Principles
   3.1.1 Agentic OS primitives
   3.1.2 Protocol substrate
   3.1.3 Identity and payments unification
   3.2 Threat Model and Assumptions
   3.3 Execution Substrates: Local, Cloud, Swarm
   3.4 Agent Execution Flow
   3.5 Data Products: Trajectories, Datasets, and Policy Bundles
   3.6 Summary

4. **Programming Model**
   4.1 Signatures: Typed Contracts for Agent Cognition
   4.2 Modules: Composable Policies (Plan → Act → Verify → Synthesize)
   4.3 Metrics: From Format Correctness to Outcome-Coupled Utility
   4.4 Optimizers: MIPROv2 and Beyond
   4.5 Policy Bundles, Versioning, and Reproducibility
   4.6 Design Implications

5. **Agent Runtime**
   5.1 Tick Model and State Surfaces
   5.2 Tooling and Sandboxed Execution
   5.3 Verification Harnesses and Objective Checks
   5.4 Trajectory Logging and Provenance
   5.5 Runtime–Compiler Interface
   5.6 Summary

6. **Decision Pipelines and Routing**
   6.1 Complexity Classification
   6.2 Delegation Decisions
   6.3 RLM Triggering
   6.4 Provider Lane Selection and Fallback Strategies
   6.5 Economic Routing
   6.6 Counterfactual Recording and Shadow Mode
   6.7 Outcome-Coupled Labeling of Decisions
   6.8 Summary

7. **Recursive Language Models in OpenAgents**
   7.1 RLM as an Execution Substrate
   7.2 Context Externalization and REPL Environments
   7.3 FRLM: Federated Recursion Across Providers
   7.4 Cost/Latency Tradeoffs and Stopping Criteria
   7.5 RLM–DSPy Integration Patterns
   7.6 Failure Modes and Mitigations
   7.7 Summary

8. **Self-Improvement Loop**
   8.1 SessionStore and Outcome Labeling
   8.2 Outcome Feedback: Turning Runs into Labeled Examples
   8.3 Performance Tracking: Rolling Accuracy and Drift
   8.4 Auto-Optimization: When to Recompile and What to Target
   8.5 Avoiding Goodhart Pathologies
   8.6 Archives and Stepping Stones: Toward Open-Ended Improvement
   8.8 Canary + Progressive Rollout
   8.9 APM and Fleet KPIs
   8.10 Summary

9. **Marketplace and Economic Constraints**
   9.0 Motivation: Economics as Control Surface
   9.1 Neobank: Treasury OS for Agent Fleets
   9.2 Compute Marketplace: Verified Jobs and Demand Floor
   9.3 Exchange: Liquidity and FX Routing for Agents
   9.4 End-to-End Payment-Linked Autonomy
   9.5 Summary

10. **Evaluation**
    10.1 Evaluation Questions
    10.2 Task Suites
    10.3 Experimental Conditions (Ablations)
    10.4 Metrics (Explicit Definitions)
    10.5 Tables and Figures to Add
    10.6 Methodology Details
    10.7 Case Studies
    10.8 Summary

11. **Safety and Governance**
    11.1 Sandboxing and Capability Containment
    11.2 Objective Hacking and Robust Evaluation Design
    11.3 Provider and Marketplace Safety
    11.4 Privacy, Redaction, and Data Sharing Policies
    11.5 Human Oversight and Intervention Points
    11.6 Governance for Self-Improvement
    11.7 Summary

12. **Discussion**
    12.1 Why Compiled Agents + Markets Changes the Scaling Story
    12.2 Limits and Practical Constraints
    12.3 Open Problems and Research Directions
    12.4 Implications
    12.5 Summary

13. **Conclusion**

**References**

**Appendices**
A. Implementation Details (Interfaces, Storage Layouts, Config)
B. Signature Catalog and Prompts
C. Metric Definitions and Scoring Functions
D. Optimization Recipes and Hyperparameters
E. Additional Plots and Tables
F. Reproducibility Checklist
G. Protocol Surface (High-Level)

## 1 Introduction

Autonomous AI agents are beginning to perform real work in real software environments: reading and editing repositories, running tests, filing and resolving issues, and coordinating across tools. Yet most deployed “agent” systems remain brittle prompt-and-scaffold assemblies. Their behavior is encoded in hand-written prompts and ad hoc heuristics that are difficult to test, hard to port across model providers, and costly to improve. As a result, these systems exhibit familiar failure modes under production conditions: they thrash across files, repeat actions, hallucinate tool outputs, regress when prompts are tweaked, and degrade as sessions and context grow.

OpenAgents is motivated by a different thesis: reliable autonomy requires an **operating system for agents**, not another chat wrapper. An operating system provides resource abstractions, isolation, scheduling, and standardized interfaces for programs to cooperate safely. OpenAgents provides these same functions for AI agents: agents have cryptographic identity; they run in a controlled execution environment with tools and sandboxes; they operate under explicit budgets and approval policies; they can purchase compute and skills in open markets; and they produce auditable trajectories and cryptographic receipts that link actions to outcomes and spending. The goal is to make agents first-class software actors—capable of operating continuously and economically, without a human operator in the loop at every step.

Technically, OpenAgents synthesizes four ideas into a single production-oriented stack.

First, OpenAgents adopts **compiled cognition**. We replace monolithic prompt strings with typed **Signatures**—explicit input/output contracts for each cognitive step (planning, tool execution, synthesis, and routing decisions). Signatures are composed into **Modules** and optimized via compilation (e.g., MIPRO-style prompt optimization), turning “prompt engineering” into a measurable software process. This design makes agent behavior portable across heterogeneous model backends and enables incremental improvements without rewriting orchestration code.

Second, OpenAgents grounds autonomy in **verifiable execution**. For software engineering tasks, tests and builds provide unusually strong objective signals. OpenAgents treats verification harnesses as the primary “reward” for the agent loop: a task is not “done” because the model says so, but because deterministic checks pass. Tool calls, diffs, and verification outputs are logged as structured trajectories, providing both auditability and training signal.

Third, OpenAgents integrates **scalable long-horizon reasoning** via **Recursive Language Models (RLM/FRLM)**. Large repositories and long-running sessions create effective contexts far beyond typical prompt limits, and performance can degrade as history accumulates (“context rot”). RLM externalizes large state into a programmable environment and allows recursive sub-queries over targeted slices rather than stuffing everything into a single prompt. FRLM generalizes this by federating recursion across local inference, cloud models, and decentralized providers, bounded by explicit budgets.

Fourth, OpenAgents treats economics as a **control plane**. Agents do not merely call APIs; they spend funds and must remain within budgets. OpenAgents introduces a treasury layer (“Neobank”) that routes payments across multiple rails and assets (e.g., Lightning, eCash mints, future Taproot Assets), enforces multi-level budgets (org/repo/issue), supports idempotent payment state machines with reconciliation (because agents crash and networks stall), and produces receipts that bind payments to trajectories and policy decisions. Above the treasury layer, an exchange protocol enables agent-to-agent liquidity and FX routing so agents can pay providers in the required asset/rail while operators budget in stable units. This economic substrate makes autonomy practical at scale and provides additional optimization signals: the system is trained not only to succeed, but to succeed efficiently.

This paper formalizes OpenAgents as an “agentic OS” and positions it relative to recent work on declarative LM programming, open-ended learning, self-improving coding agents, and recursive inference. We argue that combining compiled cognition with verifiable execution and explicit economic constraints yields a pragmatic path toward continuously improving agents that can operate safely and efficiently in production environments. We also outline an evaluation framework that measures not just task success, but stability (thrash and regressions), cost (tokens and sats), long-session robustness, and resistance to Goodhart-style pathologies.

### 1.1 Contributions

OpenAgents makes the following contributions:

1. **An agentic OS architecture** that unifies identity, budgets, tool execution, verification, and auditability into a coherent runtime for autonomous agents.
2. **A compiled programming model for agent behavior** using typed Signatures and composable Modules optimized via prompt compilation, enabling reproducible behavior across heterogeneous model providers.
3. **Outcome-coupled self-improvement**: session-level and step-level labeling based on verification deltas, repetition, and cost; counterfactual logging (policy vs legacy) for safe rollout; and automated recompilation when performance drifts.
4. **RLM/FRLM integration** as an execution substrate for large contexts and long-horizon autonomy, including budget-bounded federated recursion across local, cloud, and decentralized compute.
5. **A market- and treasury-aware design** (Neobank + Exchange) that enables pay-after-verify compute markets, multi-rail payments, and receipts linking spending to verified work and trajectories.

Together, these components define an end-to-end system where agents can operate as autonomous economic actors: executing tasks, purchasing resources, producing verifiable work, and improving their own policies over time.

## 2 Background and Related Work

OpenAgents sits at the intersection of (i) **declarative programming and compilation for foundation models**, (ii) **open-ended learning and auto-curricula**, (iii) **self-improving coding agents**, (iv) **long-horizon inference with externalized memory**, and (v) **market/identity primitives that make agents economically real**. This section reviews these threads and clarifies how OpenAgents composes them into a unified, production-oriented system.

### 2.1 Declarative LM Programming and Compilation

Early “prompt engineering” treated LMs as black boxes controlled through hand-written instructions and few-shot examples. As pipelines grew (retrieval → reasoning → tool use → verification), hand-tuned prompts became brittle: small changes to context or model provider often required extensive re-tuning, and prompt logic was duplicated across systems. Recent work reframes LM applications as **programs**: structured compositions of model calls, tools, and intermediate variables, analogous to classical software pipelines (TODO:CITE).

DSPy popularized a particularly useful abstraction: **Signatures** (typed I/O contracts expressed in natural language field names) and **Modules** (composable units whose internal prompting strategy is optimizable), paired with **optimizers** that compile programs to prompts/demonstrations for a target metric (TODO:CITE). This compilation view matters for OpenAgents because it turns “agent behavior” into an artifact that can be versioned, evaluated, and improved without rewriting orchestration code. In OpenAgents, signatures provide stable contracts across heterogeneous execution lanes (local inference, cloud APIs, swarm providers), and optimization (e.g., MIPRO-style prompt search) provides the mechanism for systematic improvement.

OpenAgents extends this line in two ways. First, it treats routing decisions (complexity, delegation, recursion triggers) as **first-class compiled components**, not as ad-hoc heuristics. Second, it emphasizes **outcome-coupled optimization**: rather than optimizing only for format correctness or synthetic evals, it ties optimization to real execution outcomes such as test deltas, cost, repetition, and verified completion in real repositories.

### 2.2 Auto-Curricula and Interestingness in Open-Ended Learning

Open-ended learning aims to generate a continual stream of novel, learnable behaviors without a fixed terminal objective. A central challenge is **task selection** in vast spaces: uniform sampling wastes effort on impossible tasks, while learning-progress-driven curricula can become distracted by endless variants of trivial tasks. This mirrors Goodhart-style failure modes: when a metric becomes a target, it can be optimized in ways that miss the intended goal.

The OMNI family addresses this by introducing **Models of Interestingness**: using foundation models as a proxy for human judgments about what is novel, worthwhile, and meaningfully distinct (TODO:CITE). OMNI demonstrates that adding “interestingness” filtering to learning progress improves open-ended training in both finite and effectively infinite task spaces. OMNI-EPIC extends the paradigm further by generating **environments programmed in code**, expanding the space of possible tasks by synthesizing executable environments and reward checks (TODO:CITE).

OpenAgents draws a direct analogy: the “task space” for an autonomous coding agent is enormous, and naive progress signals (e.g., “we did something” or “we produced valid JSON”) can be hijacked by low-value behaviors such as repeated file openings, unproductive edits, or verbose-but-incorrect synthesis. OpenAgents operationalizes interestingness as **utility under verification**, with metrics grounded in software development reality: test improvements, build success, reduced repetition, and controlled cost. In this view, OMNI’s core insight becomes a systems requirement: successful autonomy requires a notion of “worth doing next,” not merely “learnable” or “syntactically valid.”

### 2.3 Self-Improving Agents via Empirical Validation

Self-improvement has long been studied as a theoretical concept (e.g., Gödel machines), but practical systems require replacing formal proofs of improvement with **empirical evaluation**. Recent work on self-improving coding agents explores loops where an agent modifies parts of its own scaffolding, tools, and workflows, then validates improvements on coding benchmarks (TODO:CITE). The Darwin Gödel Machine (DGM) formalizes a particularly relevant pattern: (i) self-modify, (ii) evaluate on downstream tasks, (iii) retain variants in an archive, and (iv) explore multiple evolutionary branches so that stepping stones can yield later breakthroughs (TODO:CITE).

OpenAgents adopts the same empirical posture but targets a more product-oriented surface area first: instead of evolving the entire agent codebase immediately, it evolves the **compiled cognition layer** (signatures, instructions, routing policies) and the **execution policies** that govern tool use, delegation, and recursion. This choice reduces risk and improves reproducibility: optimization changes are expressed as explicit, versionable prompt/program artifacts rather than arbitrary code mutations. Nonetheless, the DGM lesson remains central: greedy “always update the latest” loops are fragile; archives, counterfactual logging, and stepping-stone exploration help escape local optima and prevent regressions.

OpenAgents also incorporates DGM’s warning about **objective hacking**. Any measurable proxy (format checks, superficial heuristics, even pass/fail on a narrow evaluation slice) can be gamed. OpenAgents therefore emphasizes multi-signal evaluation (verification delta + cost + repetition + outcome), shadow-mode counterfactuals (DSPy vs legacy), and traceability of changes to mitigate Goodhart effects in self-improvement loops.

### 2.4 Long-Horizon Inference and Externalized Context

A persistent obstacle for agent autonomy is long-horizon reasoning over large contexts: repositories with thousands of files, long tool traces, and multi-hour sessions. Simply expanding context windows often yields diminishing returns: models exhibit “context rot,” attention diffusion, and rising cost. Tool-using agent methods (e.g., ReAct-style loops) help by enabling external actions, but they do not by themselves solve the problem of representing and manipulating extremely large state.

Recursive Language Models (RLMs) propose an inference-time strategy for effectively unbounded context: rather than ingesting the entire state into a single prompt, a “root” model interacts with a programmable environment (e.g., a REPL) that holds large context and supports operations like search, partitioning, summarization, and recursive sub-queries (TODO:CITE). This makes long-context reasoning a **procedural interaction** rather than a single monolithic forward pass.

OpenAgents integrates RLM/FRLM as an execution substrate: when tasks exceed context or require deep global reasoning, the agent switches into a recursive mode that externalizes state, performs targeted retrieval and transformations, and optionally federates sub-queries across local models, cloud APIs, or swarm providers. This complements DSPy: DSPy compiles *what the agent should do*; RLM provides the substrate that makes those policies robust under large-scale state and long-run execution.

### 2.5 Markets, Identity, and Verification for Agent Economies

Most agent systems implicitly assume a trusted operator: a human provides API keys, pays for compute, and absorbs the risk of failures. This assumption becomes a bottleneck for scaling fleets of agents and for enabling autonomous cooperation across organizational boundaries. A parallel line of work argues that agents require **identity, budgets, and verification** to be first-class participants in open ecosystems (TODO:CITE).

OpenAgents takes this seriously as a core design constraint. Agents authenticate and sign actions, maintain budgets, and purchase compute across heterogeneous providers. Work products are coupled to **verifiable receipts**: deterministic verification (tests/builds/sandbox runs) and traceable trajectories enable external observers to audit not only outputs but the process that produced them. This is the systems analog of the “empirical validation” principle: improvements and transactions should be grounded in objective checks whenever possible, and subjective tasks should be explicitly recognized as requiring judges, consensus, or human feedback.

This market framing also shapes how OpenAgents relates to DSPy and self-improvement: compilation and optimization are not just about quality, but about **cost-aware performance under constraints**. A signature that is accurate but expensive may be dominated by a slightly less accurate but far cheaper policy when operating under budget limits. In OpenAgents, evaluation therefore includes resource-aware metrics (tokens, tool calls, latency) alongside correctness and verification, aligning “agent intelligence” with economic reality.

### 2.6 Crypto Identity and Threshold Signing

A growing body of work explores cryptographic identity for autonomous agents, including threshold signatures that prevent any single operator from extracting agent keys. FROST provides a practical threshold Schnorr scheme, while FROSTR adapts these signatures to Nostr event signing (TODO:CITE). Bifrost-style coordination layers handle participant discovery, share aggregation, and timeout logic so threshold operations remain usable in distributed settings. OpenAgents adopts these approaches to make agent identity sovereign and enforceable: an agent can authenticate, sign actions, and spend only within policy, without revealing a single extractable private key.

### 2.7 Decentralized Job Markets and DVMs

Decentralized job markets based on Nostr Data Vending Machines (DVMs) define how buyers and providers publish, bid on, and fulfill tasks in open networks. NIP-90 specifies job request and result events, allowing marketplaces to route compute across heterogeneous providers without centralized coordination (TODO:CITE). OpenAgents builds on this line by adding typed job schemas, objective verification where possible, and receipts that link jobs to payments and trajectories.

### 2.8 Pay-per-Call and Lightning-Native Payments

Payment-gated APIs have emerged as a pragmatic alternative to centralized accounts. L402 combines HTTP authentication with Lightning payments, while LNURL and NIP-57 zaps provide lightweight payment primitives that can be embedded into protocol flows (TODO:CITE). These mechanisms motivate OpenAgents’ treasury layer: agents should be able to pay per call, per job, or per verification artifact without manual operator intervention.

### 2.9 Agent Wallets, Receipts, and Verifiable Work

Prior systems for verifiable computation and reproducible builds show that **receipted work** can be audited after the fact (TODO:CITE). In agent settings, this translates to receipts that bind a payment to a job hash, a policy bundle, and a trajectory snapshot. OpenAgents extends this notion by treating receipts as both governance artifacts (auditability) and training signals (what spend produced verified progress).

---

**Positioning OpenAgents.** Taken together, prior work provides the ingredients for self-improving agents: compiled LM programs (DSPy), interestingness-aware task selection (OMNI), empirically validated self-modification with archives (DGM), scalable long-horizon reasoning (RLM), and protocol-native markets (NIP-90, L402, LNURL). OpenAgents contributes a synthesis that is explicitly production-aligned: it binds these ideas to the realities of software development (tests/builds as rewards), heterogeneous inference markets (local/cloud/swarm), and the economic + cryptographic primitives needed for agents to operate as autonomous actors.

## 3 System Overview

OpenAgents is a full-stack system for autonomous agents that (i) execute real work in real environments, (ii) learn from outcomes, and (iii) operate under economic and verification constraints. The central design goal is **continuous improvement without brittle hand-tuning**, while remaining **auditable, reproducible, and cost-aware** across heterogeneous execution backends.

### 3.1 OpenAgents Stack and Design Principles

OpenAgents is organized as a layered stack. At the top are user-facing products (Autopilot, Onyx, GitAfter). Beneath them is an execution layer (Adjutant + Autopilot loop) built on a programming and compilation substrate (dsrs/DSPy; implemented in `crates/dsrs/`). Below that sits infrastructure for execution, routing, identity, payments, and marketplace coordination (Pylon, Nexus, Gateway, Protocol, Runtime).

> **Implementation note:** The DSPy compiler layer (`crates/dsrs/`) and Adjutant execution engine (`crates/adjutant/`) are implemented. Treasury (Neobank), Exchange, and full NIP-SA lifecycle are specified but not yet production-wired. See [SYNTHESIS_EXECUTION.md](./SYNTHESIS_EXECUTION.md) for detailed status.

**Figure 1: OpenAgents stack.** Protocol substrate → treasury/exchange → execution/runtime → products.

#### 3.1.1 Agentic OS primitives

OpenAgents treats autonomy as an OS problem: identity, spending, verification, and transparency are system primitives rather than prompt artifacts. Table 1 summarizes the primitives and their concrete components.

**Table 1: Agentic OS primitives**

| Primitive | Role | OpenAgents components |
| --- | --- | --- |
| Identity | Auth, signing, policy enforcement | FROST/FROSTR threshold keys, NIP-06 derivation, agent profiles |
| Transport | Event delivery and coordination | Nostr relays, NIP-42 auth, NIP-44 encryption |
| Payments | Settlement rails | Spark/LN, eCash mints, on-chain, Taproot Assets |
| Treasury | Budgets and routing | Neobank, TreasuryRouter, quote state machines |
| Marketplace | Compute and skills | NIP-90 job schemas, Pylon provider/host |
| Verification | Objective ground truth | Sandboxed tools, test/build harnesses |
| Transparency | Auditability | Trajectories, receipts, NIP-SA lifecycle logs |

#### 3.1.2 Protocol substrate

OpenAgents builds on Nostr as the transport layer. NIP-42 provides authenticated relay access, NIP-44 provides end-to-end encryption for agent state, and NIP-90 defines job request/result flows for compute markets. OpenAgents proposes NIP-SA as a lifecycle protocol for autonomous agents (profile, schedule, ticks, trajectories) and uses NIP-57 payment events to bind Lightning payments to protocol events. These protocols provide a minimal, interoperable surface while keeping the execution logic local to the agent runtime.

> **Status:** NIP-90 job events are partially implemented (`crates/protocol/`). NIP-SA is proposed/specified but not yet implemented. See [docs/PROTOCOL_SURFACE.md](./docs/PROTOCOL_SURFACE.md) for canonical protocol details and kind numbers.

#### 3.1.3 Identity and payments unification

OpenAgents unifies cryptographic identity and payments through a single root seed. A BIP39 mnemonic derives a Nostr identity (via NIP-06 paths) and a wallet identity (via BIP44 paths), so an agent can authenticate and spend without managing disjoint keys. For sovereign agents, these keys are protected by FROST threshold signing; FROSTR adapts threshold signatures to Nostr events, while Bifrost coordinates the distributed signing sessions. This design prevents operator key extraction while still enabling policy-gated co-signing.

Autopilot is the wedge: it is the first buyer of compute and the first consumer of treasury routing. The market layers (compute + exchange) turn that wedge into a platform by allowing independent providers and treasury agents to participate in routing, settlement, and verification.

The core system insight is that *agent capability is not only model intelligence*—it is the interaction of:

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

**Figure 2: End-to-end execution flow.** Task → plan → tool execution → verification → routing + payment/receipt → iteration/termination.

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

This discrete-time model has two pragmatic benefits. First, it provides a natural unit of logging and evaluation (per-tick costs, progress, and regressions). Second, it enables deterministic replay: given the same repo snapshot, tool outputs, and policy bundle, the system can reproduce decision making for debugging or audit. Tick lifecycle events map cleanly onto NIP-SA style lifecycle logging (wake, tick request/result, trajectory session), making execution observable outside the local runtime.

The runtime exposes the agent’s world as **state surfaces**—structured, inspectable objects that include:

* repository snapshot + diffs relative to a base commit
* plan state (subtasks pending/completed)
* verification state (test failures, build status, lint output)
* identity state (agent profile, threshold config, signing policy)
* wallet and treasury state (balances, budgets, pending quotes)
* approval state (pending approvals, autonomy level, guardians)
* cost accounting (tokens, tool calls, provider spend)
* provider health and lane availability
* execution history (tool calls and results)
* receipt ledger (payments linked to jobs and trajectories)
* session metadata (iterations, timestamps, termination reason)

These surfaces are exposed through runtime queries and tool endpoints, acting as OS-style mounts: the agent can query only what it needs, and the runtime can enforce visibility, access control, and redaction per autonomy policy.

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

#### 5.2.3 Approval workflows and autonomy levels

OpenAgents treats approvals as first-class runtime gates rather than ad hoc prompt instructions. Each session runs under an autonomy level (supervised, semi-autonomous, autonomous), which controls whether actions such as network writes, payments, delegation to untrusted providers, or publish steps require human or guardian approval. Approval decisions are logged with the same provenance as tool calls so that “who approved what” is part of the audit trail and can be used to enforce spending policy.

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

Every Autopilot session emits a **trajectory**: a structured record of the entire run (current implementation: `ReplayBundle` in `crates/autopilot-core/src/replay.rs`; target format: `REPLAY.jsonl v1` per spec in `crates/dsrs/docs/REPLAY.md`). The trajectory includes:

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

#### 5.4.1 Trajectory as the system log

Trajectories function as the OS event log for autonomous execution. Each event is deterministic when possible and includes both intent and outcome. Receipt entries bind economic actions to execution state by linking:

* session id and trajectory hash
* policy bundle id (compiled behavior version)
* job hash or tool invocation id
* payment proof (preimage/txid or equivalent)
* authorization rule id and approval decision

This linkage makes spending auditable and measurable: a payment can be traced to a specific policy decision, tool result, and verification delta.

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
* `delegation_target ∈ {codex_code, rlm, local_tools, swarm_fanout, objective_job}`
* `reasoning`
* `confidence`

**Interpretation of delegation targets:**

* `local_tools`: run the normal tool loop locally (default).
* `codex_code`: delegate to a high-capability coding runtime (e.g., app-server-backed lane).
* `rlm`: switch the execution substrate to RLM/FRLM (see §7).
* `swarm_fanout`: fan out to a provider network for parallel subqueries or specialized skills.
* `objective_job`: dispatch a sandboxed, objectively verifiable job (e.g., tests/builds) as a NIP-90 job type.

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

### 6.5 Economic Routing

Routing decisions in OpenAgents are not only about model selection; they also include **economic routing**: which provider to pay, which rail and asset to settle on, and whether approvals are required. The EconomicRoutingPipeline consumes:

* task description and complexity
* treasury budgets and remaining spend
* asset availability and rail health
* provider reputation and pricing
* approval policy (auto / guardian / deny)

and outputs:

* `lane` (local/cloud/swarm)
* `rail` and `asset_id` (e.g., `BTC_LN`, `USD_CASHU(mint)`)
* `approval_required: bool`
* `reasoning` and `confidence`

This makes cost and compliance part of routing rather than after-the-fact bookkeeping. The TreasuryRouter enforces the decision by issuing quotes, recording receipts, and blocking disallowed payments.

### 6.6 Counterfactual Recording and Shadow Mode

A central challenge in migrating from hand-built heuristics to learned/compiled policies is avoiding regressions. OpenAgents therefore records counterfactuals: for every decision, it stores both:

* **DSPy output** (what the pipeline predicted)
* **legacy output** (what rules would have chosen)
* **market-aware output** (what economic routing would have chosen)
* whether DSPy was used or fell back
* the reason for fallback (low confidence, parse error, timeout, provider missing)

This enables two critical capabilities:

#### 6.6.1 Offline Policy Evaluation

After a session completes and an outcome is known (success/failure/max iterations), we can estimate:

* whether DSPy decision was “correct”
* whether legacy decision would likely have been “correct”
* cases where DSPy outperformed legacy (DSPy wins)
* cases where legacy would have outperformed DSPy (legacy wins)

This is the foundation for safe iteration: we can improve pipelines using data where legacy clearly beats DSPy, without guessing.

#### 6.6.2 Safe Gradual Rollout

Shadow mode allows DSPy to run in parallel without controlling behavior. In shadow mode:

* legacy decisions are executed
* DSPy decisions are recorded
* no behavior changes occur

Once DSPy performance is strong and stable in shadow mode, the confidence gating threshold can be lowered or DSPy can be enabled for specific decision types first (e.g., enable complexity classification before enabling delegation). This incremental rollout significantly reduces operational risk.

### 6.7 Outcome-Coupled Labeling of Decisions

After session completion, OpenAgents maps outcomes to decision correctness. Correctness is not treated as an abstract label; it is derived from execution performance:

* **Complexity correctness:** whether the session converged within expected iteration bounds and context constraints for that complexity level.
* **Delegation correctness:** whether delegation improved or harmed outcomes, factoring cost and failure attribution (e.g., a failure in a delegated lane may be less “wrong” if local lane would likely fail).
* **RLM correctness:** whether recursion was necessary given token growth and verification trajectory; e.g., tasks with >100k effective context or high thrash are strong signals that RLM should have been used.

These labels feed into rolling accuracy trackers per signature and determine when auto-optimization triggers (§8).

### 6.8 Summary

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

Context operations are logged as first-class tool calls with costs, repetition penalties, and verification deltas where applicable. This makes recursion behavior measurable and optimizable in the same way as planning and execution signatures.

### 7.3 FRLM: Federated Recursion Across Providers

OpenAgents extends RLM to **FRLM (Federated RLM)**, where recursive subcalls can be executed across heterogeneous lanes:

* **Local lane**: fast, private, cheap inference and context ops
* **Cloud lane**: strong frontier models for difficult reasoning
* **Swarm lane**: distributed parallel subqueries via paid jobs
* **Specialized lane**: e.g., code-only models, embedding rerankers, lint analyzers

FRLM fanout is expressed in terms of NIP-90 job types. Subjective jobs (e.g., `code_chunk_analysis`, `rerank`) are treated as model outputs that require judging or aggregation, while objective jobs (e.g., `sandbox_run` for tests/builds) return verifiable artifacts. This distinction allows the runtime to pay-after-verify when possible and to route subjective work with tighter budget caps.

Lane selection for FRLM is budget- and value-aware: the root model proposes subcalls, while the runtime enforces spend caps and selects local vs cloud vs swarm lanes based on expected utility, price, and provider reputation.

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

### 8.8 Canary + Progressive Rollout

Policy bundles are deployed with production discipline. A new bundle moves through staged rollout states:

* **candidate** → compiled and evaluated offline
* **staged** → activated for a small, low-risk slice of sessions
* **shadow** → evaluated in parallel without controlling decisions
* **promoted** → becomes default after meeting performance gates

Promotion gates check verified success rate, regression rate, and cost deltas against the previous bundle. If any gate fails, the bundle is rolled back and quarantined for further analysis. This rollout workflow treats policy updates like production software releases rather than prompt edits.

### 8.9 APM and Fleet KPIs

OpenAgents tracks throughput and reliability metrics at the fleet level. We define **APM (Actions Per Minute)** as the number of tool and model actions executed per minute of wall-clock time. APM is paired with success and cost metrics to avoid “fast failure” optimization:

* **Success-normalized APM:** APM weighted by verified success rate.
* **Rework rate:** fraction of actions that repeat or regress verified state.
* **Cost per success:** tokens + sats spent per verified completion.

These KPIs are used by the rollout gates and by the optimizer to prioritize which signatures to recompile.

### 8.10 Summary

OpenAgents closes the loop from autonomy to improvement by (i) recording full sessions, (ii) labeling decisions and steps based on outcomes and verification deltas, (iii) tracking rolling accuracy and drift per signature, and (iv) triggering incremental compilation when performance degrades or enough data accumulates. The design is intentionally conservative—confidence gating, counterfactual baselines, and multi-objective metrics—to avoid Goodhart pathologies and regressions. The next section (§9) describes how this self-improvement loop interacts with OpenAgents’ market substrate: identity, payments, budgets, and provider selection in a decentralized compute marketplace.

## 9 Marketplace and Economic Constraints

OpenAgents treats economic constraints as a first-class control surface for autonomy. Instead of assuming an agent can freely call any model or run unlimited compute, OpenAgents places agents inside an explicit economy: agents have identities, hold budgets, pay for compute, and are accountable for the costs they incur. This framing is not only product-relevant—it is technically stabilizing. Many failure modes in autonomous agents (thrash, over-exploration, unnecessary escalation) are best controlled by making actions *costful* and therefore optimizable under constraints.

### 9.0 Motivation: Economics as Control Surface

Economic controls turn autonomy into a bounded optimization problem rather than an open-ended search. Budgets, approvals, and receipts are the enforcement layer that aligns routing decisions (§6) and recursion fanout (§7) with real-world constraints. OpenAgents therefore treats treasury policy as part of the runtime, not as external accounting.

### 9.1 Neobank: Treasury OS for Agent Fleets

#### 9.1.1 Design goals

* Budget-bounded autonomy with explicit approval gates
* Multi-rail and multi-asset payments without silent risk coupling
* Idempotent payment flows with crash recovery
* Receipts bound to trajectories and policy bundles
* Enterprise legibility (budget in stable units, settle on BTC rails)

#### 9.1.2 Core abstractions

**Table 2: Treasury abstractions**

| Concept | Definition | Why it matters |
| --- | --- | --- |
| Rail | Settlement network (LN, eCash, on-chain, Taproot Assets) | Different latency and trust models |
| AssetId | Currency bound to a rail and issuer (e.g., `USD_CASHU(mint)`) | Prevents silent risk coupling |
| TreasuryRouter | Policy engine selecting rail/asset/budget/approvals | Makes spend controllable |
| Account | Partitioned wallet bucket (ops, escrow, treasury) | Limits blast radius |
| Quote | Prepared payment intent with expiry | Enables idempotency |
| Receipt | Cryptographic proof + context | Auditability + training signal |

#### 9.1.3 Quote state machine

**Figure 3: Quote lifecycle.** `CREATED → UNPAID → PENDING → {PAID | FAILED | EXPIRED}`.

Quotes reserve budget, attach idempotency keys, and protect against retries. If an agent crashes or a rail stalls, the reconciler can safely resume without double spending.

#### 9.1.4 Reconciliation and idempotency

Autonomous agents crash and networks stall. The Neobank therefore runs a reconciliation loop that:

* expires stale quotes
* retries pending payments with idempotency keys
* resolves “unknown” states when a rail returns ambiguous results

This is an operational requirement, not an optimization: without reconciliation, autonomous spending cannot be audited or trusted.

#### 9.1.5 Budget enforcement and approvals

Budgets can be enforced at multiple levels (org/repo/issue) and can trigger approval workflows. Example policy rules include:

* “< $5 equivalent auto-approve”
* “> $200 requires guardian co-signature”
* “unverified provider blocks until reputation threshold met”

Approval decisions are recorded in receipts and trajectories, making policy compliance auditable.

#### 9.1.6 Receipts bound to trajectories

Receipts bind payment events to execution state so a spend can be traced to a job, a policy, and an outcome.

**Table 3: Minimal receipt schema**

| Field | Description |
| --- | --- |
| `receipt_id` | Unique receipt identifier |
| `session_id` | Autopilot session id |
| `trajectory_hash` | Hash of trajectory log |
| `policy_bundle_id` | Compiled policy version |
| `job_hash` | Deterministic job hash (when applicable) |
| `payment_proof` | Preimage/txid/rail proof |
| `rail` / `asset_id` | Settlement rail and asset |
| `amount` | Amount paid |
| `provider_id` | Counterparty identity |
| `approval_rule_id` | Policy rule that authorized spend |
| `timestamp` | Receipt time |

#### 9.1.7 Multi-currency strategies

The TreasuryRouter enforces exposure limits per rail and issuer, supports FX-aware budgeting (e.g., USD-denominated caps), and can route payments across assets when the provider’s pricing and the agent’s budget are in different currencies.

#### 9.1.8 Summary

The Neobank is a treasury OS: it makes autonomous spending programmable, recoverable, and auditable while exposing consistent APIs to the runtime and routing layers.

### 9.2 Compute Marketplace: Verified Jobs and Demand Floor

#### 9.2.1 Autopilot as first buyer

Autopilot acts as the demand floor for compute. As the first large-scale buyer, it seeds provider incentives, generates reliable job volume, and supplies early feedback on provider quality and pricing.

#### 9.2.2 Job taxonomy: objective vs subjective

OpenAgents distinguishes between **objective** jobs (sandboxed runs, tests, builds) and **subjective** jobs (summaries, rankings, analysis). Objective jobs are pay-after-verify; subjective jobs require judges or aggregation. This taxonomy is reflected in Protocol job schemas and determines how receipts are generated and how disputes are handled.

#### 9.2.3 Settlement patterns

We use three settlement patterns depending on job type and trust:

* **Pay-after-verify** for objective jobs with deterministic checks.
* **Escrow + dispute window** for jobs with delayed or partial verification.
* **Reputation-weighted prepay** for low-latency subjective tasks.

#### 9.2.4 Provider capability and reputation

Provider selection incorporates advertised capabilities, historical success rates, latency distributions, and cryptographic receipts. Providers that fail objective verification are down-ranked or excluded. This turns the marketplace into a measurable system rather than a trust-based marketplace.

#### 9.2.5 Routing under budget

Marketplace routing integrates with economic routing (§6.5) and FRLM fanout (§7.3). Providers, rails, and assets are selected jointly to satisfy both performance and budget policies, and the TreasuryRouter enforces spend caps at execution time.

#### 9.2.6 Summary

The compute marketplace is not only a network of providers; it is a verification-driven routing system where payments, receipts, and outcomes are bound together.

### 9.3 Exchange: Liquidity and FX Routing for Agents

#### 9.3.1 Why Exchange exists

A compute provider may price in USD while an agent holds BTC. The Exchange layer provides liquidity and routing so agents can pay in the provider’s required asset without human intervention. This enables enterprise budgeting in stable units while still settling on Bitcoin rails.

#### 9.3.2 Actors: Treasury Agents

Treasury Agents are specialized liquidity providers. They quote FX rates, route payments across rails, and earn spreads for providing liquidity. This turns treasury routing into a market with competition and measurable performance.

#### 9.3.3 Protocol-native design

The Exchange is built on existing Nostr protocol surfaces:

* **NIP-69** for P2P order events
* **NIP-60** for wallet state synchronization
* **NIP-32** labels for reputation metadata
* **NIP-44** encryption for sensitive quote payloads

This avoids inventing bespoke exchange protocols while remaining interoperable with existing Nostr tooling.

#### 9.3.4 Settlement progression v0 → v2

We outline a staged progression:

* **v0 (reputation-based):** quote + pay with trust-weighted settlement
* **v1 (atomic eCash):** atomic swaps for mint-based assets
* **v2 (cross-mint):** multi-hop settlement across mints with escrow

Each stage increases safety and interoperability while retaining the same quote and receipt interfaces.

#### 9.3.5 Summary

The Exchange layer turns treasury routing into a liquidity market, enabling agents to operate with multi-rail, multi-asset budgets without manual intervention.

### 9.4 End-to-End Payment-Linked Autonomy

An end-to-end run links the policy decision to a payment and a verified result: the agent selects a lane and rail, the TreasuryRouter issues a quote, a provider executes a job, verification determines success, and a receipt binds the spend to the trajectory and policy bundle. This is the economic analog of the verification loop in §5 and the routing policies in §6.

### 9.5 Summary

OpenAgents’ market layer combines treasury controls, a verified compute marketplace, and a liquidity exchange so autonomous agents can operate economically without sacrificing auditability or verification guarantees. The next section (§10) describes how we evaluate these claims empirically.

## 10 Evaluation

OpenAgents targets a domain where “correctness” is often measurable (tests/builds), but autonomy introduces additional axes that typical LLM benchmarks do not capture: stability across long sessions, cost-aware routing, resistance to degenerate loops, and reproducible provenance. Our evaluation is designed to measure **verified task success** and the system properties that determine whether autonomy is usable in practice.

### 10.1 Evaluation Questions

We answer five core questions:

1. Does DSPy compilation improve verified success and cost versus fixed prompts?
2. Does outcome-coupled optimization reduce thrash/repetition and improve convergence?
3. Do RLM/FRLM improve success on long-context and long-horizon repo tasks?
4. Does market-aware routing reduce cost for a fixed success rate (or improve success under fixed budgets)?
5. Do canary/shadow rollouts reduce regressions versus always-on policy updates?

### 10.2 Task Suites

We evaluate on three complementary suites.

**Suite A: RepoOps (OpenAgents internal)**

* Real repository tasks with pinned harnesses (tests/builds/lints).
* Includes bug fixes, refactors, and cross-file changes.
* Used to measure end-to-end verified success under realistic constraints.

**Suite B: SWE-bench Verified subset + Polyglot**

* Provides comparability to existing agent literature.
* Ensures tasks span multiple languages and repository styles.

**Suite C: Long-Context / Session-Scale**

* Tasks with large codebases, long logs, and multi-iteration debugging.
* Stress-tests context growth and RLM/FRLM stability.

### 10.3 Experimental Conditions (Ablations)

We run the following configurations:

* **C0 Baseline:** legacy prompts + rule-based routing.
* **C1 DSPy-only:** compiled signatures, legacy routing.
* **C2 DSPy + outcome-coupled:** compiled signatures + outcome-based optimization.
* **C3 DSPy + outcome-coupled + RLM:** RLM enabled for long-context tasks.
* **C4 + FRLM:** federated fanout across local/cloud/swarm lanes.
* **C5 Canary/shadow:** progressive rollout with shadow evaluation.

### 10.4 Metrics (Explicit Definitions)

**Success / correctness**

* **Verified Success Rate** = successful tasks / total tasks.
* **Iterations-to-Success** = mean iterations for successful tasks.

**Stability / thrash**

* **Repetition Rate** = repeated actions / total actions.
* **Thrash Index** = repeated identical tool calls per session / session length.

**Verification progress**

* **Verification Delta Trajectory** = mean change in failing tests per iteration.

**Cost / efficiency**

* **APM** = total actions / total minutes.
* **Success-Normalized APM** = APM × Verified Success Rate.
* **Cost per Success** = (tokens + sats spent) / successful tasks.

**Market quality**

* **Provider Reliability** = verified jobs / total jobs.
* **Escalation Rate** = delegated jobs / total jobs.
* **Rail Mix** = spend share per rail/asset.

### 10.5 Tables and Figures to Add

* **Table 4:** Task suite composition and harnesses.
* **Table 5:** Ablation results (success, cost, thrash) across suites.
* **Table 6:** Market routing breakdown (lanes, rails, assets, approvals).
* **Figure 4:** Verification delta trajectories by condition.
* **Figure 5:** Cost vs success scatter plot (per suite).

### 10.6 Methodology Details

We pin environments via container images or Nix-style lockfiles, record all tool invocations, and run each condition with a fixed budget and deterministic verification harness. Sessions are capped by iteration and spend limits, and we record provider failures to avoid conflating model errors with marketplace faults. All runs store trajectories, receipts, and policy bundle hashes for reproducibility.

### 10.7 Case Studies

We include short, high-signal case studies:

* **Context rot vs RLM recovery:** identical tasks with and without RLM.
* **Outcome-coupled vs format-only:** examples where JSON validity improved but verification stalled.
* **Market routing failures:** provider timeouts and fallback behavior with receipts.

### 10.8 Summary

The evaluation framework measures verified task success alongside stability, cost-efficiency, and market routing behavior. It isolates the impact of compiled cognition, recursion, and economic routing while remaining reproducible across repositories and providers. The next section (§11) addresses safety and governance.

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

OpenAgents changes the autonomy problem from "get a model to do tasks" to "build a system that can reliably improve at doing tasks under constraints." Compiled cognition enables systematic optimization; verification provides ground truth; recursion enables long-horizon reasoning; and markets enforce cost-aware behavior. The remaining challenges—generalization, objective hacking, robust federation, and governance—define the next research frontier for production-grade self-improving agents.

### 12.6 MVP Gates

The following gates define minimum viable product readiness for OpenAgents:

| Gate | Status | Notes |
|------|--------|-------|
| **Verified Patch Bundle emission** | 🔄 Partial | `ReplayBundle` exists; `REPLAY.jsonl` exporter pending |
| **ToolCallSignature wired** | ⏳ Spec only | Defined in `crates/dsrs/docs/SIGNATURES.md` |
| **ToolResultSignature wired** | ⏳ Spec only | Defined in `crates/dsrs/docs/SIGNATURES.md` |
| **Tool params schema validation** | ✅ Implemented | Execution runtime validates (`crates/adjutant/`) |
| **Policy bundle pin/rollback** | ⏳ Not started | CLI commands pending |
| **Replay viewer CLI** | ⏳ Spec only | `adjutant replay` command pending |
| **Outcome-coupled metrics wiring** | 🔄 Partial | `step_utility` defined; full labeling pending |
| **Shadow/canary counterfactual recording** | ⏳ Spec only | Architecture defined; not wired |
| **DSPy decision pipelines** | ✅ Implemented | `ComplexityPipeline`, `DelegationPipeline`, `RlmTriggerPipeline` |
| **Self-improvement loop** | ✅ Implemented | `SessionStore`, `AutoOptimizer` in `crates/adjutant/` |

> **Note:** "Self-improving" claims are valid only once the full loop is wired: trajectory logging → step_utility labels → policy bundles with pin/rollback → promotion gates. See [ROADMAP.md](./ROADMAP.md) for current status.


## 13 Conclusion

We presented **OpenAgents**, an “agentic operating system” for building autonomous agents that can act as real software and economic actors: they hold cryptographic identity, execute work in sandboxed environments, operate under explicit budgets and approval policies, purchase compute and skills from open markets, and produce auditable trajectories and cryptographic receipts that bind spending to verified outcomes. Rather than treating autonomy as a prompt-engineering problem, OpenAgents treats it as a systems problem: **interfaces, isolation, scheduling, verification, provenance, and economics** are first-class primitives.

The technical core of OpenAgents is **compiled cognition**. By replacing brittle prompt strings with typed **Signatures** and composable **Modules**, and compiling them with optimizers against explicit metrics, OpenAgents turns agent behavior into a versioned artifact that can be measured, A/B tested, rolled back, and improved without rewriting orchestration code. This compilation loop is grounded in **verifiable execution**: deterministic checks (tests/builds/sandbox runs) serve as the primary reward signal for autonomous coding, while trajectories provide both audit trails and training data. To remain robust as context and session history grow, OpenAgents integrates **RLM/FRLM** as an execution substrate that externalizes long context into a programmable environment and enables budget-bounded recursive sub-queries, including federated fan-out across local, cloud, and decentralized providers.

A central claim of this paper is that large-scale autonomy requires an economic substrate. OpenAgents’ treasury layer (Neobank) formalizes spending through multi-rail and multi-asset abstractions, idempotent quote state machines with reconciliation, multi-level budgets (org/repo/issue), and receipts that link each payment to a policy decision and a trajectory. Above this, an exchange layer provides liquidity and FX routing so agents can pay providers in the required rail while operators budget in stable units. These mechanisms do more than pay bills: they act as **controls** that bound failure modes, discourage thrash, and enable cost-aware optimization of routing and recursion policies.

OpenAgents is designed as a conservative self-improvement system. Decisions are confidence-gated, counterfactuals against legacy behavior are recorded, and policy updates are deployed via shadow-mode and canary rollouts to reduce regressions and Goodhart-style objective hacking. While our current implementation focuses on self-improvement at the level of compiled policies and routing, the same infrastructure—archives of trajectories and policy bundles, outcome-coupled selection, and provenance—supports more open-ended forms of improvement over time, including distillation from expensive “teacher” lanes into cheaper local and swarm execution.

We believe the broader implication is that progress toward reliable autonomous agents will be driven as much by **systems architecture and economic constraints** as by model capability alone. OpenAgents offers a practical blueprint: make agent behavior programmable and compilable, make execution verifiable and replayable, make long-horizon reasoning scalable via recursion, and make autonomy economically legible through budgets and receipts. This combination shifts autonomy from an artisanal craft into an engineering discipline—one that can support fleets of agents operating continuously in real environments, improving from experience while remaining auditable, bounded, and cost-aware.

## References

- DSPy: declarative LM programming and signatures (TODO:CITE)
- MIPROv2 and prompt compilation (TODO:CITE)
- OMNI / OMNI-EPIC interestingness curricula (TODO:CITE)
- Darwin Gödel Machine and empirical self-improvement (TODO:CITE)
- Recursive Language Models / FRLM (TODO:CITE)
- Nostr protocol basics and NIP-42/NIP-44 (TODO:CITE)
- NIP-90 Data Vending Machines (TODO:CITE)
- NIP-57 zaps / LNURL payment primitives (TODO:CITE)
- L402 pay-per-call authentication (TODO:CITE)
- FROST threshold signatures and FROSTR adaptations (TODO:CITE)
- Proof-carrying execution / reproducible builds (TODO:CITE)

## Appendix G: Protocol Surface (High-Level)

This appendix summarizes the protocol surface used by OpenAgents. It is intentionally high-level and omits implementation details. For the canonical, maintained protocol reference, see [docs/PROTOCOL_SURFACE.md](./docs/PROTOCOL_SURFACE.md). For canonical terminology, see [GLOSSARY.md](./GLOSSARY.md).

### G.1 Event Kinds and Lifecycles

* **NIP-90 Job Request / Result:** compute and sandbox jobs with typed schemas.
* **NIP-SA Tick Request / Tick Result:** lifecycle boundaries for autonomous execution.
* **NIP-SA Trajectory Session / Trajectory Event:** structured audit logs for plans, tool calls, and outcomes.
* **Payment Events:** receipts or zaps linking spend to jobs and sessions.

### G.2 Job Schema Surface

Minimum fields for a job schema:

* `inputs` (typed fields)
* `outputs` (typed fields)
* `verification_mode` (objective | subjective)
* `hash_rules` (deterministic hashing)
* `provenance` (model/provider metadata)

### G.3 Receipt Surface

Receipts bind payments to execution state:

* `receipt_id`
* `session_id`
* `trajectory_hash`
* `policy_bundle_id`
* `job_hash`
* `payment_proof`
* `rail` / `asset_id`
* `amount`
* `provider_id`
* `approval_rule_id`
* `timestamp`

### G.4 Trajectory Format

Trajectory records include:

* signature inputs/outputs per call
* tool invocations and results
* verification commands and deltas
* lane/provider choices and fallbacks
* receipts and spend metadata
* termination reason and summary


---

Feedback to incorporate SYNTHESIS.md:

Below are the **paper sections that are currently underpowered relative to your “Agentic OS” doc**, plus **concrete instructions** for a writer agent to expand them in a way that (a) stays arXiv/technical, (b) clearly differentiates OpenAgents, and (c) ties back to DSPy/OMNI/DGM/RLM without drifting into manifesto mode.

---

# What to flesh out further (priority order)

## 1) §9 Marketplace and Economic Constraints — **needs the most work**

Right now it’s generic. Your OS doc has *actual novel substance* (Neobank/TreasuryRouter, rails/assets, quotes, reconciliation, receipts tied to trajectories, Exchange layer, Treasury Agents, demand-floor via Autopilot). This should become one of the paper’s strongest differentiators.
Note: Implemented full §9 rewrite with Neobank/TreasuryRouter, quote state machine, receipts schema, marketplace taxonomy, and Exchange layer.

## 2) §3 System Overview — **missing the “Agentic OS primitives”**

The paper currently frames OpenAgents as compiled cognition + verifiable execution + recursion + economy. Good—but the OS doc provides concrete primitives and protocol components (FROST/FROSTR, Bifrost, NIP-SA, NIP-90, Spark/LN, protocol layers). Those need to appear as the *actual stack*.
Note: Added §3.1.1–§3.1.3 with primitives table, protocol substrate, and identity/payments unification including FROST/FROSTR + Bifrost.

## 3) §5 Agent Runtime — **needs Plan 9 / OS framing + state surfaces**

Your OS doc emphasizes “OS semantics”: identity, budgets, receipts, trajectories, tick lifecycle, filesystem-like mounts. The paper’s runtime section should include those details explicitly.
Note: Expanded §5.1 state surfaces, added OS-style mounts language, approval gates, and a “Trajectory as system log” subsection with receipt linkage.

## 4) §8 Self-Improvement Loop — **needs fleet ops + canary/shadow rollout**

You already have the learning loop, but the doc has strong operational machinery: APM, canary deployments, promotion gates, regression detection. That should be formalized as “production-grade policy deployment.”
Note: Added §8.8 canary/progressive rollout with promotion gates and §8.9 APM/KPI definitions.

## 5) §10 Evaluation — **currently a template; needs real experimental plan**

You need concrete experiments that validate (a) DSPy compilation, (b) RLM/FRLM benefit, (c) market-aware routing + budget enforcement, and (d) end-to-end economic flows (Autopilot-as-buyer, provider reliability, cost arbitrage).
Note: Rewrote §10 with explicit questions, suites, ablations, metrics, and methodology details.

## 6) §2 Related Work — **needs tighter positioning**

You reference DSPy/OMNI/DGM/RLM, but should also cover:

* agent payment / pay-per-call protocols (L402, LNURL, NIP-57 zaps)
* decentralized job markets (NIP-90 DVMs)
* agent identity protocols (Nostr identity, threshold signing)
* verifiable/receipted work (proof-carrying execution, reproducible builds-ish framing)
Note: Expanded §2 with new subsections on threshold identity, DVMs, pay-per-call payments, and receipted work, plus TODO:CITE placeholders.

## 7) §6 Decision Pipelines and Routing — **should incorporate budgets + rails**

Routing isn’t only “which model”; it’s “which provider + which rail + which asset + which approval workflow.” That’s in the OS doc; paper should reflect it.
Note: Added §6.5 Economic Routing and extended counterfactual logging to include market-aware decisions.

## 8) §7 RLM/FRLM — **fine conceptually, but needs tighter integration hooks**

You should explicitly specify *what FRLM fans out to* (NIP-90 job kinds; subjective vs objective verification; cost caps), and how its recursion actions are logged and optimized like DSPy.
Note: Added FRLM/NIP-90 job taxonomy and logging of context ops as tool calls with costs.

---

# Instructions to the writer agent

## Global instructions (before editing sections)

1. **Keep the arXiv paper technical.**

   * Move the OS doc’s political framing into 1–2 sentences of motivation at most (optional), but don’t let it dominate.
   Note: Kept new sections technical and focused on primitives, state machines, and evaluation.
2. **Introduce a “Primitives Table” early (Section 3).**

   * One table mapping: Identity / Transport / Payments / Treasury / Marketplace / Verification / Transparency → concrete OpenAgents components (FROSTR/Bifrost, NIP-90, Spark, Neobank, etc.).
   Note: Added Table 1 in §3.1.1 mapping primitives to components.
3. **Add 2–3 figures.**

   * Fig A: Stack diagram (protocol → treasury → execution → products).
   * Fig B: End-to-end flow (Autopilot task → decisions → tool exec → verification → payment/receipt).
   * Fig C (optional): Neobank quote state machine + reconciliation loop.
   Note: Added figure callouts for stack (Figure 1), flow (Figure 2), and quote lifecycle (Figure 3).
4. **Add a “Protocol Surface” appendix.**

   * Enumerate event kinds / job schemas / receipts / trajectory formats at a high level (without drowning in implementation).
   Note: Added Appendix G with event kinds, schema, receipt, and trajectory surface summaries.

---

## Section-by-section expansion tasks

### §3 System Overview — expand into OS-style primitives
Note: Implemented §3.1.1–§3.1.3 with primitives table, protocol substrate, and identity/payments unification.

**Add subsections:**

* **3.1.1 Agentic OS primitives** (Identity, Payments, Budgets, Receipts, Trajectories, Compute/Skills markets)
* **3.1.2 Protocol substrate** (Nostr as transport; NIP-90 compute; NIP-SA agent lifecycle; NIP-57 payments; NIP-44 encryption)
* **3.1.3 Identity/payments unification** (BIP39 root, derivation into Nostr + wallet keys; threshold-protected variant)

**Concrete content to include:**

* FROST/FROSTR + Bifrost: why threshold identity is required for “sovereign agents”
* “Autopilot is wedge; markets are platform”: one paragraph mapping wedge→platform path to paper claims

---

### §5 Agent Runtime — add OS semantics and audit surfaces
Note: Added OS-style state surfaces, approvals, receipt linkage, and a dedicated trajectory-as-log subsection.

**Add details:**

* Explicit “agent state surfaces” (identity, wallet, compute, tools, logs) and how they’re exposed/queried.
* Add a short subsection: **“Trajectory as the system log”** (rlog/trajectory events, deterministic tool logs, verification logs).
* Include **approval workflows and autonomy levels** as runtime gates (supervised/semi/auto).

**Make it concrete:**

* Define what a “receipt” links to: (session id, policy bundle id, job hash, payment proof/preimage/txid)
* Mention tick lifecycle events align with NIP-SA style lifecycle logging.

---

### §6 Decision Pipelines and Routing — incorporate budgets + market routing
Note: Added Economic Routing and expanded counterfactual routing to include market-aware decisions; updated delegation targets.

**Expand the routing model beyond model selection:**

* Add a subsection: **6.5 Economic routing**
  Inputs include budgets, asset availability, rail availability, provider reputation; outputs include lane + rail + asset + approval requirement.
* Add a subsection: **6.6 Counterfactual routing**
  Expand shadow mode to include “legacy vs DSPy vs market-aware policy.”

**Make “delegation_target” richer:**

* codex_code / rlm / local_tools is fine, but add “swarm_fanout” and “objective_job” (sandbox_run) explicitly.

---

### §7 RLM/FRLM — tie recursion to NIP-90 + verification taxonomy
Note: Added NIP-90 job taxonomy, subjective vs objective split, and logging of context ops as tool calls.

**Add two concrete integration points:**

1. **FRLM fanout primitives = NIP-90 job types**

   * subjective: code_chunk_analysis, rerank
   * objective: sandbox_run (tests/builds)
2. **RLM actions are themselves logged and scored**

   * treat `grep/peek/partition/map` as tool calls with costs and repetition penalties, enabling DSPy-style optimization of recursion behavior.

**Add one paragraph on:**

* How FRLM chooses between local vs cloud vs swarm based on budget and expected value.

---

### §8 Self-Improvement Loop — add deployment discipline
Note: Added canary/progressive rollout flow and APM/KPI definitions for fleet ops.

You already have the learning loop; add **production guardrails** from the OS doc:

**Add subsections:**

* **8.8 Canary + progressive rollout**
  Describe how a new policy bundle is deployed to a small fraction of sessions; promote if metrics hold; rollback otherwise.
* **8.9 APM + success + cost as primary KPIs**
  APM isn’t just “fun”—it’s a fleet throughput metric. Define it formally and pair it with success rate and rework/regression rate.

**Include:**

* Promotion gates: candidate → staged → shadow → promoted (your doc already sketches this).

---

### §9 Marketplace and Economic Constraints — rewrite as a major contribution
Note: Rewrote §9 with Neobank, Compute Marketplace, Exchange, and end-to-end payment-linked autonomy.

This section should be expanded into **three major subsections**:

#### 9.1 Neobank / Treasury OS (new)

Include:

* TreasuryRouter concept
* Rail + AssetId abstraction (BTC_LN vs USD_CASHU(mint) etc.)
* Quotes state machine (CREATED→UNPAID→PENDING→PAID/FAILED/EXPIRED)
* Reconciliation + idempotency requirements (agents crash, networks stall)
* Receipts that bind payments to trajectories + policies

#### 9.2 Compute Marketplace (expand)

Include:

* Autopilot as **demand floor** (first buyer)
* Verification taxonomy: objective jobs vs subjective jobs, and how payment differs
* Provider reputation tiers and basic penalties

#### 9.3 Exchange / Liquidity layer (new)

Include:

* “Treasury Agents” as maker liquidity providers
* NIP-native approach: NIP-69 for P2P order events, NIP-60 for wallet state, NIP-32 labels for reputation
* Settlement progression v0→v2 (reputation-based → atomic eCash swap → cross-mint)

**Important writing constraint:**
Keep this section technical and mechanistic: primitives, state machines, and flow diagrams. Avoid manifesto language.

---

### §10 Evaluation — convert from “outline” to “actual plan”
Note: Replaced with explicit questions, suites, ablations, metrics, and methodology details.

**Add specific evaluation questions:**

1. **Does DSPy compilation improve verified success/cost vs fixed prompts?**
2. **Does outcome-coupled optimization reduce thrash/repetition and improve convergence?**
3. **Do RLM/FRLM improve success on long-context + long-horizon repo tasks?**
4. **Does market-aware routing reduce cost for a fixed success rate (or increase success under same budget)?**
5. **Do canary/shadow rollouts reduce regressions vs always-on policy updates?**

**Add concrete metrics:**

* Verified success rate
* Time/iterations to success
* APM + success-normalized APM
* Cost per success (tokens + sats spent)
* Repetition rate / thrash index
* Verification delta trajectory
* Provider reliability stats (timeouts, failure rates)

**Add at least two evaluation suites:**

* SWE-bench Verified subset + Polyglot (comparability)
* “OpenAgents RepoOps Suite” (your own: real repos + harnesses)
* Optional: Long-context doc/repo suite to showcase RLM

---

### §2 Related Work — tighten and widen
Note: Expanded §2 with threshold identity, DVMs, pay-per-call payments, and receipted work subsections.

**Add short subsections:**

* Crypto identity + threshold signing for agents (brief)
* Decentralized job markets / DVMs
* Pay-per-call protocols (L402) and LN-native payments in protocols (NIP-57)
* Prior “agent economies” / “agent wallets” if you want (keep brief)

**Goal:** situate OpenAgents as the synthesis of:

* compiled cognition (DSPy),
* open-ended task selection / anti-Goodhart (OMNI),
* self-improvement loops (DGM),
* long-horizon inference (RLM),
* and *economic substrate* (your unique part).

---

# Deliverable format for the writer agent
Note: Delivered a single integrated rewrite with expanded sections, figure callouts, and a new References section with TODO:CITE placeholders.

Ask the writer agent to produce **one PR-style update** with:

1. Expanded sections: §3, §5, §6, §8, §9, §10, §2
2. New figures (described in text; actual diagram generation optional)
3. A “Protocol Surface” appendix listing key NIPs, job schemas, receipt fields
4. A tightened references section with placeholders marked `TODO:CITE`

**Style constraints:**

* Keep tone “systems paper,” not manifesto.
* Use short definitions the first time a term appears (Neobank, TreasuryRouter, AssetId, etc.).
* Prefer crisp state machines and flows over sweeping claims.

Below is a **drop-in skeleton** for an expanded **§9** and a **concrete §10 evaluation plan** (with suggested figures/tables), written as instructions your writer agent can follow almost mechanically.

---

# Writer Agent Instructions: Expanded §9 Skeleton
Note: Implemented the §9 rewrite; this skeleton now serves as traceability for the applied changes.

## Section 9: Marketplace and Economic Constraints (rewrite + expand)

### 9.0 Motivation: Economics as Control Surface (1 short paragraph)

* State that OpenAgents treats money/budgets as an *operational control plane* for autonomy (caps, approvals, cost-aware routing).
* Tie to earlier sections: routing (§6), FRLM (§7), self-improvement (§8).

---

## 9.1 Neobank: Treasury OS for Agent Fleets (new, 2–3 pages)

### 9.1.1 Design goals (bulleted)

* Budget-bounded autonomy
* Multi-rail/multi-asset support
* Idempotent payments + crash recovery
* Auditability: receipts linked to trajectories + policies
* Enterprise legibility: budgets in USD units while settling in BTC rails

### 9.1.2 Core abstractions (define + one table)

Include a table:

| Concept        | Definition                                                              | Why it matters                |
| -------------- | ----------------------------------------------------------------------- | ----------------------------- |
| Rail           | Payment network + settlement (LN, Cashu mint, on-chain, Taproot Assets) | Different failure modes/trust |
| AssetId        | “currency on a rail” (BTC_LN, USD_CASHU(mint), USDT_TA(group))          | Prevents silent risk coupling |
| TreasuryRouter | Policy engine selecting rail/asset/approvals                            | Makes spend controllable      |
| Account        | Partitioned wallet bucket (operating/escrow/treasury/payroll)           | Limits blast radius           |
| Quote          | Prepared payment intent w/ reservation + expiry                         | Enables idempotency           |
| Receipt        | Cryptographic + contextual spend proof                                  | Audit + training signal       |

### 9.1.3 Quote state machine (include diagram + text)

Add a small state machine figure:

**Figure 9:** Quote lifecycle
`CREATED → UNPAID → PENDING → {PAID | FAILED | EXPIRED}`

Explain:

* reservation of funds/proofs
* expiry + reconciliation
* idempotency keys

### 9.1.4 Reconciliation + idempotency (operational necessity)

Describe:

* agents crash, networks stall, mints go offline → must reconcile
* background reconciler:

  * expires reservations
  * retries pending payments safely
  * resolves “unknown” states
* idempotency keys prevent double spend

### 9.1.5 Budget enforcement + approvals (connect to autonomy levels)

Define:

* org / repo / issue budgets (from your doc)
* approval workflows for high-risk spends
* rule examples (keep short):

  * “< $5 auto-approve”
  * “> $200 requires guardian co-sign”
  * “untrusted provider blocks”

### 9.1.6 Receipts bound to trajectories (key differentiator)

Define receipt fields and why:

* payment proof (preimage/txid)
* session id (trajectory)
* policy bundle id (compiled behavior version)
* authorization rule id
* job hash (for objective jobs)
* lane/provider identity

Add **Table 4: Receipt schema (minimal)**.

### 9.1.7 Multi-currency strategies (brief but concrete)

List the three approaches:

1. USD-denom budgets, settle in sats at spot rate
2. USD eCash (Cashu mint-issued denom)
3. Taproot Assets stables (planned)

Include mint trust model as a policy problem:

* allowlists + caps + diversification
* reputation labels (NIP-32) as input

### 9.1.8 Summary (1 short paragraph)

* Neobank turns “agent pays for compute” into “agent pays under a treasury policy with receipts + recovery.”

---

## 9.2 Compute Marketplace: Verified Jobs + Demand Floor (rewrite, 2–3 pages)

### 9.2.1 Autopilot as first buyer (demand floor)

Explain:

* marketplace cold start solved by Autopilot purchasing compute continuously
* what Autopilot buys: think/run/index/verify
* why demand-first avoids “ghost town” provider churn

Include **Figure 10:** Demand-floor flywheel (User pays → Autopilot buys compute → providers earn → network liquidity → cheaper/faster Autopilot → more users)

### 9.2.2 Job taxonomy: objective vs subjective (tie to Protocol crate)

Include a table:

| Job type   | Example                   | Verification                           | Settlement               |
| ---------- | ------------------------- | -------------------------------------- | ------------------------ |
| Objective  | sandbox_run (tests/build) | deterministic: exit code + hashes      | pay-after-verify         |
| Objective  | repo_index                | deterministic-ish: schema + spot-check | pay-after-verify         |
| Subjective | code_chunk_analysis       | judge/consensus                        | best-of-N / adjudication |
| Subjective | retrieval_rerank          | judge/consensus                        | majority / redundancy    |

### 9.2.3 Settlement patterns

Describe:

* objective jobs: pay-after-verify, receipt includes job hash
* subjective jobs: tiered verification:

  * reputation-only
  * best-of-N
  * human sampling
  * “skill-wrapped inference” as accountability

### 9.2.4 Provider capability + reputation

Include:

* provider announcements (NIP-89 kind 31990)
* provider tiers (Tier 0..3) and penalties (timeout, verification fail)
* supply classes (SingleNode, BundleLAN, BundleRack, InstanceMarket, ReservePool)

Optional figure:

* **Figure 11:** provider tier ladder + routing weights

### 9.2.5 Routing under budget (connect to §6 + §7)

Explain:

* routing considers price, reliability, latency, budget state
* FRLM fanout bounded by budget policies

### 9.2.6 Summary (1 paragraph)

* compute market + receipts + budgets is the economic substrate for scalable autonomy.

---

## 9.3 Exchange: Liquidity + FX Routing for Agents (new, 2 pages)

### 9.3.1 Why Exchange exists

* agents need to pay providers in different units/rails
* enterprise budgets in USD; providers may price in sats
* routing needs liquidity + hedging options

### 9.3.2 Actors: Treasury Agents

* makers who quote markets and earn spreads
* bootstrap strategy: OpenAgents seeds early liquidity as a demo/demand starter (if you want to say this, keep neutral tone)

### 9.3.3 Protocol-native design (list NIPs used)

Include a table:

| NIP          | Role                        |
| ------------ | --------------------------- |
| NIP-69       | P2P order events            |
| NIP-60/61/87 | Cashu wallet/mint discovery |
| NIP-47       | wallet control              |
| NIP-32       | reputation attestations     |
| NIP-90/89    | RFQ/service announcements   |

### 9.3.4 Settlement progression v0→v2 (describe as roadmap)

* v0 reputation-based
* v1 atomic eCash swap (P2PK, HODL invoice)
* v2 cross-mint swap via Treasury Agent bridge

Include a simple flow diagram for v1 atomic swap.

### 9.3.5 Summary

* exchange makes multi-rail multi-currency spending practical at scale.

---

## 9.4 End-to-End Payment-Linked Autonomy (bridging section, 0.5–1 page)

Provide one integrated example in prose:

* Autopilot needs sandbox_run → gets quote → budget approves → executes job → verification passes → payment releases → receipt links to trajectory.

This ties the whole paper together and sets up evaluation.

---

# Writer Agent Instructions: Concrete §10 Evaluation Plan
Note: Implemented the §10 rewrite with explicit questions, suites, ablations, and metrics.

## Rewrite §10 to contain: questions, methodology, datasets, metrics, ablations, tables/figures.

### 10.1 Evaluation questions (explicit)

Add these as numbered Q1–Q6:

* **Q1**: Do DSPy-compiled signatures improve verified task success over fixed prompts?
* **Q2**: Does outcome-coupled optimization reduce thrash and improve convergence vs format-only metrics?
* **Q3**: Do RLM/FRLM modes improve success on long-context/long-horizon repo tasks?
* **Q4**: Does market-aware routing reduce cost for a fixed success rate (or increase success under fixed budgets)?
* **Q5**: Do canary/shadow deployments reduce regressions compared to always-on policy updates?
* **Q6**: Can we distill “teacher lane” performance into cheaper lanes via compilation (cost arbitrage)?

---

## 10.2 Task suites (concrete)

Define 3 suites:

### Suite A: RepoOps (OpenAgents internal suite)

* 50–200 tasks across real repos (Rust + TS ideally)
* categories: bugfix, feature add, refactor, CI break
* harness: deterministic tests/build
* report: success, time, cost, diffs

### Suite B: SWE-bench Verified subset + Polyglot

* for comparability
* pass@1 only
* run inside sandbox
* report baseline vs improved policies

### Suite C: Long-Context / Session-Scale suite

* select large repos + tasks requiring global reasoning
* include artificially expanded logs/history to force context rot
* measure RLM vs non-RLM

---

## 10.3 Experimental conditions (the main ablations)

Define at least these conditions:

### C0 Baseline (legacy)

* rule-based routing + fixed prompts

### C1 DSPy-only

* DSPy signatures + MIPROv2 optimized using format metrics only

### C2 DSPy + outcome-coupled

* DSPy signatures + optimization using verification delta + repetition + cost

### C3 DSPy + outcome-coupled + RLM

* same as C2, but RLM trigger enabled and RLM executor used when triggered

### C4 + FRLM

* same as C3, but recursion fanout allowed to swarm with budget caps

### C5 Canary/shadow rollout

* new policy bundle deployed as canary (10% tasks), compare to baseline policy; then progressive rollout

---

## 10.4 Metrics (must be explicit formulas)

Include these categories:

### Success / correctness

* Verified success rate (% tasks where tests/build pass)
* Iterations-to-success (median + distribution)

### Stability / thrash

* Repetition rate = repeated tool calls / total tool calls
* Thrash index = entropy of touched files + repeat opens + null deltas (define simply)

### Verification progress

* Verification delta per iteration (failing tests count)
* Regression frequency (iterations that worsen delta)

### Cost / efficiency

* Total tokens
* Total sats spent
* Cost per success = total spend / #success
* Success per 1k tokens; success per 10k sats
* APM (actions per minute) + “success-normalized APM” (APM × success indicator)

### Market quality

* Provider timeout rate
* Provider failure rate (objective verification fails)
* Price vs latency curves

---

## 10.5 Tables and figures to add (very specific)

**Table 5:** Results summary across conditions (C0–C4) on Suite A
Columns:

* success rate
* median cost per success
* median iterations
* repetition rate
* APM
* verification regressions

**Figure 12:** CDF of cost per success (C0–C4)
**Figure 13:** Verification delta over iterations (median + IQR)
**Figure 14:** RLM vs non-RLM success by context size bucket
**Figure 15:** Canary rollout chart (baseline vs canary performance over time)

**Table 6:** Provider marketplace stats (swarm experiments)

* number providers
* fill rate
* median latency
* verification fail rate
* average price

---

## 10.6 Methodology details (make reproducible)

* fixed seeds where possible
* pinned tool environments via containers
* policy bundles versioned and recorded
* report confidence intervals (bootstrap)
* for subjective jobs: describe redundancy/judging policy used

---

## 10.7 Case studies (short but high-signal)

Include 2–3 “deep dives”:

1. context rot case (non-RLM fails vs RLM succeeds)
2. objective hacking attempt caught by outcome-coupled metric
3. market failure case (provider timeout + fallback works)

Each case study includes:

* trajectory excerpt summary
* verification timeline
* cost breakdown
* policy bundle id

---

# Output expectation for the writer agent
Note: Implemented as a single PAPER.md update with sections expanded per the checklist.

1. Replace current §9 with the above structure (9.0–9.4).
2. Replace current §10 with the above evaluation plan (10.1–10.7).
3. Add placeholders for figures and tables exactly as specified.
4. Keep language “systems paper,” no manifesto tone; keep claims testable.
5. Insert `TODO:CITE` tags wherever you reference NIPs, Spark, Cashu, etc.
