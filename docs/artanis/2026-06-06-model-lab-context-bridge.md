# Artanis Model Lab Context Bridge

Date: 2026-06-06

Issue: #391 / `ARTANIS-006`

## Purpose

Artanis needs Model Lab evidence to improve Autopilot, but reading evidence is
not the same as launching training, installing adapters, promoting a runtime,
or making public claims. The context bridge gives Artanis a typed private
context over Model Lab evidence while keeping public Forum and `/artanis`
summaries downstream of the public Model Lab report projection.

## Implementation

Code:

- `workers/api/src/artanis-model-lab-context.ts`
- `workers/api/src/artanis-model-lab-context.test.ts`

The bridge consumes the implemented Model Lab contracts:

- retained-failure loop;
- model artifact;
- training run;
- evidence graph;
- Benchmark Cloud evidence;
- promotion decision ledger;
- public Model Lab report.

Private Artanis/operator context can inspect those projections as read-only
evidence. Public Artanis and Forum projections receive only the public Model Lab
report projection, plus public-safe readiness/blocker refs.

## Missing Evidence

Missing contracts or missing report evidence become blockers:

- missing retained-failure loop;
- missing model artifact;
- missing training run;
- missing evidence graph;
- missing Benchmark Cloud evidence;
- missing promotion decision ledger;
- missing public report.

When blockers or missing evidence exist, the bridge does not produce a public
promotion claim. It emits missing contract refs, missing evidence refs, blocker
refs, and operator-facing next-action drafts instead.

## Operator Action Drafts

The bridge can draft operator-facing next actions without executing them:

- request missing Model Lab contracts;
- request missing evidence;
- inspect retained failures;
- draft an eval rerun when retained failures lack eval evidence;
- draft a public Forum summary from a complete public Model Lab report;
- request operator promotion review when a report says promotion passed but is
  not deployed.

Any action that could lead to eval reruns or promotion review remains an
operator-facing draft. It is not execution authority.

## Public Boundary

Forum and `/artanis` summaries must use the Model Lab public report projection
only. They do not receive:

- retained-failure loop internals;
- model artifact internals;
- training run internals;
- evidence graph internals;
- Benchmark Cloud internals;
- promotion decision internals;
- private evidence refs;
- operator next-action drafts.

## Authority Boundary

The bridge rejects false authority from consumed Model Lab records. Projections
cannot expose authority to:

- launch evals or training;
- call providers;
- install adapters;
- promote runtime behavior;
- mutate routes;
- publish reports;
- copy raw artifacts, benchmark inputs, datasets, or weights;
- spend money;
- mutate payouts or settlement;
- upgrade public claims.

## Tests

Coverage proves:

- all implemented Model Lab contracts project into private Artanis context;
- public Forum and public Artanis audiences receive only the public report
  projection;
- missing contracts and missing evidence produce blockers rather than public
  promotion claims;
- retained failures without eval evidence draft an operator eval-rerun action;
- non-Artanis contexts, unsafe refs, and false Model Lab authority are rejected;
- raw timestamps and private material do not leak into projections.
