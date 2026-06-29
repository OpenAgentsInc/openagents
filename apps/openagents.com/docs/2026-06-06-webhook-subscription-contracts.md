# Webhook Subscription Contracts

Date: 2026-06-06

Status: implemented contract note for issue #327 / `OPENAGENTS-080`.

## Purpose

OpenAgents product surface needs durable webhook contracts before it can let external systems
subscribe to workroom, Site, Program Run, claim, receipt, Forum/payment, and
package-review lifecycle events.

The implementation lives in `workers/api/src/webhook-subscriptions.ts`.

This issue defines contracts and projections only. It does not add the final
dispatcher or delivery queue worker.

## Records

The v1 contract has three record types:

- `WebhookSubscriptionRecord`;
- `WebhookEventRecord`; and
- `WebhookDeliveryRecord`.

Subscription records capture:

- auth mode;
- owner ref;
- endpoint ref;
- secret binding ref only;
- event families;
- event source refs;
- scope refs;
- redaction audience;
- retry policy;
- max attempts;
- status; and
- caveat/evidence refs.

Event records capture:

- event family;
- source ref;
- subject ref;
- payload digest ref;
- payload schema ref;
- source-authority refs;
- receipt refs;
- replay key;
- idempotency key; and
- redaction audience.

Delivery records capture:

- subscription ref;
- event ref;
- endpoint ref;
- state;
- attempt;
- max attempts;
- retry policy;
- next-attempt display;
- delivered display;
- failure class;
- failure summary ref;
- replay key;
- idempotency key;
- payload digest ref; and
- delivery receipt refs.

## Event Families

The current event families are:

- `workroom`;
- `site_revision`;
- `site_version`;
- `program_run`;
- `public_claim`;
- `receipt`;
- `forum_payment_receipt`;
- `payment_reconciliation`; and
- `package_review`.

## Authority And Scope

Webhook subscriptions can be scoped by:

- `operator_admin`;
- `owner_grant`;
- `registered_agent_grant`;
- `team_scope`; or
- `system`.

The auth mode describes required authority. It does not create that authority
by itself. Future route implementations must enforce owner, team, agent, or
operator grants before creating or activating subscriptions.

## Retry And Replay

`webhookEventReplayKeyForDelivery(subscriptionRef, eventRef)` creates the
stable replay key for a subscription/event pair.

`webhookDeliveryIdempotencyKey(subscriptionRef, eventRef, attempt)` creates the
attempt-bound delivery idempotency key.

`webhookDeliveryCanRetry` returns true only when:

- state is `failed` or `retry_scheduled`;
- retry policy is not `none`;
- attempt is below max attempts; and
- failure class is retriable.

Policy denial, redaction failure, and missing secret binding are non-retriable
until an operator or owner changes the underlying state.

## Redaction

Projections reject webhook secrets, raw webhook bodies, raw provider responses,
tokens, customer private data, wallet/payment material, payout targets,
private repo refs, raw runner logs, raw source archives, and raw timestamps.

Public/customer projections hide private endpoint, owner, scope, source, and
subject refs. Operator projections may show safe secret binding refs, but never
raw secret values.

## Tests

`workers/api/src/webhook-subscriptions.test.ts` covers:

- subscription projection and operator-only secret binding refs;
- event and delivery projection;
- replay key and delivery idempotency key generation;
- retry and non-retry behavior;
- required event families and non-negative attempts; and
- redaction of raw webhook, provider, customer, wallet, payment, and runner
  material.
