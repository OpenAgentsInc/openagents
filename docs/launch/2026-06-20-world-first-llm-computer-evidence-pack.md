# Evidence Pack — World-First Claim 2: "first public LLM-computer training run"

- Date: 2026-06-20
- Promise: `claims.world_first_public_llm_computer_training_run.v1` (state: **red**)
- Blocker advanced: `blocker.product_promises.world_first_evidence_pack_missing`
- Companion docs:
  - `docs/launch/2026-06-18-world-firsts-verification.md` (prior-art search, Claim 2)
  - `docs/launch/2026-06-18-evidence-pack.md` (broad Episode 238 pack; §3 + §7)
  - `docs/launch/2026-06-20-llm-computer-training-run-definition.md` (claim boundary)
  - `docs/launch/2026-06-18-pylon-v1-launch-readiness-audit.md` (claim-by-claim gate)
- Related promises: `compute.tassadar_executor_poc.v1` (green),
  `models.tassadar_percepta_executor.v1`, `proof.claim_upgrade_receipts.v1`

## Why this doc exists

The broad Episode 238 evidence pack (`2026-06-18-evidence-pack.md`) covers every
on-camera claim. It does **not** isolate the LLM-computer world-first into a
single, skeptic-runnable pack tied qualifier-by-qualifier to the live-run
receipts. The promise's `verification` field names that as the first thing green
requires:

> "(1) a dereferenceable evidence pack tying the qualified world-first
> specifically to the live-run receipts"

This document is that focused pack. It restates the **owner-final defensible
wording** for Claim 2, then maps each load-bearing qualifier to a public,
dereferenceable ref a critic can check. It does **not** flip the promise (still
**red**); it clears the *evidence-pack-missing* blocker and leaves the
owner-signed-upgrade blocker standing.

> Honest-scope note: this document is public-safe. It contains no secrets, no
> wallet seeds, no payment hashes, no raw Lightning/Bitcoin addresses, and no
> private data. It links only to public projections and content-addressed refs.

> Status legend (same as the broad pack):
> - **settled-live** — true today on the live system for the scoped wording.
> - **mechanism-proven** — proven end-to-end at least once, not yet at scale.
> - **gated** — defensible only with full qualifiers and/or held RED pending the
>   receipt-first upgrade. Not safe as bare copy.

## The claim, in its only defensible form

> "The first **public, open-contributor LLM-computer training run** — the
> compiled-program-in-weights paradigm **defined by Percepta** — run for the
> first time as a public network anyone can join and get paid (in Bitcoin) for
> **replay-verified** executor work."

Three load-bearing qualifiers carry the firstness; none may ever be dropped.
This pack proves each one in turn.

## Qualifier 1 — "public / open-contributor network" (where the firstness lives)

Percepta ran a closed single-organization research artifact. Our firstness is
that the paradigm is now run as a **public network anyone can join and get paid
for**. The discriminator is the public paid contributor loop, proven on the live
run.

**Proof (live-run receipts):**
- Run is live and public: `https://openagents.com/api/public/tassadar-run-summary`
  → `runRef: run.tassadar.executor.20260615`, `runState: active`.
- Public front door anyone can enter: `https://openagents.com/AGENTS.md`
  (Pylon-first join path) and `https://openagents.com/INSTALL.md`.
- Real Bitcoin paid to **two distinct independent contributors**
  (`providerConfirmedSettledPayoutSats: 1005`, `settledReceiptCount: 2`,
  `qualifiedContributorCount: 2`), each receipt dereferenceable:
  - `https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618`
  - `https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1`
- Per-run settlements feed (real-vs-sim distinguished):
  `https://openagents.com/api/public/training/runs/run.tassadar.executor.20260615/settlements`

**Status: mechanism-proven (bounded).** The public paid loop is proven
end-to-end with two independent contributors, not yet at scale. Do not imply
network-scale participation — see `pylon.consumer_compute_earns_bitcoin_self_serve.v1`
(red). The firstness is "public paid run exists," not "at scale."

## Qualifier 2 — "LLM-computer paradigm, defined by Percepta" (the credit)

We did **not** invent the LLM-computer. Percepta defined and demonstrated the
compiled-program-in-weights paradigm. This credit must always travel with the
claim.

**Proof (paradigm provenance):**
- Percepta blog: `https://www.percepta.ai/blog/constructing-llm-computer`
- Percepta reference artifact: `https://github.com/Percepta-Core/transformer-vm`
  (Apache-2.0 WebAssembly interpreter compiled into transformer weights).
- Internal notes crediting Percepta:
  `docs/tassadar/2026-06-10-percepta-constructing-llm-computer-notes.md`,
  `docs/tassadar/2026-06-11-llm-computer-full-introduction.md`.
- Prior-art search confirming Percepta is the originator and is not defeated as a
  *public paid* run: `docs/launch/2026-06-18-world-firsts-verification.md` §"Claim 2".

**Status: settled (provenance).** Percepta credit is documented and the prior-art
search is on file. Never say "first LLM-computer" or "we invented the
LLM-computer."

## Qualifier 3 — "executor / exact-trace / replay-verified" (anchors "training run")

"Training run" here means the **executor-construction / exact-trace sense
(sense B)** defined in `2026-06-20-llm-computer-training-run-definition.md`, not
gradient-descent model training (sense A). The anchor is independent replay
verification of exact program traces.

**Proof (live-run verification receipts):**
- Run summary `realGradient.verifiedReplayPairs` — verified `exact_trace_replay`
  pairs, each with a **distinct** `workerRef` and `validatorRef`, a `challengeRef`,
  and a `verdictRef` (`verifiedWorkCount: 6`, `acceptedTraceCount: 6`).
- Run summary `realGradient.rejectedReplayPairs` — `ExecutorTraceMismatch`
  rejections (`rejectedWorkCount: 3`): the verifier is not a rubber stamp.
- A directly dereferenceable verified challenge:
  `https://openagents.com/api/public/training/verification-challenges/training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c`
  → `state: Verified`.
- Verifier policy from the run manifest: `verifierPolicy: exact_trace_replay`.
- Run objective (in the summary): "Grow the Tassadar verified-trace corpus via
  paid executor-trace work, verified by exact replay." — explicitly **not**
  fitting model weights to data.
- Backing promise: `compute.tassadar_executor_poc.v1` (green).

**Status: mechanism-proven.** Exact-trace replay verification (with real
rejections) is proven end-to-end. "Training run" is defensible only in sense B.

## Overclaims this pack does NOT back (the refuse-list)

- ❌ Bare "world first" with no qualifiers.
- ❌ "first LLM-computer" / "we invented the LLM-computer" — Percepta did.
- ❌ Bare "training run" implying gradient-descent model training — sense A is
  not happening here; the core has no gradient descent.
- ❌ "first decentralized training run" — token-paid decentralized training
  predates us; the Bitcoin discriminator belongs to Claim 1, not Claim 2.
- ❌ General LLM-computer capability, performance parity vs CPUs, or
  transformers-as-a-served-product — all gated separately.
- ❌ Network-scale framing from a bounded two-contributor PoC.

## One-screen verification recipe (for a skeptic / agent)

```
# Q1 public/open-contributor run is live + paid two contributors
curl -s https://openagents.com/api/public/tassadar-run-summary \
  | grep -o '"runState":"[^"]*"'                      # -> "active"
curl -s https://openagents.com/api/public/tassadar-run-summary \
  | grep -o '"qualifiedContributorCount":[0-9]*'      # -> 2

# Q2 Percepta paradigm credit (paradigm originator, not us)
#   https://www.percepta.ai/blog/constructing-llm-computer
#   https://github.com/Percepta-Core/transformer-vm

# Q3 exact-trace replay verification (with real rejections)
curl -s https://openagents.com/api/public/training/verification-challenges/training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c \
  | grep -o '"state":"[^"]*"'                          # -> "Verified"

# Honest state: this promise is RED, not green
curl -s https://openagents.com/api/public/product-promises \
  | grep -o 'claims.world_first_public_llm_computer_training_run.v1'
```

(Public endpoints in the broad pack were curl-checked on 2026-06-18; this focused
pack reuses those same already-verified refs and adds no new unverified URLs.)

## What this resolves vs. what remains

**Resolved by this doc** (clears `world_first_evidence_pack_missing` for this
promise): a single, focused, dereferenceable evidence pack that ties the
*qualified* Claim-2 world-first to the live-run receipts qualifier-by-qualifier,
with a skeptic-runnable verification recipe and an explicit refuse-list.

**Still open (do not drop):**
- `blocker.product_promises.world_first_owner_signed_upgrade_missing` — the
  receipt-first, owner-signed upgrade required by `proof.claim_upgrade_receipts.v1`
  before any green flip.
- Scale: the public paid loop is bounded (two contributors); network-scale
  framing remains out of scope.

The promise stays **red**. This document changes only the evidence backing, not
the state.
