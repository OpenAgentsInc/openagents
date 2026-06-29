# Omni Route Scorecard v1

Date: 2026-06-05

Status: implemented for issue #215.

## Purpose

Route scorecards record why a workroom used one account, model, runtime, and
provider route instead of another. They are evidence for future route quality,
account reliability, market memory, and operator debugging, not public
provider-account logs.

## D1 Record

`omni_route_scorecards` records:

- `id` and unique `idempotency_key`;
- required `workroom_id`;
- work kind;
- selected route ref;
- selected provider ref;
- optional selected account ref;
- selected model ref;
- selected runtime ref;
- rejected candidates;
- decision reason refs;
- observed result kind/ref;
- optional post-closeout score from 0 through 100;
- cost cents;
- latency milliseconds;
- privacy tier;
- trust tier;
- public caveat ref;
- bounded metadata;
- lifecycle timestamps.

## Rejected Candidates

Rejected route candidates carry:

- candidate ref;
- reason kind;
- reason ref.

Supported reason kinds:

- `cost`
- `latency`
- `privacy`
- `trust`
- `capability`
- `availability`
- `quality`
- `quota`

## Projection Split

Public projection exposes only customer-safe route summary fields:

- workroom id;
- work kind;
- selected model ref;
- selected runtime ref;
- observed result kind/ref;
- post-closeout score;
- trust tier;
- public caveat ref.

Customer projection additionally exposes selected route ref, decision reason
refs, and privacy tier.

Operator projection includes the full scorecard, including selected account
ref, rejected candidates, cost, latency, and bounded metadata.

## Guardrails

`recordOmniRouteScorecard`:

- records idempotently by `idempotency_key`;
- requires an existing active workroom;
- requires scorecard `workKind` to match the workroom;
- validates rejected candidates and reason refs;
- rejects provider-account secrets, raw provider payloads, raw run logs, raw
  emails, payment/wallet material, and customer-private material in refs or
  metadata;
- validates non-negative cost and latency;
- validates post-closeout scores from 0 through 100.

## Boundaries

This slice does not:

- change routing policy;
- fail over provider accounts;
- spend credits;
- publish private account identifiers;
- update market memory directly.

It creates the route evidence that market memory, projection policy, and
future Blueprint routing signatures can consume.
