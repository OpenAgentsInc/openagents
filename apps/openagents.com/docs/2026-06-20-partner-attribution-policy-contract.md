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

## Policy Rules

- `referral` is rejected. The referral rail owns referral payouts.
- `partnerUserId` must differ from `customerUserId`.
- The event time must satisfy `effectiveFromIso <= eventIso` and, when present,
  `eventIso < effectiveUntilIso`.
- When multiple active agreements cover a customer, `design_partner` wins over
  `affiliate`; within a role, the earliest effective agreement wins.
- A missing active agreement records no eligibility. There is no fallback.

## Remaining Blockers

This clears the stale source-level `partner_attribution_policy_missing` blocker.
It does not claim a live partner revenue stream. The promise remains red until
partner settlement dispatch has a dereferenceable public receipt and at least
one real partner payout settles with owner sign-off.
