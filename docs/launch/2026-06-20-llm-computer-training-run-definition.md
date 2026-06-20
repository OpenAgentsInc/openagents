# Definition — "LLM-computer training run" (claim boundary)

- Date: 2026-06-20
- Promise: `claims.world_first_public_llm_computer_training_run.v1` (state: **red**)
- Blocker advanced: `blocker.product_promises.llm_computer_training_run_definition_missing`
- Companion docs:
  - `docs/launch/2026-06-18-world-firsts-verification.md` (prior-art search, Claim 2)
  - `docs/launch/2026-06-18-evidence-pack.md` (dereferenceable backing)
  - `docs/launch/2026-06-18-pylon-v1-launch-readiness-audit.md` (claim-by-claim gate)
- Related promises: `compute.tassadar_executor_poc.v1` (green),
  `models.tassadar_percepta_executor.v1`, `proof.claim_upgrade_receipts.v1`

## Why this doc exists

The world-firsts verification (L-3) found the Episode 238 Claim 2 defensible
*only* as "first **public/open-contributor** LLM-computer training run," with
Percepta credited as the paradigm originator. But the phrase "training run"
carries a dangerous ambiguity: to most listeners it means **gradient-descent
model training**, which is **not** what the LLM-computer core does or what the
live Tassadar run does. The promise's own `verification` field names this as the
first thing green requires:

> "(1) a precise definition of 'LLM-computer training run' that does not
> overclaim against the no-gradient-descent executor PoC"

This document supplies that precise definition and fixes the claim boundary so
no on-camera or copy use of the phrase can drift into the overclaim. It does
**not** flip the promise (still red); it removes the *definition-missing*
blocker and leaves the evidence-pack and owner-signed-upgrade blockers standing.

## Terms, defined precisely

### LLM-computer (Percepta-class)

An **LLM-computer** is a transformer whose weights are **constructed by
compiling a program** (e.g. a WASM interpreter / VM) directly into the weights,
so the forward pass **executes that program exactly**, token by token, with no
learned approximation. Percepta's `transformer-vm` is the reference artifact:
a WebAssembly interpreter compiled into transformer weights, running programs
at 100% accuracy. DeepMind's Tracr (RASP → weights) is the same family of
*technique*. The defining property is **construction, not learning**: weights
are *assembled* to be provably correct, not *fit* to data.

### "Training run" — the two senses (this is the whole hazard)

| Sense | What it means | Does the LLM-computer core do this? | Does the live Tassadar run do this? |
|---|---|---|---|
| **(A) Gradient-descent training** | Iteratively update weights via backprop on a loss over data; the everyday meaning of "training run." | **No.** The core has no gradient descent. | **No.** No public gradient enters any optimizer (see `training.public_gradient_windows.v1`, planned). |
| **(B) Executor-construction / exact-trace run** | A coordinated, bounded run that *constructs and exercises* compiled-program executors and **verifies their exact traces** by independent replay, paying contributors for verified work. | **Yes** — this is the paradigm. | **Yes** — this is exactly `compute.tassadar_executor_poc.v1`. |

The Episode 238 claim is true **only in sense (B)**. The word "training" is
load-bearing-and-misleading: it must always be qualified as the
**LLM-computer / executor-construction** sense, never left bare next to the
gradient-descent connotation.

### What the live Tassadar run actually is

Per the green `compute.tassadar_executor_poc.v1` promise and the evidence pack:
a bounded **exact-trace executor run** — `run.tassadar.executor.20260615`,
state `active` — that dispatches digest-pinned exact-program workloads to real
contributor Pylons, has each trace **independently replayed by a separate
validator device**, records Verified/Rejected challenge receipts, and settles
real Bitcoin for verified work. Its stated objective is literally "Grow the
Tassadar verified-trace corpus via paid executor-trace" work. It is **not**
fitting model weights to data.

## The claim boundary (what may and may not be said)

### Defensible (sense B, fully qualified)

> "The first **public, open-contributor LLM-computer training run** — the
> compiled-program-in-weights paradigm **defined by Percepta** — run for the
> first time as a public network anyone can join and get paid (in Bitcoin)
> for **replay-verified** executor work."

Load-bearing qualifiers that may **never** be dropped:
1. **public / open-contributor network** — the firstness lives here; Percepta
   ran a closed research artifact, not a public paid network.
2. **Percepta credit** — Percepta defined and demonstrated the paradigm
   (March 2026); we did not invent the LLM-computer.
3. **executor / exact-trace / replay-verified** — anchors "training run" in
   sense (B) and ties it to the bounded PoC scope.

### Overclaims to refuse

- ❌ "We invented the LLM-computer" / "first LLM-computer" — Percepta did.
- ❌ Bare "training run" implying gradient-descent model training — sense (A) is
  not happening here.
- ❌ "We trained a model" / "we trained an LLM" from this lane —
  weights are *constructed*, not *fit*; no loss, no backprop, no optimizer.
- ❌ General "LLM-computer capability," performance parity/superiority vs CPUs,
  or transformers-as-a-served-product — all gated separately and out of scope
  for the bounded PoC.
- ❌ Network-scale framing from a bounded one-workload-family PoC.

## What this resolves vs. what remains

**Resolved by this doc** (clears `llm_computer_training_run_definition_missing`):
a precise, written definition of "LLM-computer training run" that pins the
phrase to sense (B), credits Percepta, and enumerates the refuse-list so the
phrase cannot be used to overclaim against the no-gradient-descent executor PoC.

**Still open (do not drop these blockers):**
- `blocker.product_promises.world_first_evidence_pack_missing` — a single
  dereferenceable evidence pack tying the *qualified* world-first specifically
  to the live-run receipts (the §-by-§ pack exists for the broader launch, but
  not a focused world-first-Claim-2 pack).
- `blocker.product_promises.world_first_owner_signed_upgrade_missing`
  (carried on the sibling `claims.world_first_ai_training_paid_bitcoin.v1`) and
  the receipt-first, owner-signed upgrade required by
  `proof.claim_upgrade_receipts.v1` before any green flip.

The promise stays **red**. This document changes only the precision of the
language, not the state.
