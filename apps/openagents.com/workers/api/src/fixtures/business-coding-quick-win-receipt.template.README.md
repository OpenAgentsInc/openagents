# Business Coding Quick Win — paid receipt template

Promise: `business.coding_quick_win.v1`
Blocker:  `blocker.product_promises.business_coding_quick_win_paid_receipt_missing`

`business-coding-quick-win-receipt.template.json` is the **canonical,
public-safe SHAPE TEMPLATE** for the per-run paid customer receipt evidence
document that the documented remaining step for this blocker consumes:

> run `assertFirstPaidQuickWinReceipt` against the live run's REAL
> receipt file and cite the successful check.

It exists because the verifier + parse boundary
(`src/business-quick-win-receipt.ts`) already define the document SHAPE in
code, but there was no checked-in template an auditor could copy to assemble the
real evidence file, and no test exercising the real **file → parse → verify**
path (every other test builds objects in-memory). This template + its disk-load
test (`business-quick-win-receipt.test.ts`, the "evidence document
template" suite) close that gap.

## What this is NOT (honesty)

- It is **synthetic**. Every ref is a self-evident placeholder
  (`signup.example.coding_quick_win_1`, `spec.example…`, `payment.example…`). It is
  **not** the live run's evidence and does **not** assert that the real run
  conforms or that any real payment was made.
- It does **not** clear the blocker or flip any promise state. Clearing the
  blocker still requires dropping in the run's REAL per-customer receipt,
  citing the successful check, plus owner sign-off
  (receipt-first per `proof.claim_upgrade_receipts.v1`).

## Constraints the template honors (so it parses)

The parse boundary enforces a **closed key allowlist** at every level
and rejects any extra field — that is how a
leaked raw address / balance / credential is kept out of a published evidence
artifact. So this template carries ONLY the allowed keys; you cannot annotate the
JSON itself with a "this is a template" field (it would be rejected). That framing
lives here instead.
