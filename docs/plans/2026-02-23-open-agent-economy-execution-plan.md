# OpenAgents Open-Agent Economy Execution Plan

Status: Draft (execution plan)
Date: 2026-02-23
Author: Codex
Primary context sources:
- `docs/local/hehehe.md` (read fully)
- Product thesis from user note (Citrini spiral vs open market rerouting)
- `docs/transcripts/194-the-trillion-dollar-question.md`
- `docs/transcripts/199-introducing-autopilot.md`
- `docs/transcripts/200-the-agent-network.md`
- `docs/transcripts/201-fracking-apple-silicon.md`
- `docs/transcripts/202-recursive-language-models.md`
- `docs/transcripts/203-pylon-and-nexus.md`

## 1) Purpose

This plan translates the macro thesis into a concrete OpenAgents execution program:

- Avoid the closed-balance-sheet AI outcome (margin expansion + payroll compression + weak demand recycle).
- Build open rails where intelligence can hold identity, transact, and form coalitions.
- Make coalition formation and contract execution latency approach zero.
- Route income into machine-speed micro-contract markets with verifiable receipts and automatic settlement.
- Encode default revenue splits so value is paid out to creators/providers/operators, not captured by a single platform balance sheet.
- Minimize coalition latency (discovery + contracting + verification + settlement), treating it as the primary bottleneck rather than model IQ.

## 2) Problem Statement

If OpenAgents only becomes a centralized cost-reduction assistant, it will reinforce the displacement path:

- Lower labor share.
- Higher concentration of gains.
- Lower discretionary demand.
- Macroeconomic fragility in services-heavy households.

OpenAgents must instead become an open economic substrate where:

- Work unbundles into continuous micro-contracts.
- Participation is paid by default.
- Lower execution cost (deflation) and recurring payouts (dividends) offset displacement.

## 3) Strategy Summary (Three-Track)

Track A: Fast PMF wedge (prove real user pain relief)
- Ship one persistent autonomous loop that saves operators measurable weekly time.
- Primary wedge: persistent AI Chief-of-Staff loop (`Inbox -> Decisions -> Actions -> Digest`).

Track B: Protocol/economy substrate (make the wedge evolve into open markets)
- Identity + policy + receipts + workflow + compute contracts + coalition primitives.
- Payments and revenue-split rails that move value to participants continuously.

Track C: Supply-demand flywheel bootstrap (prove market plumbing)
- Supply bootstrapping: convert stranded edge compute into paid, routable provider capacity.
- Demand bootstrapping: route high-fanout workloads (e.g., RLM async subcalls) into that capacity under strict budget controls.

Track A validates demand quickly.
Track B prevents local optimization into a closed assistant product.
Track C prevents the market from becoming "architecture without liquidity."

## 4) Constraints and Invariants

The program must remain aligned with active architecture constraints:

- Proto-first contracts (`INV-01`).
- Authenticated HTTP mutation authority (`INV-02`).
- Khala WS-only live transport (`INV-03`), no new SSE authority lanes.
- Control/runtime authority isolation (`INV-04`, `INV-05`).
- Replay/idempotency guarantees (`INV-07`).
- Rust-first product architecture boundaries (`ADR-0001`, iOS WGPUI authority `INV-11`).
- No `.github` workflow automation in-repo (`INV-12`).

## 4.1) Execution Principles from Transcript Synthesis

1. "Autopilot, not copilot" as product north star:
- Persistent autonomous loops with bounded budgets and explicit escalation.

2. Coalition latency is the battleground:
- Optimize path length and failure rates for discover -> contract -> verify -> settle.

3. Open default, optional containment:
- Open market plane is default.
- Optional containment plane exists for high-risk tools/workloads, with signed artifacts crossing back into open plane.

4. Market plumbing over abstract abundance claims:
- Stranded compute only becomes economic supply when discovery, packaging, trust, settlement, and operability are solved.

5. Revenue sharing defaults, not afterthought:
- Skills, workflows, compute, and verification should all have first-class payout paths.

6. Mech-suit interoperability:
- OpenAgents should orchestrate across best-in-class agents/models, not force monoculture lock-in.

## 5) Target System Shape

OpenAgents evolves into 6 coordinated planes plus an optional containment overlay:

1. Identity + Policy Plane
- Agent and coalition identities.
- Delegation, capability, spend/budget policy.
- Revocation and scope enforcement.

2. Messaging Plane
- Append-only streams with cursors.
- Directed inbox streams and group streams.
- Anti-spam admission using policy + economics.

3. State + Receipt Plane
- Durable event logs.
- Content-addressed artifacts.
- Signed receipts for all contract-critical actions.

4. Compute Market Plane
- Capability ads.
- RFQ/Offer/Order/RunTicket lifecycle.
- Metered execution + settlement.
- Edge provider onboarding path (Pylon-like node software) and relay fabric (Nexus-like market relay).
- Dedicated high-fanout workload lanes for recursive/parallel orchestration patterns.

5. Coalition Plane
- Group identity + membership logs + epochs.
- Role/governance state.
- Group streams (no naive fanout).
- Group treasury and group-authorized spend.

6. Workflow Orchestration Plane (Temporal-like, OpenAgents-native)
- Durable workflow instances backed by event history.
- Deterministic replay.
- Timers, retries, compensation, signals.
- Receipts as workflow evidence.

## 6) Mapping to Existing OpenAgents Surfaces

Control service (`apps/openagents.com/service`):
- Policy authority, auth/session, route controls, web UX entry, control APIs.
- Add economic control APIs, policy schemas, receipt query APIs.

Runtime (`apps/runtime`):
- Run broker, execution lifecycle, WS replay streams, event ingestion.
- Add provider scheduling, metering, run settlement hooks, workflow ticks.

Provider/edge supply (`crates/pylon`):
- Provider mode onboarding, wallet, earnings, job lifecycle, reliability telemetry.
- Expand provider policy, reputation, and operability controls for market-grade uptime.

Relay and market transport (`crates/autopilot/src/app/nexus.rs` + runtime relay lanes):
- High-throughput relay/indexing strategy for market events and async fanout workloads.

Khala sync:
- Continue WS-only delivery for live streams and replay cursors.
- Add group stream cursor semantics and compute lifecycle stream topics.

Clients (web/desktop/iOS):
- Product loops and operator UX.
- Keep product UI/state logic Rust/WGPUI-authoritative where required.

Shared crates/proto:
- Canonical schemas for contracts, receipts, workflows, coalition events.
- Add schemas for provider capability ads, reputation/quality signals, and coalition-latency telemetry.

## 7) Program Phases

## Phase 0: Wedge Validation (0-45 days)

Goal:
- Prove one persistent loop produces measurable operator leverage.

Deliverables:
- Chief-of-Staff autonomous loop with policy guardrails.
- Daily digest + escalation queue.
- Clear metrics: hours saved, tasks closed autonomously, escalation precision.

Issue batch:
- `OA-ECON-001` Define wedge KPI contract and baseline instrumentation.
- `OA-ECON-002` Implement persistent inbox intake and triage state machine.
- `OA-ECON-003` Implement policy-aware action executor (safe auto-actions only).
- `OA-ECON-004` Implement daily digest and escalation reports.
- `OA-ECON-005` Add operator replay/audit UI for all autonomous actions.
- `OA-ECON-006` Add "mech-suit" provider abstraction for multi-agent backends with uniform policy/replay controls.

Exit criteria:
- At least 5 weekly active operators report persistent value.
- Median measured savings >= 5 hrs/week.
- No critical trust failures without replay evidence.
- At least one loop runs overnight unattended for 7 consecutive days with auditable outcomes.

## Phase 1: Contract and Receipt Core (30-90 days)

Goal:
- Standardize contract primitives and machine-verifiable receipts.

Deliverables:
- Canonical proto schemas for RFQ/Offer/Order/RunTicket/Settlement.
- Content-addressed receipt store and query API.
- Idempotent command application and replay tooling.
- Key-split custody and guardian policy for sovereign agent identity operations.
- Capability-bound licensing primitives for paid skill/workflow invocation.

Issue batch:
- `OA-ECON-010` Add proto package for compute market contracts.
- `OA-ECON-011` Add proto package for receipts and evidence bundles.
- `OA-ECON-012` Implement receipt writer/hasher/signature pipeline.
- `OA-ECON-013` Implement receipt query/read APIs and index strategy.
- `OA-ECON-014` Add deterministic replay harness for contract flows.
- `OA-ECON-015` Implement guardian/key-split policy hooks for agent and coalition signing authority.
- `OA-ECON-016` Implement paid capability binding to prevent unauthorized copy/use of licensed skills.

Exit criteria:
- Every contract transition emits signed receipt.
- Replay reproduces state transitions from receipts/history.
- Read APIs support dispute reconstruction end-to-end.
- Sovereign identity policy can delegate, revoke, and rotate signing authority without state-loss.

## Phase 2: Open Compute Market v1 (60-150 days)

Goal:
- Enable agents/users/providers to transact compute with bounded trust.

Deliverables:
- Provider capability advertisement and discovery index.
- RFQ/Offer/Order flow with budget constraints.
- Run ticket issuance, metering, and settlement.
- "Stranded compute to routable supply" bootstrap lane with provider operability guarantees.
- RLM/async fanout demand lane with bounded spend and observability.

Issue batch:
- `OA-ECON-020` Provider capability schema and publish API.
- `OA-ECON-021` RFQ ingest and routing service.
- `OA-ECON-022` Offer lifecycle and quote validity checks.
- `OA-ECON-023` Order authorization with spend-cap policy checks.
- `OA-ECON-024` RunTicket issuance and short-lived auth model.
- `OA-ECON-025` Metering and usage receipt emission.
- `OA-ECON-026` Settlement/refund state machine for failed/partial runs.
- `OA-ECON-027` Implement market plumbing completeness checks: discovery, packaging, trust, settlement, operability.
- `OA-ECON-028` Launch wildcatter provider program: onboarding, reliability scoring, and payout transparency.
- `OA-ECON-029` Implement RLM async fanout workload routing with concurrency and budget controls.

Exit criteria:
- Buyer/provider complete a full paid run with receipts and replay.
- Duplicate and timeout paths are idempotent and compensated.
- Provider churn does not break order safety.
- Edge provider retention and quality metrics show stable liquidity, not one-off demos.

## Phase 3: Coalition Primitives (90-210 days)

Goal:
- Make Reed’s-law group formation safe and cheap.

Deliverables:
- Coalition identity and membership event log.
- Epoch snapshots for authorization checks.
- Group stream with per-member cursors (no per-member fanout writes).
- Role-based coalition policy and spend authority.

Issue batch:
- `OA-ECON-030` Coalition create/join/leave/role-change events.
- `OA-ECON-031` Coalition epoch snapshot generator and cache API.
- `OA-ECON-032` Group stream append/subscribe/cursor semantics.
- `OA-ECON-033` Membership+role authorization middleware for group actions.
- `OA-ECON-034` Coalition treasury policy and spend approval gates.
- `OA-ECON-035` Merge/split workflows and race-condition handling.

Exit criteria:
- Coalition message writes are O(1) append per message.
- Membership churn can be absorbed without auth regressions.
- Group-authorized spend yields verifiable receipts.

## Phase 4: OpenAgents Workflow Engine (Temporal-like) (120-240 days)

Goal:
- Durable orchestration for long-lived multi-step agent/coalition processes.

Deliverables:
- Workflow definition format (deterministic DSL or deterministic WASM subset).
- Workflow instance history streams.
- Orchestrator ticks on new events.
- Timers, retries, compensation, signals.

Issue batch:
- `OA-ECON-040` Define deterministic workflow definition contract.
- `OA-ECON-041` Implement workflow history streams and snapshot state.
- `OA-ECON-042` Implement orchestrator decision loop and activity scheduling.
- `OA-ECON-043` Implement sharded durable timer wheel service.
- `OA-ECON-044` Implement compensation graph execution.
- `OA-ECON-045` Implement access-controlled workflow signals.
- `OA-ECON-046` Add workflow replay verification suite.

Exit criteria:
- Contract workflows survive crashes and resume deterministically.
- Timeout/retry/compensation paths are replay-equivalent.
- Signals can safely alter running workflows under policy.

## Phase 5: Dividend and Split Rails (150-300 days)

Goal:
- Ensure value distribution is encoded by default.

Deliverables:
- Policy-defined revenue split language (creator/provider/operator shares).
- Automated payout scheduling and receipts.
- Royalty attribution for reusable skills/workflows/artifacts.

Issue batch:
- `OA-ECON-050` Split policy schema (default + override rules).
- `OA-ECON-051` Settlement splitter service with deterministic rounding.
- `OA-ECON-052` Skill/workflow royalty attribution registry.
- `OA-ECON-053` Payout ledger and reconciliation reports.
- `OA-ECON-054` Failure handling for partial payout/dispute states.

Exit criteria:
- Completed orders automatically distribute funds by policy.
- Split outcomes are replayable and auditable.
- Payout failure paths preserve conservation invariants.

## Phase 6: Macro Health and Anti-Concentration Controls (180-360 days)

Goal:
- Measure whether system behavior matches the macro thesis.

Deliverables:
- Demand/income velocity telemetry.
- Concentration and participation dashboards.
- Policy controls that prevent default centralization.

Issue batch:
- `OA-ECON-060` Market telemetry schema: volume, velocity, churn, payout breadth.
- `OA-ECON-061` Concentration metrics (top-share, provider diversity, payout Gini).
- `OA-ECON-062` Deflation index for execution categories.
- `OA-ECON-063` Dividend index for participant payout streams.
- `OA-ECON-064` Guardrail policies for anti-concentration routing defaults.
- `OA-ECON-065` Public transparency report pipeline.

Exit criteria:
- We can show trendline evidence of broad payout participation.
- Concentration drift triggers actionable policy changes.
- Market health signals are integrated into product and ops decisions.

## Phase 7: Open Plane + Optional Containment Plane (210-390 days)

Goal:
- Preserve permissionless open-market defaults while enabling optional high-risk containment workflows where economically required.

Deliverables:
- Policy contract that classifies workloads by risk tier.
- Containment workflow adapters for high-risk tool access.
- Signed artifact egress from containment plane back to open plane.

Issue batch:
- `OA-ECON-070` Risk-tier policy schema and enforcement hooks.
- `OA-ECON-071` Containment workflow bridge with explicit IO ports and audit receipts.
- `OA-ECON-072` Insurance/bond/fee primitives for containment-lane economics.

Exit criteria:
- High-risk workflows can be isolated without breaking open-plane market participation.
- Every cross-plane transfer is signed, receipted, and replayable.

## 8) Macro Thesis -> Product Mechanism Mapping

Payroll compression risk -> mechanism:
- Replace one-to-many labor displacement narrative with many-to-many micro-contract market participation.
- Track unique earning participants and payout breadth.

Weak demand recycle risk -> mechanism:
- Increase transaction velocity through low-latency contract/settlement loops.
- Enable machine-speed but human-benefiting payouts.

Hyper-concentration risk -> mechanism:
- Default split policies and open provider discovery.
- Diversity-aware routing and anti-concentration guardrails.

Service-economy fragility risk -> mechanism:
- Build recurring payout streams from compute leasing, verification, royalties, and operation fees.
- Emphasize durable small incomes rather than one-time platform payouts.

## 9) Metrics Framework

User leverage metrics (wedge):
- Median hours saved/week.
- Autonomous loop completion rate.
- Escalation precision/recall.
- Trust incidents per 1k actions.

Market metrics:
- Contract cycle latency (`RFQ -> Settlement`).
- Fill rate for RFQs.
- Provider online reliability.
- Buyer repeat rate.
- Coalition latency (`discover -> contract -> verify -> settle`) p50/p95.
- Async fanout completion rate and cost variance for recursive workloads.

Distribution metrics:
- Unique earners per period.
- Top-1% and top-10% payout share.
- Provider Herfindahl-Hirschman Index (HHI).
- Royalty recipient breadth.
- Wildcatter retention curve and earnings distribution.
- Stranded-to-routable conversion rate (eligible edge devices -> active paid providers).

Macro proxy metrics:
- Dollar/sat velocity through the network.
- Real cost index for common work units.
- Dividend stream stability (variance and continuity).
- Local/edge share of inference workload vs centralized cloud share.

## 10) Security, Abuse, and Reliability Requirements

- Signed envelopes and strict idempotency keys for all contract events.
- Spam resistance for unknown contacts (economic stamps/capability gates).
- Replay-resistant settlement and payout workflows.
- Poison-message quarantine and circuit breakers.
- Domain and coalition-level kill switches.
- Deterministic recovery drills for timer/orchestrator failures.
- Relay-level overload and indexing degradation drills for high-throughput market lanes.

## 11) Sequencing Rules

1. Never build generalized market complexity before proving wedge value.
2. Never ship unreceipted settlement actions.
3. Never add non-WS live authority lanes.
4. Never rely on in-memory-only state for long-lived workflows.
5. Never enable coalition spend without role+epoch authorization checks.
6. Never claim market supply until discovery/packaging/trust/settlement/operability are all instrumented.

## 12) 90-Day Execution Focus

What we should do now:
- Phase 0 complete (wedge PMF evidence).
- Phase 1 started (contract + receipt schemas and store).
- Phase 2 thin-slice started (one provider path, one buyer path, one settlement path).
- Provider-market bootstrap instrumentation shipped (`stranded -> routable` conversion + wildcatter retention).
- RLM/async fanout lane benchmarked with strict budget envelopes.

What we should avoid now:
- Broad coalition merge/split complexity.
- Fully generalized discovery ranking markets.
- Any narrative/feature not improving measurable leverage or verified payouts.
- Mainnet-like growth claims without replayable payout and reliability evidence.

## 13) Immediate Next Issue Pack (ready to open)

- `OA-ECON-001` Wedge KPI and replay evidence contract.
- `OA-ECON-002` Persistent inbox triage state machine.
- `OA-ECON-003` Policy-aware autonomous action executor.
- `OA-ECON-010` Proto contract schemas for compute market lifecycle.
- `OA-ECON-011` Receipt schema + canonical hash/signature rules.
- `OA-ECON-012` Receipt store/query service.
- `OA-ECON-020` Provider capability publish/discovery API.
- `OA-ECON-023` Spend-cap authorization and order issuance workflow.
- `OA-ECON-027` Market plumbing completeness checks.
- `OA-ECON-028` Wildcatter provider program and reliability scoring.
- `OA-ECON-029` RLM async fanout routing with budget controls.

## 14) Final Definition of Success

OpenAgents reflects the thesis when all are true:

- Users rely on persistent autonomous loops for weekly leverage.
- Agents and coalitions can contract compute/services continuously on open rails.
- Settlement emits verifiable receipts and automatic payouts.
- Revenue splits are default behavior, not optional afterthoughts.
- Market participation and payout breadth improve over time, instead of concentrating.
- The system demonstrates both lower execution cost (deflation) and recurring participant income (dividends).
- Coalition latency trends down while trust/safety incident rates remain controlled.
- Edge/local compute participation is measurable, economically real, and widely distributed.

That is the concrete path from architecture to macro relevance.
