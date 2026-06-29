# Omni Workroom Records v1

Date: 2026-06-05

Status: implemented for issue #210.

## Purpose

Omni workroom records promote runnable software orders into a durable work
container without replacing or hiding the existing customer order status page.

The first workroom model links customer intent, optional Site project, optional
Adjutant assignment, accepted outcome contract, task packet, source refs,
artifact refs, email refs, receipt refs, status, and blockers. It supports
Sites and non-Sites work such as PR-style coding requests.

## D1 Record

`omni_workrooms` records:

- `id` and unique `idempotency_key`;
- required `software_order_id`;
- optional `accepted_outcome_contract_id`;
- optional `site_id`;
- optional `assignment_id`;
- `work_kind`;
- `status`;
- `visibility`;
- `customer_intent_ref`;
- optional `task_packet_ref`;
- typed public-safe ref arrays for sources, artifacts, emails, receipts, and
  blockers;
- `public_receipt_ref`;
- bounded `metadata_json`;
- lifecycle timestamps.

Supported work kinds match accepted outcome contracts:

- `site`
- `coding`
- `adjustment`
- `existing_project_import`
- `business`
- `legal_sensitive`

Supported statuses:

- `queued`
- `active`
- `blocked`
- `waiting_review`
- `completed`
- `unavailable`
- `archived`

## Service Contract

`promoteOmniWorkroom`:

- records idempotently by `idempotency_key`;
- requires an active, unarchived software order;
- requires Site workrooms to include an existing active Site ref;
- accepts non-Sites workrooms with no Site ref;
- validates optional assignment and accepted outcome contract refs when
  supplied;
- validates all source, artifact, email, receipt, and blocker refs as
  public-safe refs;
- rejects raw provider, run-log, email, payment, wallet, token, invoice,
  preimage, customer-private, and secret-like material in refs or metadata.

## Projection Split

`publicOmniWorkroomProjection` exposes only:

- software order id;
- optional Site id;
- work kind;
- status;
- visibility;
- public receipt ref.

`customerOmniWorkroomProjection` additionally exposes customer-safe intent,
artifact, email, receipt, and blocker refs.

`operatorOmniWorkroomProjection` includes operator-only linkage:

- workroom id;
- accepted outcome contract id;
- assignment id;
- source refs;
- task packet ref.

This split is the first guardrail against destroying customer order state or
leaking private run mechanics through public/customer surfaces.

## Boundaries

This slice does not:

- replace the order status API;
- change Site revision behavior;
- create customer UI;
- mutate order status;
- send email;
- accept or reject work;
- create payout eligibility or settlement claims.

It creates the durable workroom record layer that later evidence bundle,
acceptance lifecycle, Mission Briefing, and economics issues can consume.
