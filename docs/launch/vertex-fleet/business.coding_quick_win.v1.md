# business.coding_quick_win.v1 — coding-quick-win delivery-evidence contract

Promise: `business.coding_quick_win.v1` (yellow — leave as-is, no state flip).

## What this change adds

A machine-checkable **delivery-evidence contract** for a coding quick win:

- `apps/openagents.com/workers/api/src/coding-quick-win-delivery.ts`
- `apps/openagents.com/workers/api/src/coding-quick-win-delivery.test.ts`

The promise claim is "a written objective is taken into a repository, the
customer's verification command is run, and a reviewable change is handed back
with verification evidence." The generic business-quick-win receipt
(`business-quick-win-receipt.ts`) treats the `delivered_with_evidence` state as
an opaque `deliveredEvidenceRef` string — fine for an operator eyeballing a PR
link, but a self-serve loop cannot rely on a human to judge whether a delivery
is real.

This module makes that judgement deterministic and self-checkable:

- `buildCodingQuickWinDeliveryEvidence` records repo, base ref, the customer's
  verification command (verbatim), the command exit code + captured output ref,
  and a reviewable diff ref. It **derives** `verificationStatus` from the exit
  code (0 → passed, non-zero → failed, missing → not_run); a caller can never
  assert "passed" without a real 0 exit, and a "passed" run must carry captured
  output so it can be re-checked.
- `acceptableForHandback` is the single honest boolean a self-serve loop gates
  on: true only for a passed verification with a diff.
- `assertCodingQuickWinDeliverable` / `codingQuickWinDeliveredEvidenceRef`
  produce the `deliveredEvidenceRef` for the receipt **only** for a
  handback-ready delivery, so a receipt can never claim
  `delivered_with_evidence` for a failed or empty delivery.
- The default `reviewGateCaveatRef` keeps "reviewable, not merged" — no
  auto-merge/deploy authority — visible in the public projection.

## Which blocker this advances

`blocker.product_promises.business_coding_quick_win_self_serve_missing`
(partial). The self-serve delivery loop needs to validate its own output
without an operator; this is the gate that lets it do so, and the bridge from
the green coding runtime to the existing business-quick-win receipt.

It also tightens `business_coding_quick_win_paid_receipt_missing` indirectly:
the receipt's `delivered_with_evidence` reference is now produced by a verifier
rather than a hand-pasted string.

## What genuinely remains (blockers stay listed)

- Self-serve: automated intake → repo provisioning → runtime invocation → this
  delivery gate → receipt, with pricing and a customer-facing surface. This
  change only supplies the delivery-acceptance gate, not the end-to-end loop.
- Paid receipt: a real first paid coding-quick-win customer receipt
  (intake → delivery → accepted outcome → buyer_paid), per
  `proof.claim_upgrade_receipts.v1`. No paid run exists yet; no state flips.

## Validation

- `bunx tsc -p tsconfig.json --noEmit` in `workers/api`: clean.
- `bun run check:deploy` in `apps/openagents.com`: see commit notes.
