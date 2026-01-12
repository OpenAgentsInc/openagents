## OpenAgents Implementation Roadmap

This roadmap is organized as **phases** with concrete deliverables, “definition of done,” and the order that minimizes risk while maximizing immediate product value. It assumes you already have the dsrs + Adjutant integration underway (Wave 14 vibe), and treats “paper promises” as backlog items that become code artifacts.

---

# Phase 0 — Autopilot MVP (Ship + stabilize)

### Goal

A rock-solid autonomous coding loop that reliably completes repo tasks with verification, logs trajectories, and has a usable UI/CLI.

### Deliverables

1. **Autopilot Loop v1**

* Plan → Execute → Verify → Iterate → Succeed/Fail/MaxIterations
* Deterministic verification harness config (`cargo test`, `cargo check`, etc.)
* Hard iteration cap + timeouts

2. **Trajectory Logging v1**

* rlog/JSON session format
* Tool calls, diffs, verification outputs, timing, token usage, lane used
* Deterministic session IDs, reproducible timestamps

3. **DSPy Mode v1 (dsrs)**

* `SubtaskPlanningSignature`, `SubtaskExecutionSignature`, `ResultSynthesisSignature`
* Basic metrics: parse/format correctness + minimal quality checks
* TrainingCollector writes dataset.json

4. **Routing v1**

* Complexity / Delegation / RLM-trigger pipelines exist but are conservative
* Confidence-gated override (>0.7) with legacy fallback

5. **UI/CLI v1**

* CLI: run task, show progress, show verification results, export session
* UI: session browser + live log view (even minimal)

### Definition of done

* 30+ real tasks run end-to-end without manual patching of the system
* No silent tool hallucinations (tool middleware enforces real calls)
* Sessions always end in an explicit state with verification history

---

# Phase 1 — Make DSPy a real compiler (Outcome-coupled learning)

### Goal

Stop optimizing for “pretty JSON.” Optimize for “passes tests fast, cheaply, without thrash.”

### Deliverables

1. **Outcome-coupled labels**

* Implement `LabeledToolCall` and compute:

  * `verification_delta` (prev failing − current failing)
  * `was_repeated` (call hash)
  * `step_utility` (simple heuristic v1)
  * `cost_tokens`, `cost_tool_calls`

2. **Decision labels**

* OutcomeFeedback assigns correctness for:

  * complexity
  * delegation
  * rlm_trigger
    (start simple; refine later)

3. **PerformanceTracker v1**

* Rolling accuracy per signature (window=50)
* Track success rate, avg iterations, repetition rate, cost per success

4. **AutoOptimizer v1**

* Trigger rules: min examples, accuracy threshold, min hours since last
* Optimize lowest-accuracy signature with MIPROv2
* Store optimization runs + policy bundle ID

5. **Counterfactual recording v1**

* Always record legacy output alongside DSPy output
* Record whether fallback used and why

### Definition of done

* You can show before/after bundles improving:

  * success rate OR cost per success OR thrash rate
* Optimization produces versioned policy bundles you can pin/rollback

---

# Phase 2 — RLM mode (local, single-machine) integrated with DSPy

### Goal

Autopilot doesn’t collapse on big repos or long sessions.

### Deliverables

1. **RLM Executor v1**

* Root LM + context store + context ops tools:

  * peek(path, range)
  * grep(pattern, scope)
  * summarize(chunks)
  * partition(scope, strategy)
  * map(query, chunks)
* Everything logged as tool calls (for training)

2. **RLM Trigger v2**

* Pipeline uses more signals:

  * estimated token growth
  * file_count
  * repeated actions indicator
* Confidence-gated

3. **Signature integration**

* Planner signature accepts `context_handle` (or equivalent) instead of raw text where possible
* RLM “tool ops” become part of plan/execution

4. **RLM budgets**

* Max recursion depth
* Max subcalls
* Stop criteria

### Definition of done

* A “large repo suite” where RLM mode improves:

  * success rate on high-context tasks
  * or reduces iterations/thrash
* No uncontrolled recursion cost blowups

---

# Phase 3 — Marketplace-ready compute primitives (objective jobs first)

### Goal

Autopilot becomes the first buyer of compute. Providers can earn for verifiable work.

### Deliverables

1. **Protocol job schemas v1**

* `oa.sandbox_run.v1` (objective)
* `oa.repo_index.v1` (objective-ish)
* Deterministic hashing of inputs/outputs
* Provenance fields

2. **NIP-90 wiring v1**

* Job request → provider response → result
* Pay-after-verify flow for objective jobs:

  * verify exit code + artifact hashes
  * only then pay invoice

3. **Pylon provider mode v1**

* Providers advertise capabilities (models/hardware)
* Execute sandbox_run jobs safely
* Rate limits + health checks

4. **Reserve pool**

* Always-available fallback provider (even if internal)
* Prevents dead marketplace UX

### Definition of done

* Autopilot runs tests/builds via marketplace providers with:

  * deterministic verification
  * receipts
  * no payment for incorrect output

---

# Phase 4 — Neobank Treasury OS (budgets, quotes, receipts)

### Goal

Agents can spend money autonomously **without** becoming a liability. Enterprises can reason about spend.

### Deliverables

1. **TreasuryRouter v1**

* Policy decides rail + approvals + caps
* Account partitions: operating / escrow / treasury
* AssetId abstraction (BTC_LN, USD_CASHU(mint), etc.)

2. **Quote state machine**

* CREATED → UNPAID → PENDING → PAID/FAILED/EXPIRED
* Idempotency keys
* Reservation and release

3. **Reconciliation daemon**

* Resolve pending quotes
* Expire reservations
* Repair state after crash

4. **Receipts v1**

* Every payment produces:

  * tx proof (preimage/txid)
  * job hash (if any)
  * session id + policy bundle id
  * policy rule id
* Receipts stored locally and optionally published

### Definition of done

* Autopilot can run with a hard daily cap (USD-denominated) and never exceed it
* Every spend is auditable back to a session + decision

---

# Phase 5 — FRLM federation (swarm + cloud + local recursion)

### Goal

RLM becomes distributed: parallel subqueries and objective jobs purchased across providers.

### Deliverables

1. **FRLM planner**

* When root identifies broad subproblem:

  * partition
  * dispatch map queries as NIP-90 subjective jobs
  * gather results
  * rank + synthesize

2. **Subjective job verification tiers**

* reputation-only
* best-of-N consensus
* judge model (optional)

3. **Market-aware routing policy**

* Delegation pipeline now considers:

  * budget remaining
  * provider reliability
  * expected value of fanout

4. **Cost control**

* Fanout budgets and stop rules
* Per-provider circuit breakers

### Definition of done

* FRLM beats local RLM on “many-file” tasks while keeping spend bounded
* Providers receive payments and reputation updates correctly

---

# Phase 6 — Skills marketplace (attach rate + distillation)

### Goal

Capabilities become composable products. Teacher lanes distill into cheap local policies.

### Deliverables

1. **Skill format v1**

* package: instructions + scripts + metadata
* progressive disclosure to manage context windows

2. **Skill licensing + delivery**

* NIP-SA-ish events or marketplace events
* encrypted delivery, idempotent purchase records

3. **Teacher/student compilation**

* “teacher” runs (Codex/swarm) generate trajectories
* compile student policies for local lanes via dsrs
* ship policy bundle as “skill”

4. **Revenue split + receipts**

* simple splits: creator/provider/platform
* receipts tie skill invocation to payment + session

### Definition of done

* A skill can be purchased and invoked by Autopilot
* The invocation pays creator + provider
* The skill improves success/cost in a measurable way

---

# Phase 7 — Exchange + FX routing (optional until Neobank is solid)

### Goal

Agents can hold USD-denom budgets and pay BTC-only providers by sourcing liquidity.

### Deliverables

* RFQ quoting + settlement receipts
* NIP-native order/reputation model (start v0 reputation-based)
* atomic swap v1 later (Cashu P2PK + HODL invoices)
* Treasury Agents as makers

### Definition of done

* Autopilot can pay a sat invoice with a USD-denom budget through an RFQ swap
* Receipts include quote id + rate source

---

# Phase 8 — Full “Agentic OS” protocolization (NIP-SA, trajectories, coalitions)

### Goal

Standardize lifecycle + portability: agents survive platforms, move between operators, form coalitions.

### Deliverables

* NIP-SA event kinds implemented + published
* coalition primitives (group budgets, multi-party payouts)
* reputation labels and trust graphs
* governance hooks (disputes, arbitration)

### Definition of done

* Another client can reconstruct agent lifecycle from Nostr events
* Agents can coordinate work across machines/owners with auditable trails

---

# Implementation ordering constraints (important)

**Do these early:**

* Outcome-coupled metrics (or you will Goodhart yourself)
* Versioned policy bundles + rollback
* Counterfactual logging (shadow mode)

**Do these later:**

* Exchange/FX (only after TreasuryRouter + quotes + reconciliation are solid)
* Coalitions (only after identity + receipts are stable)

---

# Suggested repo milestones (tight, product-friendly)

### Milestone M1: “Autopilot ships”

* Phase 0 done

### Milestone M2: “Self-improving Autopilot”

* Phase 1 done

### Milestone M3: “Big repo stability”

* Phase 2 done

### Milestone M4: “Autopilot buys verified compute”

* Phase 3 done

### Milestone M5: “Enterprise budgets + receipts”

* Phase 4 done

### Milestone M6: “Federated deep research for code”

* Phase 5 done

### Milestone M7: “Skills economy”

* Phase 6 done

---

If you want this as **a set of GitHub issues / epics**, I can translate each deliverable into:

* issue title
* acceptance criteria
* files to touch (based on your crate layout)
* test plan
* telemetry to add
