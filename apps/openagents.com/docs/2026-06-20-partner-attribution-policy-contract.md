# Partner Attribution Policy Contract

This note records the current #5524 partner-attribution boundary for
`autopilot_sites.partner_payout_ledger.v1`.

The partner rail is explicit-agreement-only. A fulfilled paid customer event can
feed the partner payout ledger only when an active `partner_agreements` row
covers the paying customer at the event time. There is no inferred click or
last-touch fallback on this rail; the referral rail owns referral payouts.

## Operator Route

Seed an agreement:

```sh
curl -fsS https://openagents.com/api/operator/partners/agreements \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  --data '{
    "agreementRef": "partner_agreement_example",
    "customerUserId": "github:customer",
    "effectiveFromIso": "2026-06-20T00:00:00.000Z",
    "effectiveUntilIso": null,
    "partnerRef": "design_partner_example",
    "partnerUserId": "github:partner",
    "role": "design_partner"
  }'
```

Read back active agreements for a customer:

```sh
curl -fsS \
  "https://openagents.com/api/operator/partners/agreements?customerUserId=github:customer" \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN"
```

The route is admin-token-gated, idempotent on `agreementRef`, and does not move
money. It records who may be attributed later; payout eligibility is created
only when a real paid event is processed and the active agreement wins the
policy decision.

## Public Receipt Route

Read back a settled partner payout receipt:

```sh
PARTNER_PAYOUT_RECEIPT_REF="receipt.partner_payout.hosted_mdk.example"
curl -fsS \
  "https://openagents.com/api/public/partner-payout-receipts/$PARTNER_PAYOUT_RECEIPT_REF"
```

The route resolves only settled ledger rows whose evidence refs cite the exact
public receipt ref. It returns amount, asset, settlement state, policy refs,
caveats, filtered public evidence refs, and staleness metadata. It does not
expose partner refs, user ids, payout refs, qualifying event refs, payout
destinations, invoices, preimages, provider payloads, wallet material, or ledger
ids.

## Dispatch Route

Operators can drive a sats-denominated partner payout row through the
readiness-gated dispatch coordinator:

```sh
PARTNER_PAYOUT_REF="partner_payout_ref_..."
curl -fsS -X POST \
  -H "authorization: Bearer $OPENAGENTS_ADMIN_BEARER_TOKEN" \
  "https://openagents.com/api/operator/partners/payout-ledger/$PARTNER_PAYOUT_REF/dispatch"
```

The coordinator refuses USD/credit rows before adapter call, refuses while the
owner-armed payout mode is disabled, and records `settled` only after the
injected adapter returns a public-safe `receipt.partner_payout.*` evidence ref.
Default production wiring remains inert/fail-closed until a live partner payout
rail and destination policy are explicitly armed.

## Policy Rules

- `referral` is rejected. The referral rail owns referral payouts.
- `partnerUserId` must differ from `customerUserId`.
- The event time must satisfy `effectiveFromIso <= eventIso` and, when present,
  `eventIso < effectiveUntilIso`.
- When multiple active agreements cover a customer, `design_partner` wins over
  `affiliate`; within a role, the earliest effective agreement wins.
- A missing active agreement records no eligibility. There is no fallback.

## Remaining Blockers

This clears the stale source-level `partner_attribution_policy_missing` and
`partner_payout_settlement_not_wired` blockers, and prepares the dereferenceable
public receipt route. It does not claim a live partner revenue stream. The
promise remains red until at least one real partner payout settles with a real
public receipt and owner sign-off.
