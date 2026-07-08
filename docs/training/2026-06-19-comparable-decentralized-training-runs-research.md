# Comparable Decentralized Training Runs — Research Reference

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-19
Branch: `assault-pylon`
Owner sign-off: REQUIRED before any largest-run claim (this doc is research /
evidence assembly only; it asserts no superiority claim and flips no promise).

## Purpose

`pylon.largest_decentralized_training_claim.v1` is RED with three blockers:

- `largest_training_participant_methodology_missing`
- `comparable_training_run_evidence_missing`
- `public_training_contributor_receipts_missing`

The counting methodology is documented in
`docs/training/2026-06-19-decentralized-training-participant-scale-methodology.md`.
This doc addresses `comparable_training_run_evidence_missing`: it records, with
citations, what the comparable public decentralized-training runs are and what
their participant scale is, so any future largest-run claim can be stated against
named, dated, comparable evidence rather than marketing memory.

This is research only. It does NOT claim OpenAgents has the largest run, has
beaten any network, or has any benchmark-beating contributor count.

## The number to beat (per Episode 236)

Episode 236 (`docs/transcripts/236.md`) frames the target as **~200 contributors
from Bittensor** as "the number to beat." This is the aspirational headline, not
a verified head-to-head against a single named run.

## Comparable public runs / networks (cited)

The independent prior-art review at
`docs/launch/2026-06-18-world-firsts-verification.md` (#5395) already enumerated
the relevant networks with sources. Consolidated here for the largest-run
question specifically:

| Network / run | Comparable participant scale | Incentive | Citation |
| --- | --- | --- | --- |
| Bittensor / Templar Subnet 3 — "Covenant-72B" | ~70 contributors on home internet; described as the largest decentralized LLM pretraining run | TAO (token, not BTC) | world-firsts-verification.md lines 79-84 |
| Gensyn | Verifiable-compute training marketplace, node count not a single published run figure | $AI token | world-firsts-verification.md lines 85-88 |
| Prime Intellect | Trustless training network with verifiable rewards | token rewards | world-firsts-verification.md lines 89-92 |
| Nous Research / Psyche (DisTrO) | Decentralized training on idle GPUs | ecosystem | world-firsts-verification.md lines 93-95 |

Notes:

- The single concrete "largest decentralized LLM pretraining" comparable with a
  *published contributor count* is Templar Covenant-72B at **~70 contributors**.
  The "~200" in the transcript is the broader claim-to-beat, not a single cited
  run's verified figure.
- All cited networks pay in **tokens**, not Bitcoin. The OpenAgents distinction
  (real Bitcoin to consumer devices) is captured under
  `claims.world_first_ai_training_paid_bitcoin.v1` and is a *different* axis from
  raw contributor count.

## Current OpenAgents position (honest)

- Live run `run.tassadar.executor.20260615` has a `qualifiedContributorCount` of
  exactly the small bounded set described by
  `training.decentralized_training_launch.v1` (two distinct independent
  contributors paid real Bitcoin).
- Two counted contributors is **far below** the ~70 (Templar) and ~200
  (transcript target) figures.

## Conclusion for the blocker

`comparable_training_run_evidence_missing` now has a written, cited home: the
comparable runs and their scales are documented. **But the largest-run promise
stays RED**, because:

1. `public_training_contributor_receipts_missing` — OpenAgents has two counted
   contributors, not a count comparable to ~70 / ~200.
2. A largest-run claim requires *beating* a comparable run; the evidence here
   shows OpenAgents is not currently at that scale.

A green flip requires an actual run at comparable-or-greater verified-contributor
scale with public per-contributor receipts, plus an owner-signed receipt-first
upgrade per `proof.claim_upgrade_receipts.v1`. This doc removes the
"we never wrote down the comparables" gap; it does not manufacture the scale.
