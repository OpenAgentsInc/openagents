# Omni Workroom Lifecycle v1

Date: 2026-06-05

Status: implemented for issue #212.

## Purpose

Omni workroom lifecycle decisions record human acceptance, rejection,
provisional acceptance, reopening, revision requests, and unavailable outcomes
without implying payment settlement or payout eligibility.

The lifecycle ledger is separate from the workroom, evidence bundle, and
economics ledgers. It records the review decision and the customer-safe receipt
needed to explain what happened next.

## D1 Record

`omni_workroom_lifecycle_decisions` records:

- `id` and unique `idempotency_key`;
- required `workroom_id`;
- `work_kind`;
- actor kind;
- decision kind;
- resulting lifecycle state;
- customer-safe explanation ref;
- receipt ref;
- optional Site revision feedback ref;
- optional non-Sites follow-up request ref;
- optional artifact ref;
- explicit `no_settlement_implication`;
- bounded metadata;
- lifecycle timestamps.

## Decision Kinds

Supported decision kinds:

- `accept`
- `reject`
- `provisionally_accept`
- `reopen`
- `request_revision`
- `mark_unavailable`

Resulting states:

- `accepted`
- `rejected`
- `provisionally_accepted`
- `reopened`
- `revision_requested`
- `unavailable`

## Revision Requests

Site revision requests must include a `site_revision_feedback_ref` so the
decision can connect to the existing Sites feedback queue.

Non-Site and PR-style revision requests must include a `followup_request_ref`
so the decision can connect to `order_fulfillment_feedback` or an equivalent
future follow-up queue.

## Projection Split

Public projection exposes:

- workroom id;
- work kind;
- resulting state;
- customer-safe explanation ref;
- receipt ref;
- no-settlement implication flag.

Customer projection additionally exposes decision kind and customer-safe
revision/artifact refs.

Operator projection includes actor kind, idempotency key, and bounded metadata.

## Guardrails

`recordOmniWorkroomLifecycleDecision`:

- records idempotently by `idempotency_key`;
- requires an existing active workroom;
- requires decision `workKind` to match the workroom;
- requires a customer-safe explanation ref and receipt ref;
- requires Site revision requests to carry a Site feedback ref;
- requires non-Site revision requests to carry a follow-up request ref;
- rejects raw provider, run-log, email, payment, settlement, payout, wallet,
  token, invoice, preimage, customer-private, and secret-like material in refs
  or metadata;
- always records `noSettlementImplication: true`.

## Boundaries

This slice does not:

- send email;
- mutate workroom status;
- publish a proof page;
- deploy a revision;
- create payout eligibility;
- mark any payment as settled.

It creates the decision receipt layer that Mission Briefing, proof bundle, and
economics issues can consume.
