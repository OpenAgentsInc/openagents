# Self-serve labor earning projection

Advance `provider.compliant_usage_labor.v1` by addressing `blocker.product_promises.labor_self_serve_earning_missing`.

## What was built
Added a public-safe Labor Earnings projection and the `/api/public/labor-earnings` readback endpoint (`workers/api/src/labor-earnings.ts`, `workers/api/src/labor-earnings-routes.ts`). 

This creates the missing *readback* surface that allows a contributor to self-serve view their labor earnings without an operator in the loop, providing a feed of their NIP-LBR escrow-release receipts to show actual proof of payment before tip-ladder sweeping. 

## What remains
The promise remains yellow because the external-ladder settlement has not yet run in production for a labor job (the first job settled on the credit ledger). The remaining blocker `blocker.product_promises.labor_external_ladder_settlement_missing` must be cleared by a live execution whose payout triggers the external Lightning sweep.
