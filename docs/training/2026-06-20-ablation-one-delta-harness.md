# Training ablation one-delta harness

Date: 2026-06-20

Promise: `training.ablation_system.v1` (stays **planned** — no green flip).

## What this advances

The public ablation derisking ledger now uses a typed one-delta manifest
verifier before an entry can report `oneDeltaManifestState:
manifest_verified`.

The harness accepts a manifest only when it carries exactly one delta, all
manifest refs are public-safe, and the serialized manifest contains no private
paths, wallet/payment material, raw prompts, provider payloads, or secret-shaped
strings. Multi-delta manifests fail closed before projection.

This clears:

- `blocker.product_promises.ablation_harness_missing`

## What stays blocked

This is a manifest-shape and projection advance only. No ablation has run, no
eval suite was reproduced, no paid assignment was dispatched, no verdict was
accepted, no checkpoint was promoted, and no money moved.

Remaining blockers:

- `blocker.product_promises.eval_suite_reproduction_missing`
- `blocker.product_promises.paid_ablation_dispatch_missing`

## Evidence

- `apps/openagents.com/workers/api/src/training-ablation-derisking-ledger.ts`
- `apps/openagents.com/workers/api/src/training-ablation-derisking-ledger.test.ts`
- `GET /api/public/training/ablation-derisking-ledger`

## Scope boundary

The harness proves only that an ablation candidate is represented as exactly
one intended change against a frozen baseline ref set and fixed eval-plan refs.
It grants no dispatch, spend, settlement, model-promotion, or public-claim
authority.
