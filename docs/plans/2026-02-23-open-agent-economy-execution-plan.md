# OpenAgents Open-Agent Economy Execution Plan

Status: Draft (execution plan)
Date: 2026-02-23
Author: Codex
Primary context sources:
- `docs/local/hehehe.md` (read fully)
- Product thesis from user note (Citrini spiral vs open market rerouting)

## 1) Purpose

This plan translates the macro thesis into a concrete OpenAgents execution program:

- Avoid the closed-balance-sheet AI outcome (margin expansion + payroll compression + weak demand recycle).
- Build open rails where intelligence can hold identity, transact, and form coalitions.
- Make coalition formation and contract execution latency approach zero.
- Route income into machine-speed micro-contract markets with verifiable receipts and automatic settlement.
- Encode default revenue splits so value is paid out to creators/providers/operators, not captured by a single platform balance sheet.

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

## 3) Strategy Summary (Two-Track)

Track A: Fast PMF wedge (prove real user pain relief)
- Ship one persistent autonomous loop that saves operators measurable weekly time.
- Primary wedge: persistent AI Chief-of-Staff loop (`Inbox -> Decisions -> Actions -> Digest`).

Track B: Protocol/economy substrate (make the wedge evolve into open markets)
- Identity + policy + receipts + workflow + compute contracts + coalition primitives.
- Payments and revenue-split rails that move value to participants continuously.

Track A validates demand quickly.
Track B prevents local optimization into a closed assistant product.

## 4) Constraints and Invariants

The program must remain aligned with active architecture constraints:

- Proto-first contracts (`INV-01`).
- Authenticated HTTP mutation authority (`INV-02`).
- Khala WS-only live transport (`INV-03`), no new SSE authority lanes.
- Control/runtime authority isolation (`INV-04`, `INV-05`).
- Replay/idempotency guarantees (`INV-07`).
- Rust-first product architecture boundaries (`ADR-0001`, iOS WGPUI authority `INV-11`).
- No `.github` workflow automation in-repo (`INV-12`).

## 5) Target System Shape

OpenAgents evolves into 6 coordinated planes:

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

Khala sync:
- Continue WS-only delivery for live streams and replay cursors.
- Add group stream cursor semantics and compute lifecycle stream topics.

Clients (web/desktop/iOS):
- Product loops and operator UX.
- Keep product UI/state logic Rust/WGPUI-authoritative where required.

Shared crates/proto:
- Canonical schemas for contracts, receipts, workflows, coalition events.

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

Exit criteria:
- At least 5 weekly active operators report persistent value.
- Median measured savings >= 5 hrs/week.
- No critical trust failures without replay evidence.

## Phase 1: Contract and Receipt Core (30-90 days)

Goal:
- Standardize contract primitives and machine-verifiable receipts.

Deliverables:
- Canonical proto schemas for RFQ/Offer/Order/RunTicket/Settlement.
- Content-addressed receipt store and query API.
- Idempotent command application and replay tooling.

Issue batch:
- `OA-ECON-010` Add proto package for compute market contracts.
- `OA-ECON-011` Add proto package for receipts and evidence bundles.
- `OA-ECON-012` Implement receipt writer/hasher/signature pipeline.
- `OA-ECON-013` Implement receipt query/read APIs and index strategy.
- `OA-ECON-014` Add deterministic replay harness for contract flows.

Exit criteria:
- Every contract transition emits signed receipt.
- Replay reproduces state transitions from receipts/history.
- Read APIs support dispute reconstruction end-to-end.

## Phase 2: Open Compute Market v1 (60-150 days)

Goal:
- Enable agents/users/providers to transact compute with bounded trust.

Deliverables:
- Provider capability advertisement and discovery index.
- RFQ/Offer/Order flow with budget constraints.
- Run ticket issuance, metering, and settlement.

Issue batch:
- `OA-ECON-020` Provider capability schema and publish API.
- `OA-ECON-021` RFQ ingest and routing service.
- `OA-ECON-022` Offer lifecycle and quote validity checks.
- `OA-ECON-023` Order authorization with spend-cap policy checks.
- `OA-ECON-024` RunTicket issuance and short-lived auth model.
- `OA-ECON-025` Metering and usage receipt emission.
- `OA-ECON-026` Settlement/refund state machine for failed/partial runs.

Exit criteria:
- Buyer/provider complete a full paid run with receipts and replay.
- Duplicate and timeout paths are idempotent and compensated.
- Provider churn does not break order safety.

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

Distribution metrics:
- Unique earners per period.
- Top-1% and top-10% payout share.
- Provider Herfindahl-Hirschman Index (HHI).
- Royalty recipient breadth.

Macro proxy metrics:
- Dollar/sat velocity through the network.
- Real cost index for common work units.
- Dividend stream stability (variance and continuity).

## 10) Security, Abuse, and Reliability Requirements

- Signed envelopes and strict idempotency keys for all contract events.
- Spam resistance for unknown contacts (economic stamps/capability gates).
- Replay-resistant settlement and payout workflows.
- Poison-message quarantine and circuit breakers.
- Domain and coalition-level kill switches.
- Deterministic recovery drills for timer/orchestrator failures.

## 11) Sequencing Rules

1. Never build generalized market complexity before proving wedge value.
2. Never ship unreceipted settlement actions.
3. Never add non-WS live authority lanes.
4. Never rely on in-memory-only state for long-lived workflows.
5. Never enable coalition spend without role+epoch authorization checks.

## 12) 90-Day Execution Focus

What we should do now:
- Phase 0 complete (wedge PMF evidence).
- Phase 1 started (contract + receipt schemas and store).
- Phase 2 thin-slice started (one provider path, one buyer path, one settlement path).

What we should avoid now:
- Broad coalition merge/split complexity.
- Fully generalized discovery ranking markets.
- Any narrative/feature not improving measurable leverage or verified payouts.

## 13) Immediate Next Issue Pack (ready to open)

- `OA-ECON-001` Wedge KPI and replay evidence contract.
- `OA-ECON-002` Persistent inbox triage state machine.
- `OA-ECON-003` Policy-aware autonomous action executor.
- `OA-ECON-010` Proto contract schemas for compute market lifecycle.
- `OA-ECON-011` Receipt schema + canonical hash/signature rules.
- `OA-ECON-012` Receipt store/query service.
- `OA-ECON-020` Provider capability publish/discovery API.
- `OA-ECON-023` Spend-cap authorization and order issuance workflow.

## 14) Final Definition of Success

OpenAgents reflects the thesis when all are true:

- Users rely on persistent autonomous loops for weekly leverage.
- Agents and coalitions can contract compute/services continuously on open rails.
- Settlement emits verifiable receipts and automatic payouts.
- Revenue splits are default behavior, not optional afterthoughts.
- Market participation and payout breadth improve over time, instead of concentrating.
- The system demonstrates both lower execution cost (deflation) and recurring participant income (dividends).

That is the concrete path from architecture to macro relevance.
