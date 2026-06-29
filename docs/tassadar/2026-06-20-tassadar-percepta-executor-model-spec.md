# Tassadar Percepta Executor Model Spec

Date: 2026-06-20

Promise: `models.tassadar_percepta_executor.v1`

This document clears only the model/spec gap for the Tassadar/Percepta
executor-model direction. It is not a trained-model receipt, not an architecture
receipt, and not a Pylon CPU-transform training receipt.

## Model Name And Scope

- Public promise id: `models.tassadar_percepta_executor.v1`
- Lane name: Tassadar Percepta Executor
- Product area: models
- Scope: an executor-class model direction that combines the already proven
  Tassadar exact-replay executor lane with the Percepta-style compiled
  transformer / learned-interface architecture direction.
- Non-scope: a general LLM, an agent, a hosted inference model, a trained
  checkpoint, or a contributor-earning product.

The canonical spelling is Tassadar. The withdrawn typo alias
`models.tasadar_percepta_executor.v1` remains a registry compatibility record
only.

## Runtime Boundary

The model boundary is split into three lanes:

1. Exact executor substrate: digest-pinned Tassadar programs and traces that
   can be independently replayed by the existing exact-trace verifier.
2. Learned interface / student lane: bounded student experiments trained on
   verified trace corpora, evaluated by exact rollout and first-divergence
   reporting.
3. Product-facing execution: Pylon/OpenAgents assignment and verification rails
   that may dispatch work, verify closeouts, and publish public-safe receipts.

Only lane 1 has a green bounded proof today via `compute.tassadar_executor_poc.v1`.
Lane 2 has research evidence from the W3 student-program report, not a product
model. Lane 3 has live verified-work rails, not a model capability.

## Pylon Integration Shape

Pylon integration, when receipted, must remain assignment-based:

- work is dispatched through existing OpenAgents/Pylon assignment records;
- closeouts carry public-safe artifact/proof/receipt refs;
- verification uses named verifier classes such as `exact_trace_replay`;
- settlement, when real, is recorded separately from model acceptance;
- public projections expose refs and digests, never private runner logs or raw
  provider payloads.

The existing Artanis/Tassadar loop and distillation dataset receipt can supply
verified trace refs, but they do not train or serve this model by themselves.
The architecture receipt route and CPU-transform receipt status route are now
separate public projections; the CPU-transform status route reports the real
training receipt gates as missing.

## Training And Evaluation Plan

The honest path is staged:

1. Curate verified trace refs from Artanis and the public Tassadar executor run.
2. Freeze dataset manifests with trace digest prefixes, program/profile refs,
   executor refs, and verifier refs.
3. Run a bounded student/model rehearsal against the frozen corpus.
4. Evaluate by exact rollout, output digest agreement, and first-divergence
   reports; perplexity is not sufficient.
5. Record architecture receipt(s): model profile, compiled/frozen executor
   components, learned-interface components, checkpoint/config/eval hashes, and
   replay-verifier results.
6. Record Pylon CPU-transform training receipt(s): assignment refs, accepted
   work refs, verification verdict refs, payment/settlement refs when real, and
   public-safe closeout refs.

## Artifact Lineage Requirements

Any future claim must cite public-safe refs for:

- corpus or dataset manifest;
- executor profile and compiler/evaluator hashes;
- model config or compiled architecture bundle;
- checkpoint or interface digest;
- eval report digest;
- exact-replay verdict refs;
- Pylon assignment and closeout refs;
- settlement refs only when real money moved.

## Safety Notes

- Do not claim a trained Tassadar model exists.
- Do not present the W3 student-program report as a product-model receipt.
- Do not present `compute.tassadar_executor_poc.v1` as proof of a general model.
- Do not claim CPU replacement, CPU outperformance, paid earning, hosted
  inference, or model promotion from this spec.
- Do not publish raw traces, private runner logs, provider payloads, wallet
  material, or customer-sensitive data.

## Remaining Blockers

This spec clears:

- `blocker.product_promises.tassadar_model_spec_missing`

Still blocked:

- `blocker.product_promises.tassadar_cpu_transform_real_settlement_missing`
- `blocker.product_promises.tassadar_cpu_transform_owner_green_signoff_missing`

The bounded Pylon v1.0 CPU computation-transform fixture receipt is now
projected at
`/api/public/models/tassadar-percepta-executor/cpu-transform-training-receipts`.
It clears the old missing-receipt blocker for fixture scope only; green still
requires real settlement evidence where money moved where applicable plus
receipt-first owner sign-off under `proof.claim_upgrade_receipts.v1`.
