# Pylon Marketplace Payout Flow Runbook

Date: 2026-06-07

## Status

Issue #427 adds the first OpenAgents product surface-owned bridge from Pylon marketplace accepted
work to Nexus/Treasury payout intent and settlement receipts.

The implementation is in:

- `workers/api/src/pylon-marketplace-payout-flow.ts`
- `workers/api/src/pylon-marketplace-payout-flow.test.ts`

It is intentionally simulation-only. It does not call MDK, does not shell out
to an agent wallet, does not move bitcoin, and does not read the Cloudflare
`MDK_ACCESS_TOKEN` or `MDK_MNEMONIC` secrets.

## What The Flow Proves

The flow now has a typed evidence path:

1. A Pylon marketplace assignment must be `accepted`.
2. The assignment must be in `accepted_work` payout state.
3. The assignment must carry accepted-work refs, artifact evidence, result
   evidence, Nexus receipts, Pylon receipts, Treasury receipts, payout caveats,
   provider refs, and acceptance criteria.
4. That accepted-work evidence can build a Nexus Treasury payout intent.
5. The payout intent can be created through `TreasuryPaymentAuthority`.
6. A simulation payout attempt can be dispatched through the simulation adapter.
7. A simulation reconciliation event can be recorded.
8. Public-safe payment authority receipts can be generated for:
   - payout intent created;
   - dispatch recorded;
   - confirmation recorded;
   - verification recorded;
   - settlement recorded.
9. The settlement bridge can project a timeline:
   - `reward_intent`;
   - `payout_eligible`;
   - `payout_dispatched`;
   - `payout_confirmed`;
   - `payout_verified`;
   - `settled`.
10. A paused or failed settlement path is modeled as a blocked evidence-only
    bridge record with blocker refs.

## Authority Boundary

This issue does not grant live payout authority. The new payout-flow module
builds records and projections that can be passed into the existing treasury
authority service and simulation adapter.

The settlement bridge remains evidence-only:

- no buyer charge mutation;
- no live wallet spend;
- no payout dispatch authority;
- no payout target mutation;
- no settlement mutation.

The accepted-work payout row remains read-only:

- no public claim upgrade;
- no payout target mutation;
- no settlement mutation;
- no live wallet spend.

Live bitcoin movement remains blocked until later OpenAgents product surface/Nexus gates prove
approved payout targets, spend caps, fresh wallet readiness, MDK adapter
dispatch, reconciliation, and public-safe receipt pages.

## Runtime Secrets

`MDK_ACCESS_TOKEN` and `MDK_MNEMONIC` are set in the Cloudflare Worker
dashboard as secrets. They are not required for this simulation path.

Do not put those secret values in `wrangler.jsonc`, docs, issue comments,
logs, screenshots, D1 rows, public projections, or Forum posts.

Future MDK-backed payout work should use the existing treasury payment
authority boundary. The MDK adapter may read runtime secrets only after a
durable payout intent and policy gates have passed.

## Verification

Run:

```bash
bun run --cwd workers/api test -- src/pylon-marketplace-payout-flow.test.ts src/pylon-marketplace-jobs.test.ts src/pylon-settlement-bridge.test.ts src/pylon-accepted-work-payout-rows.test.ts src/treasury-payment-authority.test.ts src/treasury-payment-simulation-adapter.test.ts src/nexus-treasury-payout-ledger.test.ts
```

The focused payout-flow test checks:

- a simulated marketplace job can move from intake to assignment to accepted
  work to payout intent to settlement receipt;
- missing accepted-work refs or a non-accepted assignment blocks payout flow;
- settlement bridge records link back to job, assignment, Artanis dispatch,
  payout intent, and adapter attempt;
- public projections redact dispatch and verification internals while keeping
  settlement claims inspectable;
- paused settlement is represented as a blocked evidence-only bridge record.

## Remaining Gaps

- #428 must connect Artanis/Nexus/Pylon adapters to this payment-backed gate.
- #429 must add public-safe Nexus/Pylon receipt pages and operator dashboard
  visibility.
- #431 must prove a real two-wallet MDK bitcoin movement through the OpenAgents product surface
  authority path.
- Marketplace finalization and payout execution are not yet exposed as a
  browser dashboard flow.
