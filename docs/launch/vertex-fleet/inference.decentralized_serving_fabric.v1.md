# inference.decentralized_serving_fabric.v1 — worker note

Date: 2026-06-20. Promise state: **red** (unchanged — no state flip in this change).

## Blocker advanced

`blocker.product_promises.shard_wan_large_model_serving_psionic_planned`

The shard-WAN large-model serving pipeline is owned by Psionic and is mostly
`planned` / partly hardware-blocked. What is buildable now is the **product-layer
hook** the design doc (`docs/inference/2026-06-19-decentralized-serving-shard-wan.md`
§3c, §8 step 2) and the promise verification both name: the per-stage payout
split *implemented against* `psionic.serve.pipeline_sharded_run_receipt.v1`.

## What I built

- `apps/openagents.com/workers/api/src/shard-wan-serving-payout-split.ts` — a
  pure, deterministic `evaluateShardWanServingPayout(...)` that:
  - decodes a product-side public-safe view of a
    `psionic.serve.pipeline_sharded_run_receipt.v1` receipt (layer-block facts +
    exact-greedy parity result only; no identity, no secrets, no raw activations);
  - enforces the shard-WAN structural invariants before anything is payable:
    more than one stage, every stage GPU-resident (no whole-model fallback faking
    a split), contiguous gap-free overlap-free layer coverage of the whole model,
    and no single stage spanning the entire model;
  - enforces the **born-verified payment gate** — `verified` parity pays,
    `mismatch` is rejected, `no_reference` defaults to HOLD (never pay against
    self-report);
  - apportions the contributor cut **per-layer-block** with the largest-remainder
    method so the split sums to the cut exactly (no sat created or lost);
  - always returns `ownerArmedRequired: true` — `payable` is a necessary, never a
    sufficient, gate. This code dispatches no money.
- `apps/openagents.com/workers/api/src/shard-wan-serving-payout-split.test.ts` —
  12 tests covering the split math, indivisible-cut rounding, parity gating, and
  every structural-invalid case.

## Psionic boundary honored

This is product-layer only: pricing/payout/marketplace/identity stay outside
Psionic, which emits the receipt this code consumes. No serving, routing,
settlement, or public-product-claim authority is granted, and no large-model
fabric claim is made — there is no real Psionic shard-WAN receipt yet.

## What remains (still red)

- A real Psionic shard-WAN run emitting a hardware-backed
  `psionic.serve.pipeline_sharded_run_receipt.v1` (Psionic-planned / hardware-blocked).
- A Pylon serving an actual gateway inference request (whole-small-model lane and
  the fabric supply adapter are also still unbuilt).
- Wiring this split into RL-2/RL-3 settlement and an owner-armed first payout.
- The open product decision on the weighting rule (this ships the documented
  `per_layer_block` default; FLOP-aware / latency-contribution remain open).

Pointer: companion design doc lives at
`docs/inference/2026-06-19-decentralized-serving-shard-wan.md`.
