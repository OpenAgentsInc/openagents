# Program Run And Receipt Webhook Subscriptions

Date: 2026-06-06

Status: implemented contract note for issue #347 / `OPENAGENTS-DEV-003`.

## Purpose

OpenAgents product surface now has a focused schema-first contract for subscribing external systems
to Program Run and receipt lifecycle events.

The implementation extends the existing webhook subscription contract in:

- `workers/api/src/webhook-subscriptions.ts`.

This is a contract and projection layer only. It does not call external
webhooks, enqueue delivery jobs, mutate Program Runs, mutate receipts, mutate
payments, escalate auth, or carry raw secret material.

## Contract Model

`ProgramRunReceiptWebhookSubscriptionContract` records:

- subscription ref;
- subscriber refs;
- selected event families;
- event topic refs;
- scoped auth refs;
- endpoint refs;
- delivery preparation refs;
- delivery attempt refs;
- retry state refs;
- replay window refs;
- redaction policy refs;
- receipt refs;
- blocker refs;
- caveat refs;
- revocation refs;
- evidence refs;
- idempotency refs;
- operator diagnostic refs; and
- last event ref.

The focused contract accepts only these event families:

- `program_run`; and
- `receipt`.

Broader webhook families still live in the generic
`WebhookSubscriptionRecord`, `WebhookEventRecord`, and `WebhookDeliveryRecord`
contracts from issue #327.

## Lifecycle Separation

The focused lifecycle states are:

- registration recorded;
- event selection recorded;
- delivery prepared;
- delivery attempt recorded;
- retry scheduled;
- replay window recorded;
- receipt recorded;
- revoked; and
- blocked.

Projection flags keep those states separate. A delivery preparation ref is not
a delivery attempt. A retry state ref is not a replay window. A receipt ref is
not a dispatcher success claim. A revocation ref prevents future delivery
authority but does not erase previous receipt evidence.

## Retry, Replay, And Idempotency

`programRunReceiptWebhookReplayKey(subscriptionRef, topicRef)` creates a
stable replay key for a subscription/topic pair.

`programRunReceiptWebhookIdempotencyRef(subscriptionRef, topicRef, state)`
creates a lifecycle-bound idempotency ref for the subscription/topic/state
combination.

The projection exposes retry, replay, and revocation state as references only.
The dispatcher, queue, and delivery worker remain future work.

## Authority Boundary

The default authority block is
`programRunReceiptWebhookContractOnlyAuthority()`.

It explicitly denies:

- auth escalation;
- delivery queue enqueue;
- external webhook calls;
- payment mutation;
- Program Run mutation;
- receipt mutation; and
- secret material.

`programRunReceiptWebhookAuthorityIsContractOnly` returns true only when the
full deny block is present.

## Projection And Redaction

Public, customer, and team projections hide private subscriber refs, private
endpoint refs, scoped auth refs, and operator diagnostics. Operator projections
may show safe binding refs and diagnostics, but never raw secrets, webhook
payloads, provider tokens, raw logs, raw invoices, wallet material, or raw
timestamps.

Projection times use friendly labels instead of raw timestamps.

## Tests

`workers/api/src/webhook-subscriptions.test.ts` covers:

- schema/projection decoding;
- Program Run/receipt event-family scoping;
- lifecycle phase separation;
- retry, replay, and revocation state;
- hard false external-send, queue, Program Run, receipt, payment, auth, and
  secret authority flags;
- idempotency and replay helper refs; and
- redaction of private endpoint, subscriber, auth, diagnostic, raw payload,
  provider, wallet/payment, and raw timestamp material.
