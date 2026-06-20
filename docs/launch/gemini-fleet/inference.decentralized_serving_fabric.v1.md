# inference.decentralized_serving_fabric.v1 — gemini fleet worker note

Date: 2026-06-20. Promise state: **red** (unchanged — no state flip in this change).

## Blocker advanced

`blocker.product_promises.shard_wan_large_model_serving_psionic_planned`

Advanced this blocker by building the missing RL-2/RL-3 settlement wiring for the product-side payout-split hook implemented against `psionic.serve.pipeline_sharded_run_receipt.v1`.

## What I built

- Added `buildShardWanServingPayoutPayInPlan` to `apps/openagents.com/workers/api/src/shard-wan-serving-payout-split.ts`. This function takes the pure, deterministic `ShardWanServingPayoutDecision` along with a stage-to-node identity map, and builds a `PayInPlan` that wires the split directly into the RL-2/RL-3 ledger settlement spine.
- Enforced the owner-armed gate directly within the settlement wiring: the function explicitly requires `ownerArmed: true` and refuses to build a live PayInPlan if the first payout gate is not armed.
- Added 3 test cases for the integration in `apps/openagents.com/workers/api/src/shard-wan-serving-payout-split.test.ts`.

## What remains (still red)

- A real Psionic shard-WAN run emitting a hardware-backed `psionic.serve.pipeline_sharded_run_receipt.v1` (Psionic-planned / hardware-blocked).
- A Pylon serving an actual gateway inference request (whole-small-model lane and the fabric supply adapter are also still unbuilt).
- The open product decision on the weighting rule (this ships the documented per_layer_block default; FLOP-aware / latency-contribution remain open).
