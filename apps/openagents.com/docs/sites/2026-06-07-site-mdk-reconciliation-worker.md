# Site MDK Reconciliation Worker Contract

Date: 2026-06-07
Issue: #449 / OPENAGENTS-H-012

## Summary

OpenAgents product surface now has a scheduled/queue-safe Site MDK reconciliation worker contract.
It builds on the existing exact-source Site MDK webhook verification and Site
MDK reconciliation projector. It does not introduce a second webhook model and
does not import MDK native node runtime code into the Cloudflare Worker.

The worker contract is a deterministic planner. A scheduled job, queue
consumer, webhook handler, or operator route can pass it the current checkout
intent, buyer payment challenge, provider event or safe provider status check,
receipt, entitlement, prior reconciliation events, and retry metadata. The
contract returns a public-safe plan describing the status and the idempotent
writes a runner may apply.

## Statuses

The projection returns one of these statuses:

| Status | Meaning |
| --- | --- |
| `pending` | The checkout is still pending and has no provider evidence yet. |
| `provider_seen` | A provider event or safe status check was observed but payment is not complete. |
| `payment_seen` | Payment evidence, receipt, and entitlement are already present. |
| `receipt_created` | Verified payment evidence exists and the next idempotent write is receipt creation. |
| `entitlement_created` | Receipt exists and the next idempotent write is entitlement creation. |
| `expired` | The challenge or checkout expired before payment evidence appeared. |
| `stale` | The checkout has been pending past the configured stale threshold. |
| `replayed` | The provider event or settled event was already recorded. |
| `conflict` | Checkout, challenge, provider status, receipt, or entitlement refs disagree. |
| `provider_unavailable` | The configured provider status-check lane is unavailable. |
| `operator_review` | Verification is missing or the plan needs human/operator review. |

## Planned Actions

The plan can ask a runner to apply these idempotent actions:

- `record_reconciliation_event_once`;
- `create_receipt_once`;
- `create_entitlement_once`;
- `expire_payment_challenge`;
- `expire_checkout_intent`;
- `schedule_status_check`;
- `request_operator_review`.

The action names are intentionally explicit. They are safe to use from a
scheduled job, queue consumer, webhook handler, or operator route because the
consumer must still apply database writes with the supplied idempotency refs and
existing uniqueness constraints.

## Replay And Conflict Handling

Replay detection compares:

- provider ref;
- provider event ref;
- event body digest ref;
- challenge ref;
- receipt ref.

Conflict detection checks:

- checkout intent to buyer payment challenge;
- receipt to challenge/product;
- entitlement to receipt/challenge/product;
- incoming provider event to checkout/site/product/provider;
- provider status check to checkout/provider;
- out-of-order provider status after checkout payment is already marked
  received.

Duplicate/replayed events produce no write actions. Conflicts request operator
review and do not grant receipt, entitlement, payout, or settlement authority.

## Retry And Backoff

The input includes bounded retry metadata:

- attempt;
- max attempts;
- backoff seconds;
- next attempt timestamp.

The projection returns `retryAllowed` and `nextAttemptAt` only for pending,
provider-seen, stale, and provider-unavailable states while the attempt count
is still under the configured maximum.

## Redaction Boundary

The worker rejects private or payment-secret material before planning and checks
the final projection again. It rejects raw invoices, raw provider payloads,
payment preimages, raw payment hashes, MDK credentials, Stripe secrets, wallet
state, provider grants, emails, source archives, and runner payloads.

Public projections omit operator refs. Operator projections can include stable
refs such as checkout intent refs, provider event refs, event body digest refs,
status check refs, duplicate refs, and conflict refs, but still must not expose
raw provider payloads or secrets.

## Current Boundary

This issue adds the reusable worker contract and regression tests. The contract
does not itself schedule cron jobs, enqueue messages, mutate D1, call MDK, call
Stripe, grant entitlement, create payout intent, or settle provider payout.

The next step is to wire this planner into the live scheduled/queue/operator
route that owns durable D1 writes, while keeping exact-source webhook
verification and configured provider status lookup as the only provider-status
inputs.
