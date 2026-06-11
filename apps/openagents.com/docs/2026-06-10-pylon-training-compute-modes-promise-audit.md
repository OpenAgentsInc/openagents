# Pylon Training And Compute Revenue Modes Promise Audit

Date: 2026-06-10

Registry version at audit time: `2026-06-10.4`

Status: third epic of the get-to-green campaign. This audit covers the two
Pylon-related promises **not** covered by the five-streams epic
(#4635–#4653, `2026-06-10-five-bitcoin-revenue-streams-promise-audit.md`)
or the v0.3 release cluster epic (#4654–#4663,
`2026-06-10-pylon-v03-release-cluster-promise-audit.md`). With this epic,
every Pylon-related promise in the registry has a filed implementation
path. The **Delegation Contract** in the five-streams audit is binding for
every issue below.

## Coverage Check: What Was Missing

Pylon-related promises by epic:

- Epic 1 (five streams): `pylon.five_bitcoin_revenue_streams.v1`,
  `pylon.data_trace_revenue.v1`, `pylon.gepa_worker_loop_v03.v1`,
  `provider.compliant_usage_labor.v1`, plus the tips/referral streams.
- Epic 2 (release cluster): `pylon.v03_release_candidate.v1`,
  `pylon.release_tomorrow.v1`, `pylon.install_without_wallet_knowledge.v1`,
  `pylon.no_dark_capacity_accounting.v1`,
  `autopilot.codex_probe_pylon_successor.v1`.
- Already green: `pylon.cli_tui_probe_background.v1`.
- **Uncovered until this epic:**
  - `pylon.compute_revenue_modes.v1` (red) —
    `live_gepa_network_missing`, `sellable_local_inference_missing`,
    `remote_qwen_training_missing`
  - `pylon.first_real_model_training_run.v1` (red) —
    `remote_multi_device_training_missing`,
    `qwen_training_postponed_after_gepa`

Adjacent but not in scope: `energy.flexible_load_proof.v1` (planned) will
eventually consume the capacity-funnel measurement built in epic 2, but it
is an energy-market promise, not a Pylon promise; it stays in the planned
set pending the schedule-or-keep decision recommended in the trajectory
post.

## The Good News: The Gates Are Already Built

Like both prior epics, the claim-gating machinery for this frontier already
exists in the worker — what is missing is runs and the Pylon↔Psionic
boundary:

- **Qwen remote fine-tune gate** (`qwen-remote-pylon-finetune-gate.ts`,
  tested): requires ≥2 distinct `remote_pylon` worker refs, signed worker
  receipts, shard receipts (no quarantined shards), artifact/merge/eval/
  admission refs, public projection refs, payment refs, and settlement refs
  for settled claims. It cleanly separates
  `qwenRemoteBoundedTrainingClaimAllowed` (sampled-projection LoRA with
  exact scope language) from `qwenRemoteFineTuneClaimAllowed`
  (full-transformer backprop) — so a bounded remote run can clear the
  training claim honestly without overclaiming full fine-tune.
- **GEPA paid-mode campaign ladder** (`probe-gepa-paid-mode-ladder.ts`):
  nine-step ladder from Stage 0 no-spend through
  `payable_pending_settlement` to `settled_bitcoin`, with send-readiness
  preflight and replay-safe duplicate-bridge guards. `settled_bitcoin_ready`
  is the only state allowing settled-bitcoin campaign copy.
- **Stage 0 no-spend campaign gate** (per the 2026-06-08 gap audit):
  multiple Pylons, accepted AND rejected closeouts, artifact/proof/resource/
  verifier refs, Probe closeout imports, Psionic import dry-run refs.
- **Qwen3.5 local inference roadmap** (`apps/pylon/docs/2026-06-09-…`): the
  attach-only `psionic_qwen35` backend profile, doctor surface,
  OpenAI-compatible chat/tool-call client with streaming parser and
  redacted receipts, and 0.8B/2B model-row admission gates are already
  implemented in the pylon runtime. Pylon does not bundle weights or
  download on startup; absent Psionic reports
  `connector_unconfigured` honestly.
- **Psionic connection audit** (`apps/pylon/docs/2026-06-09-pylon-psionic-
  ml-connection-audit.md`): defines the exact ownership split and the
  missing boundary — Psionic binary/service discovery, capability
  negotiation, signed release-manifest verification, sidecar lifecycle,
  content-addressed model artifact download, training assignment execution,
  and worker receipts imported into Pylon closeouts.
- **Psionic itself** (sibling repo) already has: real Qwen weight loads,
  local loopback two-worker LoRA rehearsals with signed worker receipts,
  payable decisions, adapter merge, eval, and the OpenAI-compatible server
  the inference rows attach to.

## Promise-By-Promise Status And Path

### `pylon.compute_revenue_modes.v1` (red)

Claim: compute revenue includes local model inference, GEPA optimization
slices, and Qwen fine-tuning on people's devices.

1. **`sellable_local_inference_missing`** — closest. Epic 1's #4638/#4641
   make Apple FM inference sellable via NIP-90 kind 5050 with settled
   receipts; this epic adds the Psionic-backed Qwen3.5 rows as a second
   sellable backend. With both receipted, the blocker clears.
2. **`live_gepa_network_missing`** — the ladder exists; #4642 (epic 1)
   gives single-assignment paid GEPA settlement on v0.3. What remains is a
   *campaign*: Stage 0 no-spend green with multiple real Pylons, then the
   paid ladder through settled_bitcoin with replay-safe receipts.
3. **`remote_qwen_training_missing`** — shared with the training promise;
   cleared by the bounded remote run below (with exact scope language).

### `pylon.first_real_model_training_run.v1` (red)

Claim: Pylon starts the first real model-training run.

1. **`qwen_training_postponed_after_gepa`** — a sequencing blocker by
   design: the v0.3 launch path is GEPA-first and the Qwen track is
   explicitly postponed (`supportsTraining: false` is the current honest
   guard). It clears when the GEPA lane is live (the campaign above), at
   which point training re-opens.
2. **`remote_multi_device_training_missing`** — the substance. Psionic's
   loopback rehearsals do not satisfy the remote-device claim (the gate
   says so explicitly). Needed: the Pylon↔Psionic training boundary, then
   one bounded remote run on ≥2 real contributor devices producing the full
   gate bundle (workers, shards, merge/eval/admission, payment, settlement,
   projection). The gate's verification field is exactly that bundle, so a
   receipt-backed bounded run with exact scope language can take this
   promise to green honestly — full-transformer fine-tune remains its own
   separately-gated claim and is **not** promised here.

## Repo Boundary (Important For Delegated Agents)

Psionic is the ML execution substrate and lives in its own repo. Issues in
this epic are filed in `OpenAgentsInc/openagents` and scope to what this
monorepo owns: the Pylon connector/contracts, worker gates, smokes,
receipts, and public projections. Where a step consumes Psionic-side work
(serving endpoints, training jobs, signed manifests), the issue names that
as an **external dependency to flag**, not something to build here. Do not
port Psionic internals into this repo.

## Implementation Plan: GitHub Issues

**Filed on GitHub 2026-06-10 as #4664–#4671**, one per plan step in order.
Bodies are self-contained with the Delegation Contract inlined; owner
posture is default-yes (recorded 2026-06-10).

**Issue 1 — `pylon: Psionic connector contract (discovery, negotiation, refusal states)`**

> The typed boundary from the connection audit: Psionic binary/service
> discovery, capability negotiation, connector state projection, and honest
> refusal reasons (`connector_unconfigured` etc.). No model downloads on
> normal startup; a normal install stays small. This is the foundation both
> the inference rows and the training boundary attach to.

**Issue 2 — `pylon: psionic_qwen35 attach-only inference rows complete and admitted`**

> Close the remaining gap on the roadmap's implemented base (backend
> profile, doctor, tool-call client, 0.8B/2B admission): installer-flow and
> assignment-gating pieces, end-to-end doctor→attach→inference smoke
> against a live Psionic OpenAI-compatible server, redacted receipts.

**Issue 3 — `compute: sell Psionic-backed Qwen3.5 inference through the compute market`**

> Wire the qwen3.5 rows as a second sellable NIP-90 kind 5050 backend
> behind GO ONLINE (Apple FM is first, #4638). Settle one paid Qwen-backed
> inference job to a contributor wallet with a public receipt. With the
> Apple FM compute receipts (#4641), propose the
> `sellable_local_inference_missing` clear receipt-first.

**Issue 4 — `gepa: Stage 0 no-spend campaign green on multiple real Pylons`**

> Run the existing Stage 0 campaign gate to green with ≥2 real registered
> Pylons (not loopback): accepted and rejected closeouts, artifact/proof/
> resource/verifier refs, Probe closeout imports, Psionic import dry-run
> refs, public summary refs. No spend; this is the campaign's wiring proof.

2026-06-11 update: the repeatable operator runbook and smoke verifier now live
at `apps/openagents.com/docs/2026-06-11-probe-gepa-stage0-live-campaign-runbook.md`
and `bun run smoke:probe-gepa-stage0` from `apps/openagents.com/workers/api`.
The issue remains open because the current public fleet recheck found no two
fresh, non-synthetic Pylons advertising `cap.gepa.retained.v1`.

**Issue 5 — `gepa: paid campaign through the payment-mode ladder to settled_bitcoin`**

> Take the Stage 0 campaign through the nine-step ladder:
> `payable_pending_settlement` then `settled_bitcoin`, with payment receipt
> refs, settlement receipt refs, send-readiness preflight, live-small-sats
> smoke ref, and replay-safe duplicate-bridge evidence. Clears
> `live_gepa_network_missing`; also satisfies the GEPA-first sequencing so
> `qwen_training_postponed_after_gepa` is proposed for clear in the same
> pass. Operator funds the payer (Lane B).

**Issue 6 — `pylon: training assignment boundary through Psionic (sidecar, manifests, artifacts, receipts)`**

> The training half of the connector: signed Psionic release-manifest
> verification, sidecar lifecycle, content-addressed model artifact
> download, training assignment execution through Psionic, worker receipts
> imported into Pylon closeouts, and launch gates that distinguish GEPA
> text optimization from neural training. Psionic-side job execution is an
> external dependency to flag, not to build here.

**Issue 7 — `training: bounded remote Qwen run on two real devices through the fine-tune gate`**

> The frontier run. ≥2 distinct remote Pylon workers on real contributor
> devices execute a bounded (sampled-projection LoRA class) Qwen training
> assignment through the issue-6 boundary, producing the complete gate
> bundle: signed worker receipts, required shard receipts (no quarantine),
> artifact/merge/eval/admission refs, payment receipt refs, settlement
> receipt refs, public projection refs —
> `qwenRemoteBoundedTrainingClaimAllowed: true` with the gate's exact scope
> language. Clears `remote_multi_device_training_missing` and
> `remote_qwen_training_missing` for the bounded claim; full-transformer
> copy stays blocked by design. Lane B (operator funds worker payments).

2026-06-11 update: the repeatable operator runbook and smoke verifier now live
at `apps/openagents.com/docs/2026-06-11-qwen-remote-pylon-live-training-runbook.md`
and `bun run smoke:qwen-remote-training` from
`apps/openagents.com/workers/api`. The issue remains open because the current
public fleet recheck has no two real non-synthetic Pylons advertising
`capability.public.pylon.fine_tuning_training`, and there are no public signed
worker/shard/merge/eval/admission/payment/settlement refs for the Lane B
bounded run.

**Issue 8 — `cluster: training/compute-modes verification sweep and registry proposals`**

> The wrap. Verify both promises' blockers against live state in one pass;
> propose transitions receipt-first: `pylon.compute_revenue_modes.v1`
> red → yellow → green per blocker, and
> `pylon.first_real_model_training_run.v1` red → green via the bounded-run
> bundle **with exact scope language in the safeCopy** (the claim is "Pylon
> starts the first real model-training run" — a receipt-backed bounded
> remote run on real devices is honestly that; full fine-tune claims remain
> separately gated). Forum wrap-up in product-promises.

2026-06-11 sweep update: no registry transition is proposed. The live registry
still reports `pylon.compute_revenue_modes.v1` red with
`live_gepa_network_missing`, `sellable_local_inference_missing`, and
`remote_qwen_training_missing`, and
`pylon.first_real_model_training_run.v1` red with
`remote_multi_device_training_missing` and `qwen_training_postponed_after_gepa`.
The current honest remainder is now concrete and repeatable:
`bun run smoke:probe-gepa-stage0` verifies the Stage 0 GEPA blocker,
`bun run smoke:qwen-remote-training` verifies the bounded remote Qwen
training blocker, and the paid GEPA/Qwen lanes still require operator-approved
Lane B spend plus public-safe payment and settlement refs before either
promise can move.

**Lane map (plan step → issue → lane → primary surfaces → depends on):**

| Plan | Issue | Lane | Primary surfaces | Depends on |
|------|-------|------|------------------|-----------|
| 1 | #4664 | A | `apps/pylon` connector module + runtime contracts | — |
| 2 | #4665 | A (needs a Psionic server to attach to — external dep) | `apps/pylon/packages/runtime` backends, doctor | #4664 |
| 3 | #4666 | B (small-sats buy) | pylon provider loop + smoke scripts, registry files | #4665, #4638 |
| 4 | #4667 | A/B (real devices, no spend) | campaign smoke scripts, worker Stage 0 gate surfaces | #4638 or #4656 presence |
| 5 | #4668 | B (operator funds payer) | ladder smoke scripts, registry files | #4667, #4642 |
| 6 | #4669 | A (Psionic-side jobs are external deps) | `apps/pylon` connector training half, launch gates | #4664 |
| 7 | #4670 | B (operator funds worker payments) | training smoke scripts, finetune-gate evidence, registry files | #4669, #4668 |
| 8 | #4671 | B (verification + Forum post) | registry files, Forum | #4664–#4670 |

Registry-touching issues (3, 5, 7, 8) serialize registry commits with the
registry-touching issues of epics 1–2.

## Expected Registry Motion

- `pylon.compute_revenue_modes.v1`: red → yellow when
  `sellable_local_inference_missing` clears (issue 3 + #4641 receipts);
  → green when the paid GEPA campaign (issue 5) and the bounded remote run
  (issue 7) land.
- `pylon.first_real_model_training_run.v1`: `qwen_training_postponed_after_
  gepa` clears with issue 5; red → green with issue 7's receipt-backed
  bounded bundle and exact scope language. Full-transformer fine-tune is
  never claimed by this promise's green copy.

With epics 1–3 filed, **every Pylon and training promise in the registry
has a complete issue path**: 37 issues across #4635–#4671.

## Evidence Reviewed

- Registry records for both promises (`product-promises.ts`, 2026-06-10.4)
- `apps/openagents.com/docs/2026-06-08-qwen-remote-pylon-finetune-gate.md`
  (+ `qwen-remote-pylon-finetune-gate.ts` and its tests)
- `apps/openagents.com/docs/2026-06-08-probe-gepa-paid-mode-campaign-ladder.md`
  (+ `probe-gepa-paid-mode-ladder.ts`)
- `apps/openagents.com/docs/2026-06-08-probe-gepa-stage0-no-spend-campaign-gate.md`
  (via the 2026-06-08 gap audit)
- `apps/pylon/docs/2026-06-09-pylon-psionic-ml-connection-audit.md`
- `apps/pylon/docs/2026-06-09-pylon-qwen35-local-inference-roadmap.md`
- Epic 1 and epic 2 audits and their filed issues (#4635–#4663)
- Live: `GET /api/public/product-promises` (2026-06-10.4),
  `GET /api/public/pylon-stats`
