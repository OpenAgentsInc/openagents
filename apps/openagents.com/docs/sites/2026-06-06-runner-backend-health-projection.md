# Runner Backend Health Projection

Date: 2026-06-06

Status: implemented for issue #284 / OPENAGENTS-RUNNER-006.

## Purpose

This slice adds operator-safe runner backend health, capacity, cold-start, and
cost projection without exposing private runner mechanics to public or customer
surfaces.

It keeps failover policy separate from health projection.

## Implemented Contract

`workers/api/src/runner-backend-health-projection.ts` models:

- backend availability;
- enabled and configured state;
- health gates;
- capacity refs;
- queue-depth refs;
- cold-start refs;
- cost tier refs;
- billing caveat refs;
- smoke refs;
- operator diagnostic refs;
- public summary refs.

## Projection Rules

Public and customer projections expose only:

- backend kind;
- high-level availability;
- public summary ref;
- safe caveat refs for failed gates.

They do not expose queue depth, cold-start details, cost tiers, capacity
metadata, operator diagnostics, raw logs, provider material, source archives,
wallet material, or failover policy.

Operator projections can include redacted billing, capacity, queue, cold-start,
health, smoke, gate, and diagnostic refs, while still filtering unsafe raw
material and failover policy refs.

## Current Non-Goals

This issue does not:

- select failover policy;
- dispatch any backend;
- bill a live run;
- expose live Cloudflare Container observability;
- publish customer-facing backend internals.

Those remain separate policy, runtime, and operator-dashboard concerns.
