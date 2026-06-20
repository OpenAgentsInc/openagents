# Decentralized Training Participant & Scale Methodology

Date: 2026-06-19
Branch: `assault-pylon`
Owner sign-off: REQUIRED before any promise green flip (this doc is evidence
assembly only; it does not flip any promise).

## Purpose

Several Pylon product promises are blocked on a missing, written, dereferenceable
methodology for how participant / contributor counts are measured for public
decentralized-training claims. Specifically:

- `pylon.consumer_compute_earns_bitcoin_self_serve.v1` →
  `blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`
- `pylon.largest_decentralized_training_claim.v1` →
  `blocker.product_promises.largest_training_participant_methodology_missing`

The counting rule is already enforced in code as a derived, provenance-labelled
metric. This document is the public, dereferenceable statement of that rule so
the methodology blocker has a citeable home. It documents the *rule and where it
lives*; it does NOT assert any scale claim, any "anybody earns" copy, or any
largest-run comparison. Those remain red and owner-gated.

## The qualified-contributor counting rule (authoritative)

A participant is counted as a **qualified contributor** on a public training run
**only if all of the following hold**:

1. The contributor was **admitted** to the run (holds or held a real window
   lease against the run), and
2. The contributor produced **accepted, replay-verified useful work** — i.e. a
   Worker-D1 `exact_trace_replay` verification challenge in state `Verified`,
   joined to one of that contributor's run leases, and
3. The contributor has a **public-safe, provider-confirmed settlement receipt
   ref** linked to that run.

The following are **never** counted:

- Raw registrations / first-run installs.
- Stale or live heartbeats with no accepted verified work.
- Pending, offered, claimed, wallet-side, or simulation-only
  (`realBitcoinMoved:false`) receipts when counting *real-paid* contributors.

This is the rule implemented for the live metric, verbatim from the metric's own
`description` field:

> "Qualified contributor count equals admitted contributors with accepted,
> replay-verified useful work and public-safe provider-confirmed settlement
> receipt refs linked to this run. It is derived from Worker D1 verified
> exact_trace_replay challenges joined to run leases plus provider-confirmed
> settled receipt projections; raw registrations and stale heartbeats never
> count."

## Where the rule is enforced in code (dereferenceable)

- `apps/openagents.com/workers/api/src/training-run-window-authority.ts`
  - `qualifiedContributorCount` metric construction (the description quoted
    above) and its `sourceRefs` (the contributor refs plus their settlement
    receipt refs).
  - `participantCountRule` manifest field on the window-seal contract, which lets
    each run carry its own per-run participant-count manifest text.
- `apps/openagents.com/workers/api/src/public-pylon-stats.ts`
  - The public aggregator that sums `summary.metrics.qualifiedContributorCount`
    across runs and carries the per-contributor `sourceRefs`.

## Public dereference path

The count is enumerable and dereferenceable per run:

- `GET /api/public/training/runs/{runRef}` →
  `summary.metrics.qualifiedContributorCount` (value + description + sourceRefs).
- `GET /api/public/training/runs/{runRef}/settlements` → the enumerable per-run
  settled feed, where each counted contributor's `realBitcoinMoved:true`
  settlement row can be resolved.

For the live run `run.tassadar.executor.20260615` at the time of writing,
`qualifiedContributorCount` is exactly the small bounded set described by
`training.decentralized_training_launch.v1` (two distinct independent
contributors paid real Bitcoin, 1,005 sats real total). This methodology does
not change that number; it only documents how it is derived.

## What this does NOT establish (boundary)

- It does NOT assert network scale, "hundreds paid", "paid at scale", or any
  largest-run comparison.
- It does NOT widen install-platform coverage. Current install evidence is
  macOS + Linux; Windows/WSL is a deliberate owner scope-out (see
  `apps/pylon/docs/platform-support.md`), and broad "anybody on any platform"
  copy stays blocked.
- It does NOT flip any promise. A green flip for
  `pylon.consumer_compute_earns_bitcoin_self_serve.v1` or
  `pylon.largest_decentralized_training_claim.v1` still requires the remaining
  blockers cleared with their own receipts and an owner-signed, receipt-first
  upgrade per `proof.claim_upgrade_receipts.v1`.

## Green-readiness status for the methodology blockers

- `consumer_compute_self_serve_scale_methodology_missing`: the methodology is now
  written and dereferenceable here and is enforced in code with a live public
  route. **Green-ready on this specific blocker pending owner review**; the other
  two blockers on that promise (Windows/WSL coverage, Spark-helper autostart
  receipt) are tracked separately.
- `largest_training_participant_methodology_missing`: the counting rule is
  written here. The *comparison* baseline is documented separately in
  `docs/training/2026-06-19-comparable-decentralized-training-runs-research.md`.
  Even with both docs, the largest-run promise stays red on
  `comparable_training_run_evidence_missing` (we do not yet have a comparable
  run at the benchmark count) and `public_training_contributor_receipts_missing`
  (we have two counted contributors, not a benchmark-beating count).
