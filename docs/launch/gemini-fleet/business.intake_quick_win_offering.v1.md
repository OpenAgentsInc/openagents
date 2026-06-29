# business.intake_quick_win_offering.v1 — gemini-fleet note

See `docs/launch/vertex-fleet/business.intake_quick_win_offering.v1.md` for the primary context.

I built the `issue-business-quick-win-receipt.ts` operator CLI script to advance `blocker.product_promises.business_first_paid_quick_win_receipt_missing`.

This tool provides the concrete issuance path for an operator to take an intake -> scoped -> delivered -> accepted -> paid quick win and deterministically generate the required first paid quick-win receipt.

What remains: An operator must actually *run* this tool on a real paid run to generate the dereferenceable receipt fixture and commit it, completing the proof for the blocker.
