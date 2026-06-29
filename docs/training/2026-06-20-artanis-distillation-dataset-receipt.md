# Artanis Tassadar Distillation Dataset Receipt

Date: 2026-06-20

Promise: `artanis.tassadar_evolution_loop.v1`

This records the refs-only dataset-curation receipt for the first Tassadar
distillation dataset manifest derived from Artanis administrator ticks.

The source material is the existing Artanis admin executor-trace closeout
ledger:

- `GET /api/public/artanis/tick-streak` reports the sustained unattended
  tick-streak gate as met on production: `longestStreak` 12, `targetReached`
  true, and 16 verified tick closeouts in the scanned window.
- Each qualifying source row is a dispatched Artanis admin assignment with an
  exact-replay closeout verdict: `outcome=verified` and
  `accept_state=accepted`.
- Each source row dereferences through the public Nexus/Pylon receipt route,
  for example
  `receipt.nexus_pylon.artanis_admin_closeout.assignment.artanis_admin.20260616123548`.

The new projection is:

- `GET /api/public/artanis/tassadar-distillation-dataset`
- Source file:
  `apps/openagents.com/workers/api/src/artanis-distillation-dataset-receipt.ts`
- Test file:
  `apps/openagents.com/workers/api/src/artanis-distillation-dataset-receipt.test.ts`

The receipt is deliberately narrow. It is a public-safe manifest of verified
trace refs and digest prefixes, not a raw trace export. It exposes no private
runner logs, prompts, provider payloads, wallet material, customer data, or
secret-bearing values. It grants no dispatch, spend, assignment, settlement,
model-training, eval, model-promotion, or registry-transition authority.

Acceptance rule:

- The receipt is `available` only when at least 10 accepted exact-replay
  Artanis closeouts are present.
- When available, it clears
  `blocker.product_promises.tassadar_distillation_dataset_receipt_missing`.
- The promise still requires owner sign-off for any future green transition per
  `proof.claim_upgrade_receipts.v1`.
