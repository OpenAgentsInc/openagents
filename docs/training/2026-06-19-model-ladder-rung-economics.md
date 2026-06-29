# Model Ladder — Rung Definitions, R1 Closeout Criteria & Economics Gate

Date: 2026-06-19
Branch: `wave2-training-methodology`
Owner sign-off: REQUIRED before any promise green flip. This document is evidence
assembly only; it flips no promise.

Promise: `training.model_ladder.v1` (state: planned).

## Purpose

`training.model_ladder.v1` claims Psion models train up a **receipt-gated
ladder** — R0 → R1 → R2 → R3 → R4 — where each rung re-runs the whole pipeline
behind an **engineering gate** and an **economics gate**. The promise is
`planned` with two blockers:

- `blocker.product_promises.r1_full_rehearsal_missing` — no operator-scale full
  rehearsal (R1) has run.
- `blocker.product_promises.rung_economics_gate_format_missing` — the per-rung
  economics-gate report format was not published in a dereferenceable form.

This document is the dereferenceable home for the **rung definitions**, the **R1
closeout criteria**, and the **published economics-gate format** (what a rung run
must demonstrate and settle). It clears the *format-missing* dimension by writing
the format down; it does NOT clear `r1_full_rehearsal_missing` (no rung above R0
has run) and flips no promise. It is the public statement of the ladder discipline
already drafted in
`docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md` §10–§12.

## Rung definitions (authoritative)

The ladder is **sequencing discipline, not capability**. Each rung re-runs the
*entire* pipeline (data → ablations → marathon → post-training → evals) at a
scale that can actually be completed; the rehearsal is the point.

| Rung | Model class | Tokens (order) | Hardware reality | What the rung proves |
| --- | --- | --- | --- | --- |
| **R0** (exists) | tri-host bringup | ~4k | 2 Macs + 1 RTX 4080 | dispatch / checkpoint / receipt mechanics |
| **R1** | ~30–50M Psion | 1–5B | operator-owned devices, days | full pipeline end-to-end, all receipted |
| **R2** | ~125–200M Psion | 10–50B | operator + early contributor Pylons | first *network* pretraining with paid verified windows |
| **R3** | ~1B Psion | 100B+ | depends on R2's measured economics | first generally-useful model; post-training arc in anger |
| **R4** | ~3B class | 1T+ | only if R3 economics close | smol-playbook scale; not promised, priced by receipts |

**Rules of the ladder:**

1. No rung starts before the previous rung's **closeout receipt** exists.
2. Every rung re-runs the *whole* pipeline.
3. Scaling-law fits from each rung (the A3 machinery) size the next rung.
4. Each rung's registry promise is **written before** the run, with
   safeCopy/unsafeCopy bounds, transitioning only on receipts — the same
   discipline as `compute.tassadar_executor_poc.v1`.
5. A rung whose **economics gate fails twice** is recorded in the registry as
   information (a business-level falsifier, not a failure to paper over).

### R0 status (the only rung that exists)

R0 is a retained tri-host 12-step rehearsal — **3,992 train tokens at 2.74
effective tokens/sec** — recorded in psionic's actual-pretraining runbook
(`https://github.com/OpenAgentsInc/psionic/blob/main/docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md`).
It proves dispatch/checkpoint/receipt mechanics only. R0 rehearsal throughput is
**not** network training capability and must not be presented as such.

### Public rung-status projection (2026-06-20)

`GET /api/public/training/model-ladder-rungs` is the public-safe, live-at-read
projection for this promise. It exposes the R0-R4 rung definitions, the retained
R0 rehearsal, the six R1 closeout criteria below, and the five-field
economics-gate format. The route is a status surface only: it reports
`rungEconomicsGateFormatAvailable=true`, but keeps
`r1FullRehearsalAvailable=false`, `r1CloseoutReceiptAvailable=false`,
`r2NetworkRungReceiptAvailable=false`, and `greenGateSatisfied=false`.

The projection does not clear `blocker.product_promises.r1_full_rehearsal_missing`.
It also does not clear
`blocker.product_promises.model_ladder_network_rungs_not_run` for
`pylon.first_real_model_training_run.v1`; that still requires a real R2-or-above
network rung with paid verified windows and a closeout receipt.

## R1 closeout criteria (what an R1 closeout receipt must demonstrate)

R1 is the **operator-scale full rehearsal**: the whole pipeline dispatched
through the rails even when every device is operator-owned, because *dispatch is
the rehearsal*. An R1 closeout receipt is admissible only when **all** of the
following are present and receipted (engineering gate):

1. **Data** — corpus v2 carried through the refinery with deterministic-recompute
   verification and source-provenance + transform digests on each shard.
2. **Ablations** — 3–5 architecture/data/optimizer ablations off the priority
   list, run through the ablation harness with comparable receipts; a WSD
   (warmup-stable-decay) confirmation among them.
3. **Marathon** — one marathon run with a public run page, checkpoint-seal
   discipline (durable content-addressed checkpoint storage bound into the window
   seal), and at least one **restart-or-continue decision recorded as a receipt**.
4. **Post-training arc** — SFT + at least one preference-optimization (DPO) stage
   closing the arc, with rollout-generation and reward-grading dispatched as
   verified work.
5. **Evals** — decontaminated eval suite run against the resulting checkpoint,
   with a retained eval series that becomes R2's reference trajectory.
6. **Economics gate** — the published economics-gate report below, populated for
   R1 with provenance labels.

R1 does not require contributor Pylons or real-Bitcoin contributor settlement
(that is R2's network leg); R1's settlements may be operator-internal, but the
**economics-gate report is still required** so R2's gate has an R1 baseline to
improve against.

## The economics gate format (published)

Every rung carries an **economics gate**, not just an engineering gate. This is
the dereferenceable report format that clears
`rung_economics_gate_format_missing`. Each rung's economics gate is a record with
exactly these fields, each carrying a **provenance label** of `modeled`,
`measured`, or `settled`:

| Field | Definition | Provenance label |
| --- | --- | --- |
| `allInCostPerAcceptedOutcome` | Total cost per accepted training outcome, **including verification and settlement overhead** — not just compute. | modeled / measured / settled |
| `contributorPayoutPerDeviceHour` | Contributor payout per device-hour, stated **against the relevant opportunity floor** for that device class. | modeled / measured / settled |
| `verificationOverheadFraction` | Verification cost as a fraction of work cost — the term the whole pipeline is bent to drive toward zero. | modeled / measured / settled |
| `fallbackComparator` | The honest "what would we have paid instead" comparator (R2+: a rented small cluster), **never a vacuum**. | modeled / measured |
| `gateOutcome` | `pass` / `fail` / `fail-twice`. A second failure at the same rung is recorded, not retried silently. | — |

**Provenance discipline:** no field may be presented as `settled` unless a real
settled receipt backs it; no field may be presented as `measured` unless it comes
from an executed run; everything else is `modeled` and labelled as such. No
modeled economics may be presented as proven, and **no accepted work means no
revenue claim**.

**Verification-overhead field is already live.** As of 2026-06-12 the
window-seal contract carries verification overhead as a fraction of window cost,
recorded per rung (#4849) — the first concrete economics-gate field to exist in
code. The remaining fields above are the published format that the rest of each
rung's gate must populate.

### Why R2's gate is the important one

R2 is the first rung where **"the network trains a model" must clear against "we
rent a small cluster."** R2's economics gate is run honestly against that
rented-cluster fallback comparator, not against a vacuum. R2 is also the honest
green path for `pylon.first_real_model_training_run.v1`. R1's gate exists chiefly
to give R2 a baseline; R2's gate is where the decentralized-training economic
thesis is first falsifiable on receipts.

## The published economics gate (what a rung run must demonstrate + settle)

A rung run clears its economics gate only when:

1. The economics-gate report above is **populated for that rung** with correct
   provenance labels.
2. For R2 and above, the report includes a **real fallback comparator** and the
   gate outcome compares network cost against it honestly.
3. The contributor-facing fields (`contributorPayoutPerDeviceHour`) are backed by
   **real settled receipts** (`realBitcoinMoved:true`) for network rungs — modeled
   payout is insufficient for a network-rung economics pass.
4. A rung whose gate yields `fail-twice` is recorded in the registry as a
   business-level falsifier (per the buildout plan §3.9), not papered over.

## Effect on the promise's remaining gate

With this document published:

- `rung_economics_gate_format_missing` is **satisfied**: the per-rung
  economics-gate report format is written, dereferenceable, and tied to the one
  field (#4849) already live in code.
- `r1_full_rehearsal_missing` **remains open**: no rung above R0 has run, so no R1
  closeout receipt exists.

So the promise's remaining gate narrows to: **run R1 to a closeout receipt that
satisfies the criteria above and populates the published economics-gate format.**
The format is no longer the blocker; an executed, receipted R1 rehearsal is.

## What this does NOT establish (boundary)

- It does NOT claim any Psion rung above R0 is trained, in progress, or
  scheduled. No rung is dated.
- It does NOT present the ladder as a commitment to reach R4 — R3/R4 are priced
  by receipts and conditioned on the prior rung's economics closing.
- It does NOT present R0 rehearsal throughput as network training capability.
- It does NOT flip `training.model_ladder.v1`. Any green move stays owner-gated
  and receipt-first per `proof.claim_upgrade_receipts.v1`, on the strength of a
  real R1 closeout receipt — not on the strength of this document alone.
