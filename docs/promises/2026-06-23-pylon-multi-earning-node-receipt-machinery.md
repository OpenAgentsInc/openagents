# Pylon multi-earning-node — cross-mode receipt machinery + capture runbook

Date: 2026-06-23
Branch: `feat/de4-pylon-promise`
Promise: `pylon.v0_3_multi_earning_node.v1` (DE-4, EPIC #5527)
Blocker addressed: `blocker.product_promises.multi_earning_mode_receipts_missing`
HARD RULE: this does NOT flip the promise green. Green stays owner-signed per
`proof.claim_upgrade_receipts.v1`, with a real settled receipt from each of >=2
earning modes captured from one install.

## Why this promise / blocker

The multi-earning-node promise needs "settled receipts for >=2 earning modes in
one install" (assessment doc `2026-06-19-pylon-non-green-promise-assault-assessment.md`,
row 6). Pylon already produces per-mode earning records, but each lives in its
OWN type-specific store with its own shape:

- NIP-90 provider earnings — `ProviderEarningRecord` (`src/provider-nip90.ts`)
- assignment closeouts — `AssignmentCloseout.receiptRefs` (`src/assignment.ts`)
- training worker receipts — `TrainingWorkerReceipt` (`src/assignment.ts`)
- forum tips — `src/tips.ts`; Tassadar executor; data; labor; referral

There was no unified, dereferenceable interface that reads those per-mode records,
distinguishes amount classes, and HONESTLY counts how many distinct modes carry a
settled receipt from one install. That missing interface IS the named blocker.
(The public-projection blocker `safe_public_projection_missing` is a separate,
workers/api-side deliverable owned by another lane and is untouched here.)

## What this change builds (contributor-node surface only)

`apps/pylon/src/multi-earning-ledger.ts` — pure, INERT, flag-gated:

- A typed per-mode earning entry (`MultiEarningLedgerEntry`) over a closed mode
  taxonomy (`compute`, `labor`, `tips`, `data`, `referral`, `inference`) and the
  five amount classes (`modeled`/`observed`/`pending`/`paid`/`settled`).
- `summarizeMultiEarning(entries, observedAt, opts)` — audits every entry with a
  closed-key public-safety guard, rolls valid entries per mode + amount class,
  and counts how many DISTINCT modes carry a `settled` receipt. Only `settled`
  counts toward the `>=2` bar. INERT by default (`PYLON_MULTI_EARNING_LEDGER_ENABLED`
  off → empty projection, zero settled modes, promise red).
- `captureMultiEarningReceipt(...)` — fail-closed: emits a self-verifying,
  round-trip-clean `MultiEarningReceipt` ONLY when the `>=2` bar is honestly met;
  otherwise returns `captured: false` with reasons and emits nothing.
- `verifyMultiEarningEntry` / `verifyMultiEarningReceipt` — deterministic auditor
  gates a reviewer runs over untrusted captured artifacts.

CLI: `pylon multi-earning ledger --json` — read-only, no-network. Emits the honest
projection (INERT by default). It ingests no live earning records yet (fail-safe);
the live earning paths feed `summarizeMultiEarning` only when an operator wires
real settled receipts.

Public-safety by construction: a closed key allowlist rejects any entry/receipt
that would smuggle a raw address, balance, invoice, mnemonic, credential, or local
path. Amounts are carried only as non-negative integer sats (an aggregate the
public surfaces already expose), never raw payment material.

## Dereferenceable receipt (re-runnable now)

- `bun test apps/pylon/src/multi-earning-ledger.test.ts` — 23 tests: distinguishes
  all five amount classes, only counts settled modes toward the bar, stays inert
  by default, rejects leak-prone entries (counting them, never crediting), and the
  capture is fail-closed + round-trip-clean.
- `bun apps/pylon/src/index.ts multi-earning ledger --json` — emits the honest
  INERT projection (`inert: true`, `settledModeCount: 0`, `promiseState: "red"`,
  the three remaining owner-gated blockers named).

## Exact remaining owner step (single arm/flip)

The machinery is built and proven up to the live earning event. To reach green,
the owner (one arm/flip):

1. On one install, set `PYLON_MULTI_EARNING_LEDGER_ENABLED=1` and wire the live
   per-mode earning sources to feed `summarizeMultiEarning` (the existing
   `ProviderEarningRecord` / `AssignmentCloseout.receiptRefs` / tips records, each
   mapped to a `MultiEarningLedgerEntry` with `amountClass: "settled"`).
2. Earn and SETTLE a real receipt in >=2 distinct modes from that one install
   (e.g. a settled Tassadar/compute receipt + a settled forum-tip receipt). This
   requires real compute / a real earning event + operator spend/settlement
   approval — owner-gated.
3. Run `captureMultiEarningReceipt(...)`; confirm `captured: true` and that the
   serialized artifact re-audits clean via `verifyMultiEarningReceipt`.
4. Owner-sign the green flip per `proof.claim_upgrade_receipts.v1`, citing the
   captured `receipt.pylon.multi_earning_node.modes_N.v0.1` ref.

No money moved, no install claimed closed, no green flipped here. The projection
is INERT by default and honest when armed (never reports a settled mode the store
did not carry).
