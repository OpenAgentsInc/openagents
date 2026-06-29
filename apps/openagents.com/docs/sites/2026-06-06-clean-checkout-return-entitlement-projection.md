# Clean Checkout Return And Entitlement Projection

Issue #304 adds the clean checkout return contract for generated Sites.

The implementation lives in `workers/api/src/site-checkout-return.ts` with
coverage in `workers/api/src/site-checkout-return.test.ts`.

## Contract

Generated Sites can project checkout return state from:

- a clean success, cancel, or status route;
- a server-side checkout intent ref;
- a buyer payment challenge;
- an optional hosted MDK checkout projection;
- an optional receipt;
- an optional entitlement;
- generated checkout UI primitives.

The projection supports these states:

- `success`;
- `cancel`;
- `pending`;
- `unpaid`;
- `paid`;
- `entitled`;
- `expired`;
- `blocked`.

## Clean URL Boundary

The contract accepts only local clean paths. It rejects public checkout result
query strings or fragments such as `checkout_id`, along with non-local paths.

Return paths should be stable paths such as:

```text
/checkout/thanks
/checkout/status
/pricing
```

Do not derive entitlement or receipt authority from public URL state.

## Entitlement Boundary

A clean success return does not create final entitlement authority. The
projection reports `pending_reconciliation` until OpenAgents product surface has a receipt or
entitlement record.

State interpretation:

- `success`: the clean success route was reached, but reconciliation still
  needs to decide whether a receipt or entitlement exists;
- `paid`: a receipt or hosted checkout payment-received state exists, but no
  final entitlement projection is active yet;
- `entitled`: an active entitlement record exists;
- `expired`: the buyer payment challenge is stale or explicitly expired;
- `blocked`: route refs, checkout refs, challenge refs, or payment product refs
  do not match.

## Redaction Boundary

The projection rejects raw invoices, payment preimages, wallet state, MDK
credentials, provider grants, provider payout claims, customer private data,
checkout query state, source archives, runner logs, and secrets.

## Verification

- `bun run --cwd workers/api test -- src/site-checkout-return.test.ts`
- `bun run --cwd workers/api test`
- `bun run --cwd workers/api typecheck`
- `bun run check:architecture`
