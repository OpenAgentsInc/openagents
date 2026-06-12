# Window-Seal Staleness Contract (W2 Day-0, Pluralis Roadmap P0.2)

Date: 2026-06-12
Issue of record: openagents#4849 (master tracking issue openagents#4855)
Rails: #4748 (W2 day-0 contract freeze), #4673 (training run/window
authority)
Public proposal: forum post `6197bd1b`
Roadmap source: `docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md`
(workspace `openagents` repo), item P0.2

## What changed

Before the first factory shard ships, the window-seal record schema in the
training run/window authority
(`workers/api/src/training-run-window-authority.ts`) gains three seal-time
fields, carried as `sealMetadata` on the sealed `TrainingWindowRecord`,
persisted in D1 (`training_windows.seal_metadata_json`, migration
`0174_training_window_seal_staleness_contract.sql`), and published in the
public window projection:

1. **Staleness distribution of merged contributions**
   (`sealMetadata.staleness`): `stepsBehindMin` / `stepsBehindP50` /
   `stepsBehindP90` / `stepsBehindMax` plus `contributionCount`, with an
   optional bounded per-contribution list (`contributionRef`, `stepsBehind`;
   at most 64 entries). The distribution must satisfy
   `min <= p50 <= p90 <= max`, every sampled `stepsBehind` must be a
   non-negative integer inside the declared bounds, and an empty
   distribution (`contributionCount: 0`) must report all-zero values.
2. **Contributor-churn events within the window**
   (`sealMetadata.churn`): per-kind totals (`joinCount`, `lossCount`,
   `standbyPromotionCount`) plus an optional bounded event-ref sample
   (kind `join | loss | standby_promotion`, at most 64 entries). The
   sampled refs may undercount but never exceed the declared totals.
3. **Verification overhead as a fraction of window cost**
   (`sealMetadata.verificationOverhead`): `fraction` in `[0, 1]` published
   per ladder rung via `ladderRungRef` (for example `ladder.rung.r1`).

All refs use the existing `PublicSafeRef` shape, so the projection carries
no raw device, wallet, or prompt material — the same redaction posture as
`receiptRefs`.

`sealMetadata` is accepted only on the `active -> sealed` transition
(`POST /api/training/windows/:windowRef/seal`); supplying it on activate or
reconcile is a typed `validation_error`. Existing seal calls without the
field still work and persist `sealMetadata: null`, so the contract change
is backward compatible.

## Why (falsifier with a slope)

These fields exist so the network-training thesis stays falsifiable with a
direction, not just a boolean:

- The staleness distribution makes gradient delay a measured, sealed
  quantity per window — the AsyncPP lesson (measure the delay, respond to
  it, never pretend it is zero) applied at the contract layer before any
  optimizer-side correction exists.
- Churn events price contributor volatility instead of treating it as an
  incident, feeding the later standby-promotion and failure-semantics work
  (P2.1).
- Verification overhead per rung is the slope that must trend **down** as
  the model ladder climbs rungs. If verification cost per window does not
  fall as a fraction of window cost from rung to rung, verified
  decentralized training does not scale economically, and the seal records
  themselves are the evidence that says so.

## `maxAllowedStale` run-config semantics

`TrainingRunRecord.maxAllowedStale` (D1 `training_runs.max_allowed_stale`,
default `5`, declared as `DefaultMaxAllowedStaleSteps`) is the sync-reentry
trigger expressed as a contract value rather than a convention: a
contribution more than `maxAllowedStale` optimizer steps behind the window
head routes to sync re-entry (re-ramped through the staged join lifecycle,
P0.1/P1.1) rather than being merged (importing divergence) or rejected
(wasting a willing device).

The default of 5 is stated, not inherited: Pluralis node0 ships
`max_allowed_stale: 5` as prior art, and ours stays provisional until R1
rehearsal seal records carry measured steps-behind distributions. Any
revision is a per-run config change (the planning request accepts
`maxAllowedStale`), and the value projects publicly on the run record. The
load-bearing use of staleness in acceptance classes is P2.2 and is out of
scope here.

## Verified vs remaining gate

Landed and test-covered (`smoke:training-runs:public`):

- schema fields with types and validation (negative steps-behind,
  fraction outside `[0, 1]`, distribution ordering, over-long or
  over-counted samples all rejected),
- `maxAllowedStale` with stated default and public projection,
- a fixture seal record carrying all three fields with realistic values,
  persisting through reconcile and projecting publicly,
- backward-compatible seal calls without metadata.

Remaining hardware-gated acceptance bullet, **not** claimed here: an R1
rehearsal producing one real sealed window whose `sealMetadata` is derived
from actual contributor devices. That waits on R1 operator devices and the
rehearsal lane; until then every seal record carrying these fields is
fixture or operator-synthesized evidence and must not be presented as a
real-gradient receipt.
