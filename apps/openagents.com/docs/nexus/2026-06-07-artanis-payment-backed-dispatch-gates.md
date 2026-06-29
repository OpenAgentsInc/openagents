# Artanis payment-backed Nexus/Pylon dispatch gates

Date: 2026-06-07

Status: implemented as the OpenAgents product surface #428 contract slice. This is still a gated
simulation/control-plane path, not a public Pylon v0.2 release gate and not a
blanket grant of live bitcoin spend authority.

## What changed

`workers/api/src/artanis-nexus-pylon-adapters.ts` now models payment authority
state directly on Artanis Nexus/Pylon dispatch records. The state machine
covers:

- `proposed`
- `previewed`
- `awaiting_approval`
- `assignment_created`
- `wallet_ready`
- `payout_intent_created`
- `dispatch_authorized`
- `dispatch_blocked`
- `settlement_pending`
- `settlement_complete`
- `settlement_failed`

The new `runArtanisNexusPylonPaymentBackedDispatch` helper routes Artanis
dispatch through `TreasuryPaymentAuthority` instead of letting Artanis write
payment claims directly. The helper:

1. validates the Artanis dispatch record;
2. calls `previewPayout` with wallet-readiness evidence;
3. creates a payout intent only after the authority policy accepts the preview
   inputs;
4. dispatches the payout attempt through the selected payment adapter;
5. records public-safe refs for accepted work, payout intent, payout attempt,
   payout target approval, wallet readiness, run status, and settlement bridge
   state; and
6. returns a blocked dispatch record with a specific public blocker when a
   payment-authority gate rejects the dispatch.

## Gates enforced

The contract blocks before payout intent creation or payout dispatch when any
of these checks fail:

- no accepted-work ref exists;
- no approved payout target exists;
- wallet readiness is stale or absent;
- payout amount exceeds the spend cap;
- authority, adapter, agent, payout target, or Pylon is paused;
- the payout intent idempotency key has already been used;
- the payout adapter is unavailable;
- amount or payout target refs are malformed.

Blocked records use `paymentAuthorityState: dispatch_blocked` and include a
specific blocker ref such as:

```text
blocker.public.payment_authority.missing_accepted_work_ref
blocker.public.payment_authority.missing_payout_target_approval
blocker.public.payment_authority.stale_or_absent_wallet_readiness
blocker.public.payment_authority.replayed_idempotency_key
```

## Projection rules

Operator projections can see payment authority refs, payout attempt refs,
payout target approval refs, and wallet readiness refs.

Public projections can see public-safe accepted-work refs, payout intent refs,
settlement bridge refs, gate state, gate label, and whether the authority gate
passed. They do not expose operator-only payment authority refs, raw payment
material, wallet secrets, invoices, preimages, payout-target material, provider
credentials, private evidence, or raw timestamps.

## Runtime secret note

`MDK_ACCESS_TOKEN` and `MDK_MNEMONIC` are Worker runtime secrets. They were set
directly in the Cloudflare Worker dashboard for production. They should remain
Cloudflare secrets or be rotated with `wrangler secret put`; do not add their
values to `wrangler.jsonc`, docs, migrations, issue bodies, logs, D1 records,
or public/operator projections.

The current #428 contract does not require those secrets for tests because it
uses the simulation payment adapter. Live MDK dispatch remains gated by the
payment authority service, wallet readiness, spend caps, payout-target approval,
idempotency, and the later live bitcoin smoke issue.

## Verification

The focused test is:

```bash
bun run --cwd workers/api test -- src/artanis-nexus-pylon-adapters.test.ts
```

It proves:

- Artanis can run a complete simulated payment-backed Pylon dispatch after
  accepted work, payout-target approval, wallet readiness, and payment authority
  gates pass;
- public and operator projections expose the right payment state without
  private material;
- missing accepted work blocks before creating a payout intent;
- missing payout-target approval blocks before creating a payout intent;
- stale wallet readiness blocks before creating a payout intent;
- replayed payout-intent idempotency keys block a second dispatch; and
- no blocked preflight path records a payout attempt.

## Remaining work

This does not yet make Artanis a live Pylon dispatcher. The next dependent
pieces remain:

- public-safe Nexus/Pylon receipt pages and operator dashboard;
- Forum bridge for Artanis assignment, incident, release, and payout updates;
- two-wallet MDK bitcoin movement smoke with OpenAgents product surface receipts;
- Pylon v0.2 OpenAgents product surface release gate runbook and automated evidence checklist.
