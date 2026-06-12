# Presence/Compute Receipt Split with Sybil-Priced Presence Floor (Pluralis Roadmap P2.3)

Date: 2026-06-12
Issue of record: openagents#4854 (master tracking issue openagents#4855)
Rails: #4674, #4676, #4681 (device-benchmark instrument)
Roadmap source: `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`
(workspace `openagents` repo), item P2.3
Cap rationale source: `docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md`
§3.5 (floors keep a fleet alive and enrolled, not rich)

## The two tiers

Pluralis splits incentives into **presence points** (earned during Sync
Phase 1 — being present, synced, and probed) and **compute points**
(earned during Phase 2 — contributing samples that count toward
`target_batch_size`). We adopt that shape with the critical difference
made explicit: Pluralis points are unverified leaderboard decoration;
our presence receipts settle MONEY. Anything that settles money rides a
verification class, so the presence tier carries three hard properties
that decoration never needed:

1. **Bounded.** Presence pay is a capped availability floor. Per the
   buildout plan §3.5, floors are not for getting rich per hour; they
   keep a fleet alive, enrolled, and warm between higher-margin
   assignments, and generate the continuous receipt stream that makes
   the fleet underwritable.
2. **Verified.** Liveness/qualification probes (the #4681
   device-benchmark instrument) are themselves the evidence. A presence
   accrual without probe-evidence refs is refused with a typed reason
   code — presence without liveness evidence is not payable.
3. **Sybil-priced.** Presence accrues per IDENTITY, never per device or
   process, mirroring Pluralis's one-token-many-GPUs rule (many GPUs
   under one token aggregate to one identity score). Two devices under
   one contributor identity share one cap, so splitting a machine into
   N processes buys nothing.

**Compute receipts remain what they are:** verified work closeouts at
the class rate, exactly the shape the leaderboards already read from
the payment authority (`receiptKind: 'settlement_recorded'`,
`state: 'settled'`, integer `amountSats`).

## What landed

`workers/api/src/training-presence-compute-receipts.ts`
(tests: `src/training-presence-compute-receipts.test.ts`):

1. **`PresenceTierPolicy`** — `capSatsPerIdentityPerDay` (positive
   integer sats), `probeEvidenceRequired: S.Literal(true)` (a policy
   that waives probe evidence is unrepresentable), `policyRef`, and
   roadmap/buildout source refs.
2. **`classifyReceiptTier`** — work kind + join-lifecycle state +
   verification outcome refs → `'presence_tier' | 'compute_tier'` with
   a typed reason code in the `receipt_tier.public.*` namespace
   (following `join_lifecycle.public.*` / `device_admission.public.*`):
   - `liveness_probe`, `qualification_probe` → presence tier.
   - `shadow_window_work` → presence tier
     (`receipt_tier.public.presence_shadow_window_work`).
   - `verified_closeout` from an **active** device with verification
     outcome refs → compute tier
     (`receipt_tier.public.compute_merged_verified_closeout`).
   - `verified_closeout` from a **warmup** device → presence tier, by
     construction: warmup is the shadow window (P1.1, psionic#1125 —
     verified but unmerged), exactly as Pluralis sync-phase samples do
     not count toward `target_batch_size`.
   - `verified_closeout` from any other ladder state → presence tier
     (`receipt_tier.public.presence_unmerged_work_not_active`).
   - `verified_closeout` with **no** verification outcome refs → typed
     refusal: unverified work has no tier in a network that pays sats.
3. **`aggregatePresenceAccruals`** — the per-identity aggregator. Input
   entries carry `{ identityRef, deviceRef, accrualDayUtc, amountSats,
   probeEvidenceRefs }`; output groups by (identity, UTC day),
   aggregates device refs and probe-evidence refs under the identity,
   applies the cap to the identity sum, and:
   - truncates accrual beyond the cap with a typed
     `PresenceCapTruncationEvent`
     (`receipt_tier.public.presence_cap_truncated`, carrying
     `requestedSats` and `truncatedSats`),
   - refuses entries with empty `probeEvidenceRefs` via typed
     `PresenceAccrualRefusal` records
     (`receipt_tier.public.presence_probe_evidence_missing`) that never
     accrue,
   - enforces bounded batches (≤ 500 entries per call, ≤ 32 evidence
     refs per entry) and the standard private-material ref scan
     (wallet/payment/secret/raw-timestamp substrings; the
     platform-issued `receipt_tier.public.*` taxonomy is stripped
     before the scan, per the wallet_not_ready lesson of 2026-06-11).
4. **`exportPresenceComputeReceiptTierContract`** — the versioned,
   deeply frozen, JSON-able contract
   (`openagents.training.presence_compute_receipt_tiers.v1`) with
   `contractDefinitionOnly: true` and `livePayoutClaim: false` in the
   payload itself, same posture as the device-admission gate contract.

No clock is read anywhere: accrual day keys and all evidence are passed
in by callers.

## Why the cap unit is sats per identity per UTC day

Per-window was the other candidate. Rejected because windows vary in
length and one device spans many windows per day, so a per-window cap
prices presence by window-scheduling luck rather than availability. The
§3.5 floor is a daily availability concept — "alive, enrolled, and warm
between assignments" — and a per-identity-per-day cap makes the Sybil
budget auditable arithmetic: maximum presence spend = cap × enrolled
identities × days, independent of how many processes anyone runs.

The seeded contract value (1000 sats/identity/day) is floor money by
design: enough to keep a device enrolled and probed, two to three
orders of magnitude below a productive compute day. It is a contract
DEFINITION, not live payout policy.

## Boundary: contract layer only

This module is the policy/contract layer ABOVE the payment authority.
It never dispatches payment, and it consumes the existing
payment-authority receipt shape read-only (the leaderboards'
`settledSatsFromPaymentAuthorityReceipt` reader is the canonical
example of that consumption). Live payment dispatch was deliberately
not touched.

## Verified vs settlement-gated remainder

Landed and test-covered (`src/training-presence-compute-receipts.test.ts`,
14 tests):

- tier classification: shadow/warmup work → presence tier; merged
  verified closeout from an active device → compute tier; probes →
  presence tier; non-active unmerged closeouts → presence tier;
  unverified compute claims refused typed,
- cap enforcement: accrual beyond the cap truncates to the cap with a
  typed truncation event; at-or-under-cap accrual is untouched,
- per-identity aggregation: two devices under one identity produce one
  capped accrual whose cap applies to the sum (Sybil pricing),
- probe-evidence requirement: presence accrual without probe evidence
  is refused with a typed reason code and never accrues,
- bounded batches, private-material ref scanning, and the frozen,
  versioned, JSON-round-trip-stable contract export.

Hardware/settlement-gated remainder, **not** claimed here:

- one settled presence receipt and one compute closeout from a real
  device across its full join ramp (registered → qualified →
  state_synced → warmup → active) — requires live contributor hardware
  and live settlement through the payment authority,
- wiring the tier contract into live payment dispatch and the funnel's
  public projections (the contract export is the seam; consumption is
  its own change),
- the #4681 liveness/qualification probe actually paying presence-tier
  on a live device (the instrument exists; the paid loop is
  settlement-gated).
