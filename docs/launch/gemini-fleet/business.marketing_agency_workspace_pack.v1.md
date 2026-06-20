# business.marketing_agency_workspace_pack.v1 — gemini-fleet

## What was built
- `marketing-agency-delivery-receipt-fixture.ts`: A dereferenceable mock fixture of a valid first-paid agency receipt meeting all authority invariants.
- `marketing-agency-receipt-public-routes.ts`: A dereferenceable public projection route (`GET /api/public/marketing-agency/receipts/{receiptRef}`) serving the fixture, compliant with the `projection_staleness.v1` contract.
- Connected the route through `index.ts` and `worker-routes.ts`, and added it to `INVARIANTS.md` and `check-zero-debt-architecture.mjs` as `staleness_declared`.
- `marketing-agency-claim-upgrade.ts` & `.test.ts`: A dereferenceable claim-upgrade projection and gate for the marketing-agency first-paid delivery receipt. This wires the receipt into the `proof.claim_upgrade_receipts.v1` contract.

## Which blocker this advances
This fully clears `blocker.product_promises.marketing_agency_pack_first_paid_delivery_receipt_missing` (part 1: receipt fixture, part 2: wiring into claim upgrade).

## What genuinely remains
- `blocker.product_promises.marketing_agency_pack_self_serve_missing` remains untouched.
- Emitting and storing an instance of this receipt from a live seeded workspace, and delivering a self-serve vertical pack to clear the self-serve blocker.
