# Autopilot Earn × Economy Kernel Gap Audit (Docs-Driven)

Date: 2026-03-05  
Author: Codex  
Status: Complete (docs-driven; implementation evidence sourced from existing trackers/audits)

## Objective

Audit what is implemented today for Autopilot Earn (compute-provider MVP + associated earnings automation surface) and what else must exist to make those Earn flows **Economy Kernel compliant**, focusing only on the kernel components that intersect Earn.

This is not a full Hydra/Aegis build plan. It is a kernel-alignment audit: receipts, determinism, policy binding, observability, and settlement truth.

## Sources Reviewed (Docs)

Earn/MVP product authority:
- `docs/MVP.md`
- `docs/EARN.md`
- `docs/AUTOPILOT_EARN_MVP.md`

Canonical Earn implementation status + evidence:
- `docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md` (canonical status authority)
- `docs/AUTOPILOT_EARN_MVP_IMPLEMENTATION_LOG.md` (historical evidence appendix)
- `docs/AUTOPILOT_EARN_MVP_TEST_HARNESS.md`
- `docs/AUTOPILOT_EARN_RECIPROCAL_LOOP_RUNBOOK.md`
- `docs/AUTOPILOT_EARNINGS_AUTOMATION.md`
- `docs/AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md`
- `docs/AUTOPILOT_EARNINGS_ROLLOUT_PLAN.md`

Authority + sync posture:
- `docs/adr/ADR-0001-spacetime-domain-authority-matrix.md`
- `docs/adr/ADR-0002-provider-presence-cardinality-and-ttl-policy.md`
- `docs/PANES.md`

Economy Kernel normative targets (this repo’s kernel spec/proto plan):
- `docs/plans/economy-kernel.md`
- `docs/plans/economy-kernel-proto.md`

## Earn MVP Loop (What Must Be True)

From `docs/MVP.md` and `docs/AUTOPILOT_EARN_MVP.md`, the Earn MVP “proof loop” is:

1. User presses `Go Online`.
2. Desktop receives at least one paid NIP-90 job and executes it.
3. Desktop publishes feedback/result events (NIP-90).
4. Wallet receives sats and the UI only credits earnings once wallet evidence exists.
5. User can withdraw by paying a Lightning invoice.

This audit treats that loop as “product correctness,” and Economy Kernel alignment as “economic correctness under automation.”

## Economy Kernel Intersection Map (Earn-Relevant Subset)

The Economy Kernel spec is broader than Earn MVP. The Earn intersection is the subset below.

### 1) Kernel invariants that Earn depends on

1. Authority boundaries:
   - Earn must keep wallet/settlement truth command-authoritative, not projection-authoritative.
   - Spacetime may be authoritative for presence/projection domains only (ADR-0001).

2. Determinism + replay safety:
   - Earn state must be replay-safe, idempotent, and stable across restarts/disconnects.

3. Receipts as truth:
   - Earn must have append-only receipts that make it possible to audit job lifecycle + settlement from durable evidence, not private logs.

4. Policy-bounded execution:
   - Earn automation (seed demand, scheduler, swaps, withdrawals) must execute under explicit policy with explainable reasons.

### 2) Kernel objects that Earn implicitly implements today

Economy-kernel objects and their Earn analogs:
- WorkUnit: a NIP-90 request/job.
- Contract: the “job lifecycle + payment terms” (often: bid/amount + invoice/payment pointer + result correlation).
- Intent: settlement intents (withdrawal payment, swap quote/execute, starter-demand dispatch).
- Receipt: job history rows + wallet evidence + run-audit receipts (current system terminology), but not yet aligned to economy-kernel receipt envelope.

### 3) Kernel modules that Earn uses (directly or implicitly)

- Settlement Engine:
  - Spark wallet receive/send is the custody boundary and settlement proof source.
  - Swap quote→execute primitives (Blink) are settlement-like actions in automation flows.
- Observability (`/stats`-like contract):
  - Mission Control network stats and public stats expectations intersect the kernel `/stats` contract.
- Policy + breakers:
  - seed-demand caps, kill switches, rollout gating, and failure taxonomy map directly onto kernel policy/withhold/breaker semantics.

Not required for Earn MVP:
- credit envelopes (CEP), bonds/collateral (ABP), warranties/claims, FX RFQ, solver routing.
These become relevant for scaling beyond the compute-provider MVP lane.

## What Is Implemented Today (Per Docs)

This section is “what we can claim is implemented” based on the repo’s canonical status/evidence docs (not a fresh code audit in this pass).

### 1) Earn MVP loop completion is claimed as implemented

`docs/AUTOPILOT_EARN_MVP_EPIC_TRACKER.md` asserts all acceptance gates are implemented and closed:
- Mission Control first-run loop
- Relay-backed NIP-90 provider runtime (request/result/feedback)
- Wallet-confirmed payout gate
- Seed-demand buyer lane with controls
- Public stats lane consistent with wallet-confirmed payouts
- Reliability/test/rollout gates green

### 2) Wallet-authoritative payout truth model is established

Docs are consistent that payout truth comes from:
- Spark wallet receive history evidence, and
- reconciliation rules that reject synthetic pointers.

Evidence surfaces:
- `docs/EARN.md` (“authoritative sources” section)
- `docs/AUTOPILOT_EARN_MVP_TEST_HARNESS.md` (wallet-confirmed payout gating harness)
- `docs/AUTOPILOT_EARNINGS_OPERATOR_RUNBOOK.md` (false-success incident definition + containment)

### 3) Deterministic provider state machine exists (Earn-specific)

Panes and harness docs describe an explicit Earn state machine:
- Go Online: `offline → connecting → online → degraded`
- Job lifecycle: `received → accepted → running → delivered → paid`
- Deterministic replay-safe apply + duplicate suppression and crash recovery are explicitly called out as implemented and tested.

Evidence surfaces:
- `docs/PANES.md` (pane contracts and explicit state machines)
- `docs/AUTOPILOT_EARN_MVP_TEST_HARNESS.md` (replay-safe apply, duplicate suppression, crash recovery, relay loss recovery harnesses)

### 4) Presence/online counters are treated as authority-scoped

Spacetime authority is explicitly scoped and presence semantics are pinned:
- `providers_online` meaning is identity-cardinality with deterministic TTL policy (ADR-0002).
- money/policy authority remains command-authoritative (ADR-0001).

## Economy Kernel Gaps (Earn-Relevant)

Even if the Earn MVP loop is complete, Earn is not yet “Economy Kernel compliant” unless it satisfies the kernel’s **receipt/policy/observability** contracts.

The gaps below are the last-mile items that prevent “two compliant implementations disagree” at the kernel layer.

### P0 Gaps (Kernel correctness / auditability)

1. **Kernel receipt envelope is not yet the Earn receipt format.**
   - Economy Kernel requires deterministic receipts with canonical hashing, idempotency linkage, policy context, and evidence refs (`docs/plans/economy-kernel.md`, §5).
   - Earn docs describe “history receipts,” “run audit receipts,” and “wallet evidence,” but do not specify that these are encoded as economy-kernel receipts with `canonical_hash`, `inputs_hash`, `outputs_hash`, and hash-bound decision fields.
   - Result: the Earn loop can be correct, but it is not yet a portable kernel receipt stream.

2. **Cross-receipt linkage (job → settlement → wallet proof) is not specified as a navigable graph.**
   - Kernel requires transitive navigability: settlement receipts must link to the governing job/contract/verdict/evidence (`economy-kernel.md`, §5.3).
   - Earn docs describe correlation by request/result/payment pointer, but the normative linkage checklist is not asserted as the Earn implementation contract.

3. **Policy binding is present operationally, but not described as PolicyBundle+version on every authority action.**
   - Kernel invariant: every authority mutation executes under explicit policy and is explainable (`economy-kernel.md`, §1.3).
   - Earn has rollout config, kill switches, and budgets, but the docs do not state that each authority effect (starter dispatch, publish result, swap execute, withdraw send) is stamped with `policy_bundle_id` + `policy_version` and recorded in receipts.

4. **/stats snapshot-as-object is not specified for Earn’s public stats lane.**
   - Kernel `/stats` requires: minute snapshots, receipt-derived provenance, deterministic boundaries, snapshot id/hash binding in gating decisions (`economy-kernel.md`, §7).
   - Earn docs call for public beacon stats and in-app stats hydration, but do not assert the full kernel snapshot contract for Earn metrics.

### P1 Gaps (Interoperability + anti-footgun alignment)

1. **WorkUnit metadata mapping for Earn jobs is not pinned to the kernel schema.**
   - Kernel requires `category`, `tfb`, `severity`, `B` (verification budget hint) for WorkUnits (§2.2).
   - Earn jobs likely have implicit category via NIP-90 kind and pricing via sats, but the mapping is not declared normatively (and thus is drift-prone).

2. **Reason-code stability is implied, not explicitly standardized for Earn failure taxonomy.**
   - Kernel now requires stable, versioned reason codes that are hash-bound and aggregate-friendly (§5.9 + hashed-decision requirement).
   - Earn docs mention “reason codes” and a failure taxonomy class, but do not pin a stable code set as a shared contract across UI, receipts, and `/stats`.

3. **Public stats redaction needs to be explicitly asserted for Earn stats.**
   - Kernel `/stats` must not leak payment proofs/invoices/private evidence (`economy-kernel.md`, §7.4).
   - Earn’s “public beacon” framing makes this easy to get wrong unless explicitly enforced (publish aggregates only).

### P2 Gaps (Needed for scaling Earn beyond seeded MVP, but not required for MVP)

1. **Buyer-side bounded spend via envelopes (CEP / NIP-AC) is not integrated into the Earn marketplace loop.**
   - Today’s Earn loop can run with direct wallet payments and seed demand.
   - For non-seeded open-network reliability, buyers will want bounded spend controls that don’t require pre-funding each invoice manually.
   - That’s kernel CEP territory and should be treated as a post-MVP scaling requirement, not a launch blocker.

2. **Liability/claims/warranties and verification tiers are not part of Earn MVP.**
   - Correct for MVP.
   - Required only once the marketplace sells higher-stakes outcomes where refunds/disputes/warranties become necessary.

## Minimum Kernel Work Needed To Support Earn MVP (Actionable Checklist)

If the goal is: “Earn MVP exists and remains correct as the economy kernel becomes real,” the minimum kernel work to connect is:

1. Define Earn receipts as economy-kernel receipts:
   - job lifecycle receipts (accepted/running/delivered/paid/failed),
   - settlement receipts (withdraw send, swap quote/execute if in Earn automation),
   - wallet receive proofs as settlement evidence refs.

2. Enforce cross-receipt linkage:
   - every “paid” history row must reference the wallet proof receipt/evidence,
   - every stats row must be derivable from receipts/snapshots,
   - any correction must be a new receipt (append-only).

3. Make policy binding explicit:
   - policy bundle id + version stamped on all authority effects,
   - reason_code must be stable and hash-bound for denials/withholds/failures.

4. Make Earn public stats conform to kernel `/stats`:
   - minute snapshot object (id/hash),
   - deterministic UTC minute boundary,
   - provenance from receipts,
   - redaction/aggregation rules to avoid leaking secrets.

## Recommendation

Treat Autopilot Earn as the first “real kernel consumer,” but keep the MVP loop intact:
- Do not bolt on full CEP/ABP/Aegis for compute-provider MVP.
- Do harden the Earn receipt/policy/snapshot contracts so future kernel extraction does not require rewriting Earn state.

