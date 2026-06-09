# Flexible-Load Event Telemetry

Date: 2026-06-06

Status: implemented for issue #365 / `OPENAGENTS-LATE-005`.

## Purpose

Flexible-load profiles say which work classes can pause, resume, checkpoint, or
respond to power events. Flexible-load event telemetry records what actually
happened during a requested response.

The implementation lives in
`workers/api/src/pylon-flexible-load-events.ts`.

## Event Shape

`PylonFlexibleLoadEventRecord` records:

- requested response watts;
- actual response watts;
- request refs;
- acknowledgement refs;
- execution refs;
- measurement refs;
- evidence refs;
- compensation refs;
- settlement refs;
- interrupted work refs;
- checkpoint refs;
- resume refs;
- lost-work cost;
- accepted-work impact refs;
- profile and work-class refs;
- provider ref;
- caveats, blockers, and source refs;
- read-only authority.

## Lifecycle

Event state can be:

- `requested`;
- `acknowledged`;
- `executed`;
- `measured`;
- `verified`;
- `compensated`;
- `settled`;
- `blocked`;
- `failed`.

The projection keeps each claim separate:

- requested response;
- acknowledged response;
- executed response;
- measured response;
- verified evidence;
- compensation evidence;
- settlement evidence;
- accepted-work impact.

`responseRatioBps` is derived from actual response watts divided by requested
response watts. It is `null` until actual response is measured.

## Evidence Rules

The contract rejects overclaims:

- all events require request refs;
- acknowledged events require acknowledgement refs;
- executed events require execution refs;
- measured events require actual response watts and measurement refs;
- verified events require evidence refs;
- compensated events require compensation refs;
- settled events require settlement refs;
- lost-work cost requires interrupted work refs;
- resume refs require checkpoint refs.

## Authority Boundary

This event ledger does not dispatch grid-service actions. It cannot:

- mutate accepted work;
- dispatch capacity;
- upgrade grid-service claims;
- spend from a wallet;
- dispatch payouts;
- mutate settlement.

It is evidence for future investor/grid proof, not a command path.

## Redaction

Public and customer projections hide private accepted-work, acknowledgement,
checkpoint, compensation, event, evidence, execution, interruption,
measurement, profile, provider, request, resume, settlement, source, and
work-class refs. Team projections still hide private compensation, measurement,
provider, and settlement refs. Operator projections may include safe internal
refs, but raw provider payloads, runner logs, wallet material, payment
material, payout targets, raw telemetry, private hardware identifiers, secrets,
and raw timestamps are rejected for every audience.

## Tests

`workers/api/src/pylon-flexible-load-events.test.ts` covers:

- settled event projection;
- requested, acknowledged, executed, measured, verified, compensated, and
  settled lifecycle separation;
- requested/actual response summaries;
- lost-work accounting;
- private ref redaction;
- false settlement and missing lifecycle evidence rejection;
- false dispatch, grid-claim, wallet, payout, and settlement authority
  rejection;
- raw telemetry, payment, provider, runner, payout target, wallet, and raw
  timestamp rejection.
