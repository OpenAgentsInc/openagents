# business.coding_quick_win.v1 — machine-checkable payment gate

(Appending to the delivery work tracked in `docs/launch/vertex-fleet/business.coding_quick_win.v1.md`)

## What this change adds

A machine-checkable **payment-evidence contract** for a business quick win:

- `apps/openagents.com/workers/api/src/business-quick-win-payment.ts`
- `apps/openagents.com/workers/api/src/business-quick-win-payment.test.ts`

The promise claims that business quick wins are packaged as a priced product. The generic receipt module treats the `buyer_paid` state as an opaque string. This module makes that judgement deterministic and self-checkable for a self-serve loop:

- `buildBusinessQuickWinPaymentEvidence` records the payment amount, currency, status, and reference. It ensures the status is explicitly tracked.
- `businessQuickWinPaidEvidenceRef` produces the `buyerPaidRef` string for the receipt ONLY for a settled payment. It rejects pending or failed payments, ensuring the loop never claims `buyer_paid` prematurely.

## Which blocker this advances

`blocker.product_promises.business_coding_quick_win_paid_receipt_missing` (partial).

The self-serve loop now has machine-checkable gates for all lifecycle evidence steps (delivery, acceptance, and payment), setting the foundation for generating a verifiable paid receipt automatically without operator intervention.

## What genuinely remains (blockers stay listed)

- Self-serve: `blocker.product_promises.business_coding_quick_win_self_serve_missing` (automated intake -> repo provisioning -> runtime invocation -> the delivery, acceptance, and payment gates -> receipt).
- Paid receipt: `blocker.product_promises.business_coding_quick_win_paid_receipt_missing` (a real first paid coding-quick-win customer receipt per `proof.claim_upgrade_receipts.v1`).

---

# business.coding_quick_win.v1 — machine-checkable repo provisioning

## What this change adds

A machine-checkable **repo provisioning** contract for the self-serve loop:

- `apps/openagents.com/workers/api/src/coding-quick-win-provisioning.ts`
- `apps/openagents.com/workers/api/src/coding-quick-win-provisioning.test.ts`

To reach self-serve delivery, the loop must translate a scoped request into an isolated worktree for the runtime. This module enforces the invariant that a repository is only `provisioned` (and thus ready for runtime invocation) if it has locked a specific `baseCommitSha` and exposes a verifiable `worktreeRef`.

## Which blocker this advances

`blocker.product_promises.business_coding_quick_win_self_serve_missing` (partial).

This explicitly addresses the "repo provisioning" step.

## What genuinely remains (blockers stay listed)

- Self-serve: `blocker.product_promises.business_coding_quick_win_self_serve_missing` (the gap is now just runtime invocation, and orchestrating these deterministic steps into a pipeline).
- Paid receipt: `blocker.product_promises.business_coding_quick_win_paid_receipt_missing` (a real first paid coding-quick-win customer receipt per `proof.claim_upgrade_receipts.v1`).
