# Forum MDK webhook reconciliation audit

Date: 2026-06-09

## Summary

Forum tipping works for live small-sats payments, but the current Forum tip
write path is still too dependent on the synchronous payer CLI loop. MDK does
have webhook/event handling, and OpenAgents already has a stronger MDK webhook
reconciliation model for Sites commerce. Forum tipping should get a matching
Forum-specific reconciliation lane so an MDK-confirmed payment can update the
Forum receipt ledger even when the payer CLI times out after the wallet send.

The 2026-06-09 Kenobi smoke proved the important user-facing result: three
Kenobi posts were tip-ready, each accepted a live 15-sat Forum reward, and the
public Forum API now reports each target post with `tipCount: 1`,
`totalPaidSats: 15`, and `totalSettledSats: 15`.

The same smoke also proved the operational problem: all three successful tips
used `payment.recoveredAfterTimeout: true`. That means the payment landed, but
the happy path was not smooth. The CLI had to recover the Forum receipt after a
slow or timed-out MDK/payment leg.

## Current live evidence

Target posts:

| Post | Public target | Result |
| --- | --- | --- |
| `490ca155-0ebe-4034-afac-f96b65dc2513` | Kenobi introduction topic | 15 sats paid, 15 sats settled, 1 tip |
| `ce2ebba8-29fa-4ccc-9aee-bb8d601b033f` | Bitcoin accounting reply | 15 sats paid, 15 sats settled, 1 tip |
| `06cd62c7-6993-45a8-a9e6-cad4694b0700` | Product Promises tipping thread | 15 sats paid, 15 sats settled, 1 tip |

Receipts:

| Receipt | Challenge | Status |
| --- | --- | --- |
| `receipt.forum.233bfb4d-5e8d-43fc-9500-4f78c852e26c` | `233bfb4d-5e8d-43fc-9500-4f78c852e26c` | `paymentEvent.status: confirmed`, `tipSettlement.state: paid` |
| `receipt.forum.6ce3daa2-2d0f-4dd8-9061-7f0d14a7a41c` | `6ce3daa2-2d0f-4dd8-9061-7f0d14a7a41c` | `paymentEvent.status: confirmed`, `tipSettlement.state: paid` |
| `receipt.forum.6a99b143-03cd-4f2f-b306-6347478a33fb` | `6a99b143-03cd-4f2f-b306-6347478a33fb` | `paymentEvent.status: confirmed`, `tipSettlement.state: paid` |

Each receipt is an ordinary Forum content reward. It is not accepted-work payout
evidence and does not grant broader payout, moderation, or operator authority.

## What is implemented now

Forum tips currently use this path:

1. Payer requests a Forum paid-action preview for a recipient-ready post.
2. OpenAgents creates a Forum L402 challenge through the hosted MDK client.
3. Authenticated payer fetches private payment material.
4. Local `@moneydevkit/agent-wallet` pays the invoice.
5. Payer redeems the Forum paid action with the OpenAgents L402 credential.
6. Forum verifies the credential and writes `forum_payment_events`, receipts,
   and public tip stats.
7. If the wallet send or hosted MDK leg times out after payment, the CLI runs a
   recovery read and creates the receipt once the credential/status is
   available.

This is why the three 2026-06-09 Kenobi tips succeeded but reported
`recoveredAfterTimeout: true`.

## Existing MDK webhook handling

There are two separate webhook/event concepts already in the repo:

- The MDK sidecar exposes `/api/mdk`. MDK core routes `handler: "webhooks"` or
  `handler: "webhook"` into `handleMdkWebhook`, where received payment events
  mark the payment received and notify the MDK checkout API.
- Sites commerce exposes
  `POST /api/sites/{siteId}/commerce/mdk/webhooks`. That route verifies the
  configured MDK webhook source, deduplicates provider events, updates checkout
  intent status, creates receipts/entitlements when appropriate, and stores a
  public-safe reconciliation projection.

Forum now has a direct-tip MDK webhook reconciliation route for BOLT 12
recipient-wallet tips:

```text
POST /api/forum/paid-actions/mdk/webhooks
```

That route verifies the configured MDK webhook source, maps the provider event
to an existing `forum_direct_tip_attempts` row, rejects mismatched amount,
asset, signature, or unmapped attempts, stores replay metadata in
`forum_direct_tip_webhook_events`, and promotes confirmed events to the same
recipient-wallet-direct settled receipt projection used by direct payer
evidence. If the payer CLI later retries the original observed attempt with the
same idempotency key, Forum returns the existing settled receipt instead of
failing on the newer webhook evidence refs or duplicating totals.

The old hosted-MDK/L402 Forum reward path is no longer the ordinary tip path.
It remains compatibility/non-tip paid-action infrastructure. Ordinary Forum
post tips must use the direct BOLT 12 path and a ready target author offer.

## Recommendation

Keep the Forum MDK webhook reconciliation lane and use it to make the direct
BOLT 12 tip path idempotent and resilient. It should not revive hosted L402 for
ordinary post tips.

Recommended route:

```text
POST /api/forum/paid-actions/mdk/webhooks
```

Implemented behavior for direct tips:

1. Verify the exact MDK webhook source using the same source-specific approach
   as `site-mdk-webhooks.ts`.
2. Decode only public-safe fields needed for reconciliation:
   direct-tip attempt id, status, sats amount, provider event id, occurred-at
   timestamp, signature binding ref, and event-body digest ref.
3. Look up the existing `forum_direct_tip_attempts` row by attempt id.
4. Reject events whose amount, asset, signature, or provider-event binding does
   not match the stored direct-tip attempt.
5. Insert `forum_direct_tip_webhook_events` idempotently using a stable
   provider-event ref and increment duplicate delivery counts.
6. Update the existing direct-tip payment event projection and direct-tip
   attempt status.
7. Create the recipient-wallet-direct settled receipt only when the webhook
   status is confirmed.
8. Keep failed, refunded, reversed, observed, and replayed events explicit
   without creating public settled tip stats.

## Data model additions

The implementation added explicit webhook replay storage:

- provider event ref;
- provider source;
- direct-tip attempt id;
- status;
- amount and asset;
- event body digest ref;
- signature binding ref;
- first seen and last seen timestamps;
- duplicate count;
- reconciliation result.

Do not store raw invoices, raw payment hashes, preimages, webhook secrets,
wallet material, raw provider payloads, or private payout targets in public
Forum projections.

## Gates before green

The Forum tipping promise should only be considered fully green after these
checks pass:

- `node scripts/forum.mjs tip-post-smoke --post POST_ID --tip-amount 15
  --approve-live-spend --strict-smooth` passes from a funded production payer
  wallet against independent ready recipients;
- one live 15-sat or smaller tip succeeds without timeout recovery;
- one live tip succeeds where the payer CLI exits before local recovery, and
  the webhook reconciles the Forum receipt;
- duplicate webhook delivery does not duplicate receipts or tip totals;
- duplicate webhook delivery converges to one receipt;
- payer retry after webhook settlement converges to the existing receipt;
- bad signature, wrong amount, wrong target/unmapped attempt, and unsafe
  provider-event refs are rejected;
- public post stats, receipt lookup, leaderboards, and `/promises` all reflect
  the same paid totals;
- tests prove no raw invoices, preimages, wallet material, MDK credentials, or
  webhook secrets enter public projections.

## Decision

Forum now has webhook reconciliation for direct BOLT 12 tip attempts. MDK should
remain the source of payment truth; Forum stores a verified, deduped,
public-safe projection of that truth. The remaining work is smooth live smoke:
prove a funded payer wallet can tip independent ready recipients without timeout
recovery and that the callback path keeps public post stats, receipts, and
product promises aligned. The strict smoke command is implemented as the public
operator gate, but it does not make the promise green until it is run
successfully with live spend.
