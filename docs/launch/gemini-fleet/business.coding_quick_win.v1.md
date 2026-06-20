# business.coding_quick_win.v1 — self-serve acceptance gate

(Appending to the delivery work tracked in `docs/launch/vertex-fleet/business.coding_quick_win.v1.md`)

## What this change adds

A machine-checkable **acceptance-evidence contract** for a coding quick win:

- `apps/openagents.com/workers/api/src/coding-quick-win-acceptance.ts`
- `apps/openagents.com/workers/api/src/coding-quick-win-acceptance.test.ts`

Just as the previous delivery-gate change allowed the self-serve loop to generate a `delivered_with_evidence` reference without an operator, this new module allows the self-serve loop to deterministically evaluate whether the outcome was accepted (`outcome_accepted` in `business-quick-win-receipt.ts`).

- `buildCodingQuickWinAcceptanceEvidence` derives `isAccepted` based on an explicit customer action (`diff_approved` or `diff_merged`).
- `codingQuickWinAcceptedEvidenceRef` acts as the verifier gate, returning the attestation reference (e.g. PR review URL or merge SHA) ONLY if the customer action qualifies as an acceptance.

## Which blocker this advances

`blocker.product_promises.business_coding_quick_win_self_serve_missing` (partial). 

The self-serve loop now has machine-checkable gates for both the delivery and the outcome acceptance steps, allowing it to feed the core receipt module deterministically.

## What genuinely remains (blockers stay listed)

- Self-serve: automated intake -> repo provisioning -> runtime invocation -> the delivery and acceptance gates -> receipt.
- Paid receipt: `blocker.product_promises.business_coding_quick_win_paid_receipt_missing` (a real first paid coding-quick-win customer receipt per `proof.claim_upgrade_receipts.v1`).
