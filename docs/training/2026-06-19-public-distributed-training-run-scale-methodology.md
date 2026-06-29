# Public Distributed Training Run — Participant-Count & Network-Scale Methodology

Date: 2026-06-19
Branch: `wave2-training-methodology`
Owner sign-off: REQUIRED before any promise green/yellow flip. This document is
evidence assembly only; it flips no promise.

Promise: `training.public_distributed_training_run.v1` (state: red).

## Purpose

`training.public_distributed_training_run.v1` claims that pylons participate in
*public distributed model-training runs* with visible run state, verified work,
reported results, and contributor payment for useful work. As of registry
`2026-06-19.7` the multi-contributor settlement leg of that claim is MET: the
live run `run.tassadar.executor.20260615` has settled real Bitcoin to **five
distinct independent contributor pylons** (1,020 sats real total), each backed
by a Verified `exact_trace_replay` challenge (see
`docs/promises/2026-06-19-training-live-run-evidence-destale.md`).

The remaining gate, per that promise's `verification` field, is twofold:

1. a **documented participant-count / network-scale methodology** — how a
   "participant" is counted and what threshold constitutes a *network-scale*
   run, and
2. **broad accepted-work receipts** beyond the five canary-scale settlements.

This document supplies (1) and states honestly where (2) stands. It is the
public, dereferenceable home for the methodology so the promise's remaining gate
is a *documented-methodology + broad-receipts* gate rather than an undocumented
one. It asserts no scale claim and authorizes no copy upgrade.

## Relationship to the existing participant-count rule

The qualified-contributor *counting rule* (who counts as one paid contributor)
is already written and code-anchored in
`docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md`
for `pylon.consumer_compute_earns_bitcoin_self_serve.v1` and
`pylon.largest_decentralized_training_claim.v1`. This document does **not**
duplicate that rule; it reuses it verbatim as the per-contributor primitive and
adds the dimension that document explicitly declined to establish: what *scale*
means for a public distributed training run, and the honest current scale.

## The participant-count primitive (reused, code-anchored)

A participant is counted as one **qualified contributor** on a public training
run only if all three hold, exactly as enforced in
`apps/openagents.com/workers/api/src/training-run-window-authority.ts`
(`qualifiedContributorCount` metric construction):

1. **Admitted** — held or holds a real window lease against the run.
2. **Accepted, replay-verified useful work** — a Worker-D1
   `exact_trace_replay` verification challenge in state `Verified`, joined to one
   of that contributor's run leases.
3. **Public-safe provider-confirmed settlement receipt ref** linked to the run
   (`settlement_recorded`, `state` settled, `realBitcoinMoved:true`).

Verbatim from the metric's own `description` field in code:

> "Qualified contributor count equals admitted contributors with accepted,
> replay-verified useful work and public-safe provider-confirmed settlement
> receipt refs linked to this run. It is derived from Worker D1 verified
> exact_trace_replay challenges joined to run leases plus provider-confirmed
> settled receipt projections; raw registrations and stale heartbeats never
> count."

Never counted: raw registrations / first-run installs; stale or live heartbeats
with no accepted verified work; pending, offered, claimed, wallet-side, or
simulation-only (`realBitcoinMoved:false`) receipts when counting *real-paid*
contributors. The one simulation row in the live feed is excluded by this rule.

## What "scale" means for a public distributed training run

A *public distributed training run* is described along three axes, each measured
from the same code-derived metrics so it is dereferenceable and cannot drift
from the receipts:

1. **Distinct-contributor scale** — `summary.metrics.qualifiedContributorCount`:
   the number of distinct contributor pylons meeting the three-part rule above.
   This is the headline participant count.
2. **Accepted-work scale** — `summary.metrics.acceptedTraceCount` and
   `summary.metrics.verifiedWorkCount`: the breadth of accepted, replay-verified
   work units, not just the count of people. Two contributors doing one unit
   each is a different scale than five contributors doing thousands of units.
3. **Settlement scale** — `summary.metrics.providerConfirmedSettledPayoutSats`
   and `settledReceiptCount`: real Bitcoin actually moved (`realBitcoinMoved:true`
   only), not settled-state simulation.

A run's scale is the *tuple* of these three axes, each carrying its own
provenance label and `sourceRefs` to the underlying D1 rows / receipt
projections. There is no single scalar "scale score"; collapsing the axes would
let one large axis paper over a tiny one. A "network-scale" claim requires all
three axes to clear a stated threshold, not just one.

### Network-scale threshold (stated, not yet met)

For this promise, **network-scale** is defined as a public run that demonstrates,
on the live feed, **all** of:

- distinct-contributor scale **>= 50** qualified contributors,
- accepted-work scale showing **sustained** accepted verified work across those
  contributors (not a one-unit-per-contributor floor), and
- settlement scale with **broad** `realBitcoinMoved:true` receipts across the
  contributor set, none owner-armed canary exceptions.

The 50-contributor floor is the methodology threshold for *this* promise's
"network-scale run" language; it is deliberately well below the separate
200-contributor *largest-run benchmark* governed by
`pylon.largest_decentralized_training_claim.v1`. Clearing this 50-floor would
support "a public distributed training run is live at network scale"; it would
**not** support a largest-run claim. The two thresholds are intentionally
distinct and live in different promises.

## Honest current scale vs target

For the live run `run.tassadar.executor.20260615` at the time of writing
(registry `2026-06-19.7`):

| Axis | Current (live feed) | Network-scale target |
| --- | --- | --- |
| Distinct contributors | **5** (`qualifiedContributorCount`) | >= 50 |
| Accepted work units | **11** (`acceptedTraceCount`) | sustained, broad |
| Real settled | **1,020 sats**, 5 counted receipts | broad, no canary exception |

Current scale is **canary-scale, multi-contributor**: real, end-to-end,
independently verified, paid — but bounded. It is two orders of magnitude below
the stated 50-contributor network-scale floor on the headline axis, and the
1,000-sat majority of the settled total is a single owner-armed canary. The five
distinct settled contributors satisfy *existence of multi-contributor
settlement*; they do not satisfy *network scale*.

## The dereferenceable per-contributor receipt basis

Each counted contributor resolves to a public-safe per-contributor receipt
through the live routes:

- `GET /api/public/training/runs/run.tassadar.executor.20260615` →
  `summary.metrics.qualifiedContributorCount` (value + description + `sourceRefs`,
  where `sourceRefs` includes each contributor pylon ref and its settlement
  receipt ref).
- `GET /api/public/training/runs/run.tassadar.executor.20260615/settlements` →
  the enumerable per-run settled feed; each counted contributor's
  `realBitcoinMoved:true` row (receipt, sats, pylon, backing challenge) resolves
  here, with the one `realBitcoinMoved:false` simulation row marked excluded.

The five counted contributors and their receipt/challenge basis are enumerated
in `docs/promises/2026-06-19-training-live-run-evidence-destale.md`. No raw
wallet address appears in any projection; pylon refs are public-safe identifiers.

## Effect on the promise's remaining gate

With this document published:

- The **participant-count / network-scale methodology** sub-gate is satisfied:
  the counting rule is code-anchored, the three scale axes and their dereference
  paths are stated, and the network-scale threshold is written down with the
  honest current-vs-target gap.
- The **broad accepted-work receipts** sub-gate remains open: the live feed shows
  five canary-scale contributors / 11 accepted units, below the stated
  network-scale floor.

So the promise's remaining gate narrows to: **broad accepted-work receipts that
clear the documented network-scale threshold** (this doc's 50-contributor /
sustained-work / broad-settlement floor). The methodology is no longer the
blocker; receipts at scale are.

## What this does NOT establish (boundary)

- It does NOT assert network scale, "hundreds paid", "paid at scale", or a
  largest-run comparison. Current scale is five canary-scale contributors.
- It does NOT widen install-platform coverage. Current install evidence is
  macOS + Linux; Windows/WSL is a deliberate owner scope-out (see
  `apps/pylon/docs/platform-support.md`).
- It does NOT flip `training.public_distributed_training_run.v1`. A red->yellow
  or any green move stays owner-gated and receipt-first per
  `proof.claim_upgrade_receipts.v1`, on the strength of broad accepted-work
  receipts that clear the threshold stated here — not on the strength of this
  document alone.
