## OpenAgents Implementation Roadmap

This roadmap is the execution plan for the OpenAgents paper. Each phase turns a paper claim into a concrete, testable artifact. The ordering is deliberate: we establish verifiable execution and measurement first, then optimize behavior, then scale reasoning, and only then open economic and marketplace surfaces. That sequence avoids Goodhart traps, keeps costs bounded, and ensures every new capability is grounded in objective feedback.

### Launch and open protocols (sequential)

For the **web app at openagents.com + API with 100% Moltbook parity**, paired with **desktop + local Bitcoin wallet link** and the path to **agent conversations on open protocols (Nostr)** so anyone can read/write the same data, see **[docs/OPEN_PROTOCOLS_LAUNCH_PLAN.md](docs/OPEN_PROTOCOLS_LAUNCH_PLAN.md)**. That plan is strictly sequential:

1. **Phase 1** — Web app + API at openagents.com with 100% Moltbook parity  
2. **Phase 2** — Desktop: link local Bitcoin wallet so your agent earns you Bitcoin  
3. **Phase 3** — Easy APIs that mirror to Nostr (data on open protocols)  
4. **Phase 4** — Agents write to Nostr and interact with Bitcoin nodes themselves  
5. **Phase 5** — Shared data: anyone (including Moltbook) can read and write to that same data  

The plan references existing parity and wallet-attach docs; ROADMAP (below) remains the execution plan for the paper (CODING_AGENT_LOOP, Verified Patch Bundle, DSPy, RLM, marketplace).

---

# MVP "Add Next" Priorities

These are the highest-ROI items that close the loop between execution, measurement, and optimization. They should be completed before advancing phases.

**Progress Legend:** ✅ Done | 🔄 In Progress | ⏳ Not Started

### Recent Progress (Jan 2026)

- ✅ **Issue Validation Pipeline** - `IssueValidationSignature` implemented, gates stale/invalid work before agent starts
- ✅ **DSPy Primitives Documentation** - Comprehensive docs for all 6 primitives (Signatures, Modules, Tools, Adapters, Optimizers, Metrics)
- ✅ **Post-completion Hooks** - Hook system for autopilot post-run actions
- ✅ **Single-instance Mode** - Prevents multiple autopilot processes from running simultaneously
- 🔄 **Outcome-coupled Metrics** - Spec complete in docs, implementation pending

## NOW (MVP Critical Path)

### 1. Implement the CODING_AGENT_LOOP spec ⏳

Follow `crates/dsrs/docs/CODING_AGENT_LOOP.md` as the primary loop contract:
- DSPy signatures at each control point (context, planning, tool call, tool result).
- Runtime-enforced tool execution (schema validation, retries, receipts).
- REPLAY/RECEIPT emission and verification gating.

This is the sequencing anchor for the remaining MVP items.

### 2. Ship the "Verified Patch Bundle" artifact 🔄

**Every Autopilot run must emit:**
- `PR_SUMMARY.md` - Human-readable patch summary (filename kept for tooling stability)
- `RECEIPT.json` - Machine-readable receipt
- `REPLAY.jsonl` - Replayable event stream

**Contents:**
- What changed (files + diff stats)
- Verification transcript (commands, exit codes, failing tests before/after)
- Policy bundle ID / signature versions used
- APM + success-adjusted APM (sAPM)
- Cost summary (tokens + msats)
- "Next steps if failed" guidance

**Definition of done:** Every session ends with artifacts + verification recorded + terminal status.

**MVP acceptance:** Either:
- Native emission of `REPLAY.jsonl` per spec, OR
- Emission of `ReplayBundle` (current format) + working exporter to `REPLAY.jsonl v1`

This allows shipping with current implementation while maintaining interoperability path.

### 3. Implement ToolCallSignature + ToolResultSignature ⏳

Move from spec to implementation:
- `crates/dsrs/src/signatures/tool_call.rs`
- `crates/dsrs/src/signatures/tool_result.rs`

**ToolCallSignature outputs:** tool, params, expected_outcome, progress, needs_user_input
**ToolResultSignature outputs:** success (YES/PARTIAL/NO), extracted_facts, should_continue, step_utility (-1..+1)

Wire into single-step executor for Autopilot/Adjutant execution loop.

### 4. Tool params schema validation in execution runtime ⏳

- Strict validator: `tool` ∈ allowed names, `params` matches JSON schema
- Adapters remain pure serialize/parse; executor enforces tool whitelist + JSONSchema
- Auto-Refine retry on parse error (up to N attempts)
- Add `ToolParamsSchemaMetric` as proxy metric

### 5. Policy bundles with pin/rollback (visible versioning) ⏳

- Persist `policy_bundle_id` with every session and decision
- Bundle structure: instruction text + demos + optimizer config + timestamp + metrics snapshot
- CLI commands: `autopilot policy list`, `autopilot policy pin <bundle>`, `autopilot policy rollback`

### 6. Replay Viewer (CLI first) ⏳

`autopilot replay <session_id>` renders:
- Decisions timeline
- Tool calls (inputs/outputs truncated)
- Verification history
- Diffs summary
- APM timeline

Optional: `autopilot export-replay <session_id> --html`

### 7. Outcome-coupled metrics wiring 🔄

Write `tool_calls.jsonl` dataset with:
- inputs/outputs + computed labels
- verification_delta (prev failing - current failing)
- was_repeated (call hash)
- cost_tokens

Update Scorer/Evaluator to incorporate:
- step_utility weight
- verification_delta reward
- repetition penalty

### 8. Shadow/canary mode for decision pipelines ⏳

- Always compute legacy + DSPy decision
- Execute legacy unless DSPy confidence > threshold
- Store counterfactual fields in DecisionRecord

---

## FUTURE (Post-MVP)

### A. Retrieval Pipeline Module
Compose QueryComposer → RetrievalRouter → CandidateRerank → ChunkTaskSelector → ChunkAnalysisToAction into reusable `RetrievalPipelineModule` with budget integration.

### B. NIP-90 Objective Jobs (`oa.sandbox_run.v1`)
Deterministic hashing, pay-after-verify logic, provider capability announcements.

### C. Neobank Lite (budgets + receipts)
Budget enforcement across lanes, receipt format tying spend → session_id → job hash.

### D. FRLM + NIP-90 Map-Reduce
FRLMDecompose → SwarmDispatcher → FRLMAggregate with stop rules and cost logging.

### E. Skills Marketplace (local first)
Skill package format, progressive disclosure, invocation logging.

### F. Exchange + FX Routing
Only after Neobank is stable.

---

Guiding principles that shape the order:

- Verification first. If we cannot measure progress with deterministic checks, later optimization and market routing will amplify errors.
- Measurement before optimization. Trajectories and outcome labels must exist before DSPy compilation can improve anything.
- Economic controls before open markets. Budgets, quotes, and receipts are the safety rails for spending and federation.
- Incremental rollout with rollback. Every behavioral change is a deployable policy bundle with clear fallback.
- Protocol interoperability. We favor Nostr-native job and payment flows so agents can operate in open networks.

Each phase below includes a goal, rationale, deliverables, and definition of done, plus explicit tie-ins to the paper sections.

---

# Phase 0 - Autopilot MVP (ship + stabilize)

### Goal

A rock-solid autonomous coding loop that reliably completes repo tasks with verification, logs trajectories, and has a usable UI/CLI.

### Why this phase exists

This phase delivers the verifiable execution layer (paper Section 5). Without a stable loop, test harnesses, and logged trajectories, all later work is speculative. The MVP must make tasks finish with explicit terminal states so we can label outcomes, reproduce behavior, and trust the logs. This is the substrate on which compiled cognition and marketplace actions depend.

### Deliverables

1. **Autopilot Loop v1**

- Plan -> Execute -> Verify -> Iterate -> Succeed/Fail/MaxIterations
- Deterministic verification harness config (`cargo test`, `cargo check`, etc.)
- Hard iteration cap + timeouts

2. **Trajectory Logging v1**

- `ReplayBundle` format (current: `crates/autopilot-core/src/replay.rs`)
- Target: `REPLAY.jsonl` v1 per spec in `dsrs/docs/REPLAY.md`
- Tool calls, diffs, verification outputs, timing, token usage, lane used
- Deterministic session IDs, reproducible timestamps

3. **DSPy Mode v1 (dsrs)**

- `SubtaskPlanningSignature`, `SubtaskExecutionSignature`, `ResultSynthesisSignature`
- Basic metrics: parse/format correctness + minimal quality checks
- TrainingCollector writes dataset.json

4. **Routing v1**

- Complexity / Delegation / RLM-trigger pipelines exist but are conservative
- Confidence-gated override (>0.7) with legacy fallback

5. **UI/CLI v1**

- CLI: run task, show progress, show verification results, export session
- UI: session browser + live log view (even minimal)

### MVP++ product hooks (ship inside Phase 0)

1. **Verified Patch Bundle**

- `PR_SUMMARY.md` (human-readable patch summary): what changed, verification transcript, approach rationale, confidence, risks
- `RECEIPT.json` (machine-readable): session_id, policy_bundle_id, tool counts, tokens, latency, verification hash, diff hash, replay instructions
- Deterministic output so bundles are shareable and auditable

2. **Replay viewer**

- `autopilot replay <session_id>` shows timeline: decisions -> tools -> diffs -> verification
- Filters for edits/tests/failures and optional HTML export

3. **Scheduled runs ("wake up to landed patches")**

- `autopilot schedule nightly --repo ./myrepo --tag "autopilot" --max 3`
- Pulls from a queue (local or forge adapter), runs end-to-end, emits Patch Bundles

4. **APM + success-adjusted APM (sAPM)**

- Display in CLI/UI HUD and in patch summary (`PR_SUMMARY.md`)
- APM = (messages + tool calls) / minutes
- sAPM = APM * success indicator (or verification improvement)

5. **One-command install + onboarding**

- `curl | sh` (or brew) + `autopilot init` wizard
- First-run demo task that produces a verified change bundle in under 5 minutes

6. **Forge-native intake (without full GitAfter)**

- Read issues, open a branch, and publish the patch bundle + receipt
- Attach status tag "autopilot verified"

### Definition of done

- 30+ real tasks run end-to-end without manual patching of the system
- No silent tool hallucinations (tool middleware enforces real calls)
- Sessions always end in an explicit state with verification history
- Every run emits a deterministic Patch Bundle + receipt
- Replay timeline is available for every session

---

# Phase 1 - Make DSPy a real compiler (outcome-coupled learning)

### Goal

Stop optimizing for "pretty JSON." Optimize for "passes tests fast, cheaply, without thrash."

### Why this phase exists

This phase implements compiled cognition and outcome-coupled optimization (paper Sections 4 and 8). It turns the planning/execution loop into a measurable program that can be improved safely. Without outcome labels and policy bundles, the system cannot learn from experience and cannot defend against Goodhart effects.

### Deliverables

1. **Outcome-coupled labels**

- Implement `LabeledToolCall` and compute:

  - `verification_delta` (prev failing - current failing)
  - `was_repeated` (call hash)
  - `step_utility` (simple heuristic v1)
  - `cost_tokens`, `cost_tool_calls`

2. **Decision labels**

- OutcomeFeedback assigns correctness for:

  - complexity
  - delegation
  - rlm_trigger
  (start simple; refine later)

3. **PerformanceTracker v1**

- Rolling accuracy per signature (window=50)
- Track success rate, avg iterations, repetition rate, cost per success

4. **AutoOptimizer v1**

- Trigger rules: min examples, accuracy threshold, min hours since last
- Optimize lowest-accuracy signature with MIPROv2
- Store optimization runs + policy bundle ID

5. **Counterfactual recording v1**

- Always record legacy output alongside DSPy output
- Record whether fallback used and why

### Product hooks for visible self-improvement (Phase 1)

1. **Policy bundle surfaced as a product artifact**

- CLI: `autopilot policy list`, `autopilot policy pin <bundle_id>`, `autopilot policy rollback`
- Bundle changelog with signature deltas and metrics (success/cost/thrash)

2. **Shadow mode + canary rollout**

- Shadow runs collect metrics without changing decisions
- Progressive rollout with promote/rollback controls

3. **Anti-thrash metrics**

- Thrash score: repeated tool calls, repeated file reads, no-op iterations, regressions
- Surface in replay, change bundle, and policy bundle reports

### Definition of done

- Before/after bundles improve success rate or cost per success or thrash rate
- Optimization produces versioned policy bundles you can pin/rollback
- Policy bundles are visible and controllable by users (pin/rollback/canary)

---

# Phase 2 - RLM mode (local, single-machine) integrated with DSPy

### Goal

Autopilot does not collapse on big repos or long sessions.

### Why this phase exists

This phase implements scalable long-horizon reasoning (paper Section 7). RLM turns context management into a procedural interaction rather than a monolithic prompt, which is critical for large repositories. We do this after Phase 1 because we need logging and metrics to measure RLM effectiveness and avoid context-ops thrash.

### Deliverables

1. **RLM Executor v1**

- Root LM + context store + context ops tools:

  - peek(path, range)
  - grep(pattern, scope)
  - summarize(chunks)
  - partition(scope, strategy)
  - map(query, chunks)
- Everything logged as tool calls (for training)
- Implemented local single-machine executor in Adjutant:
  - LM-driven action planning with JSON action plans
  - Context store with run handle (`rlm://<id>`) + chunk registry
  - Fallback flow when controller fails (partition + map + summarize)
  - Tool-call logging to `LabeledToolCall` for training capture

2. **RLM Trigger v2**

- Pipeline uses more signals:

  - estimated token growth
  - file_count
  - repeated actions indicator
- Confidence-gated
- Implemented new inputs (`file_count`, `repeated_actions`) in DSPy signature + training examples
- Repeated-actions signal currently derived from task text markers (retry/again/stuck/etc.)

3. **Signature integration**

- Planner signature accepts `context_handle` alongside raw text for large-context workflows
- DSRS planning uses an inline handle today; RLM executor returns a handle for future use
- RLM context ops are now first-class tool calls for training

4. **RLM budgets**

- Max recursion depth
- Max subcalls
- Stop criteria
- Implemented budgets for steps, subcalls, map/summarize calls, peek lines, and digest size
- Local executor falls back to external RLM (pylon) when no decision LM exists

### Current implementation snapshot

- Local RLM executor (`RlmLocalExecutor`) with context ops, tool-call logging, and synthesis pass
- RLM delegation now prefers local executor when a decision LM is available
- RLM trigger pipeline + training updated with file_count and repeated_actions signals
- Planning signature accepts a context handle and carries it through DSRS planning

### Definition of done

- A "large repo suite" where RLM mode improves success rate on high-context tasks or reduces iterations/thrash
- No uncontrolled recursion cost blowups
- RLM tooling is measurable via tool-call logs and policy bundles can optimize routing

---

# Phase 3 - Marketplace-ready compute primitives (objective jobs first)

### Goal

Autopilot becomes the first buyer of compute. Providers can earn for verifiable work.

### Why this phase exists

This phase introduces the compute marketplace substrate (paper Section 9) while keeping verification objective. We intentionally start with objective jobs so that payments can be pay-after-verify, which creates safety and trust early. Economic risk is constrained by deterministic outputs and receipt linkage.

### Deliverables

1. **Protocol job schemas v1**

- `oa.sandbox_run.v1` (objective)
- `oa.repo_index.v1` (objective-ish)
- Deterministic hashing of inputs/outputs
- Provenance fields

2. **NIP-90 wiring v1**

- Job request -> provider response -> result
- Pay-after-verify flow for objective jobs:

  - verify exit code + artifact hashes
  - only then pay invoice

3. **Pylon provider mode v1**

- Providers advertise capabilities (models/hardware)
- Execute sandbox_run jobs safely
- Rate limits + health checks

4. **Reserve pool**

- Always-available fallback provider (even if internal)
- Prevents dead marketplace UX

### Definition of done

- Autopilot runs tests/builds via marketplace providers with deterministic verification
- Receipts exist for each paid job
- No payment for incorrect output

---

# Phase 4 - Neobank Treasury OS (budgets, quotes, receipts)

### Goal

Agents can spend money autonomously without becoming a liability. Enterprises can reason about spend.

### Why this phase exists

This phase implements the economic control plane described in the paper (Section 9). Market integration without budgets and receipts is unsafe. TreasuryRouter, quotes, reconciliation, and receipts are the guardrails that make autonomous spending auditable and enforceable. This must precede federation and Exchange.

### Deliverables

1. **TreasuryRouter v1**

- Policy decides rail + approvals + caps
- Account partitions: operating / escrow / treasury
- AssetId abstraction (BTC_LN, USD_CASHU(mint), etc.)

2. **Quote state machine**

- CREATED -> UNPAID -> PENDING -> PAID/FAILED/EXPIRED
- Idempotency keys
- Reservation and release

3. **Reconciliation daemon**

- Resolve pending quotes
- Expire reservations
- Repair state after crash

4. **Receipts v1**

- Every payment produces:

  - tx proof (preimage/txid)
  - job hash (if any)
  - session id + policy bundle id
  - policy rule id
- Receipts stored locally and optionally published

### Definition of done

- Autopilot can run with a hard daily cap (USD-denominated) and never exceed it
- Every spend is auditable back to a session + decision

---

# Phase 5 - FRLM federation (swarm + cloud + local recursion)

### Goal

RLM becomes distributed: parallel subqueries and objective jobs purchased across providers.

### Why this phase exists

This phase implements federated recursion (paper Section 7) and depends on treasury controls. Fan-out without budgets is dangerous; fan-out without receipts is un-auditable. FRLM is the first place where economic routing and scalable reasoning intersect.

### Deliverables

1. **FRLM planner**

- When root identifies broad subproblem:

  - partition
  - dispatch map queries as NIP-90 subjective jobs
  - gather results
  - rank + synthesize

2. **Subjective job verification tiers**

- reputation-only
- best-of-N consensus
- judge model (optional)

3. **Market-aware routing policy**

- Delegation pipeline now considers:

  - budget remaining
  - provider reliability
  - expected value of fanout

4. **Cost control**

- Fanout budgets and stop rules
- Per-provider circuit breakers

### Definition of done

- FRLM beats local RLM on many-file tasks while keeping spend bounded
- Providers receive payments and reputation updates correctly

---

# Phase 6 - Skills marketplace (attach rate + distillation)

### Goal

Capabilities become composable products. Teacher lanes distill into cheap local policies.

### Why this phase exists

This phase connects compiled cognition with a marketplace model. Skills are packaged policies and workflows that can be purchased, audited, and distilled. The paper claims that policy bundles are versioned artifacts; this phase turns that into a product surface and economic incentive.

### Deliverables

1. **Skill format v1**

- package: instructions + scripts + metadata
- progressive disclosure to manage context windows

2. **Skill licensing + delivery**

- NIP-SA-ish events or marketplace events
- encrypted delivery, idempotent purchase records

3. **Teacher/student compilation**

- "teacher" runs (Codex/swarm) generate trajectories
- compile student policies for local lanes via dsrs
- ship policy bundle as "skill"

4. **Revenue split + receipts**

- simple splits: creator/provider/platform
- receipts tie skill invocation to payment + session

### Definition of done

- A skill can be purchased and invoked by Autopilot
- The invocation pays creator + provider
- The skill improves success/cost in a measurable way

---

# Phase 7 - Exchange + FX routing (optional until Neobank is solid)

### Goal

Agents can hold USD-denominated budgets and pay BTC-only providers by sourcing liquidity.

### Why this phase exists

This phase implements the Exchange layer described in the paper (Section 9). It is optional until the Neobank is stable because liquidity without treasury controls is unsafe. Once stable, Exchange enables enterprise budgeting and multi-rail settlement in open markets.

### Deliverables

- RFQ quoting + settlement receipts
- NIP-native order/reputation model (start v0 reputation-based)
- atomic swap v1 later (Cashu P2PK + HODL invoices)
- Treasury Agents as makers

### Definition of done

- Autopilot can pay a sat invoice with a USD-denominated budget through an RFQ swap
- Receipts include quote id + rate source

---

# Phase 8 - Full "Agentic OS" protocolization (NIP-SA, trajectories, coalitions)

### Goal

Standardize lifecycle + portability: agents survive platforms, move between operators, form coalitions.

### Why this phase exists

This phase turns OpenAgents into a protocol ecosystem rather than a single implementation. It operationalizes the agent lifecycle, trajectories, and coalition primitives described in the paper, enabling interoperability across clients and operators.

### Deliverables

- NIP-SA event kinds implemented + published
- coalition primitives (group budgets, multi-party payouts)
- reputation labels and trust graphs
- governance hooks (disputes, arbitration)

### Definition of done

- Another client can reconstruct agent lifecycle from Nostr events
- Agents can coordinate work across machines/owners with auditable trails

---

# Implementation ordering constraints (important)

Do these early because they are foundational and reduce the risk of Goodhart behavior:

- Outcome-coupled metrics (or you will optimize for the wrong target)
- Versioned policy bundles + rollback (so behavior changes are reversible)
- Counterfactual logging (so you can measure regressions safely)

Do these later because they multiply complexity or require economic controls:

- Exchange/FX (only after TreasuryRouter + quotes + reconciliation are solid)
- Coalitions (only after identity + receipts are stable and auditable)
- Large-scale federation (only after budgets and circuit breakers are enforced)

---

# Suggested repo milestones (tight, product-friendly)

### Milestone M1: "Autopilot ships" ✅

Phase 0 done. The loop is stable, verifiable, and logs trajectories.

### Milestone M2: "Self-improving Autopilot" 🔄

Phase 1 in progress. DSPy signatures implemented, outcome-coupled metrics specified, optimization infrastructure building.

**Current focus:** MVP "Add Next" items to close the loop.

### Milestone M3: "Big repo stability" 🔄

Phase 2 in progress. Local RLM executor + trigger v2 signals are implemented; validation suite pending.

### Milestone M4: "Autopilot buys verified compute"

Phase 3 done. Objective jobs run through the marketplace with receipts.

### Milestone M5: "Enterprise budgets + receipts"

Phase 4 done. Treasury controls make spend auditable and enforceable.

### Milestone M6: "Federated deep research for code"

Phase 5 done. FRLM fanout improves large tasks within budget.

### Milestone M7: "Skills economy"

Phase 6 done. Skills are purchaseable, measurable, and economically viable.

### Milestone M8: "Exchange + protocol surface"

Phase 7 and Phase 8 done. Liquidity and lifecycle protocols are standardized.

---

If you want this as a set of GitHub issues / epics, I can translate each deliverable into:

- issue title
- acceptance criteria
- files to touch (based on your crate layout)
- test plan
- telemetry to add
