# Product Promises

Product promises are the claims OpenAgents makes to users, customers,
contributors, operators, and agents about what the product does, enables,
protects, or refuses to do.

This directory generalizes the launch-promise audit and copy gates into a
product-wide promise system. The goal is simple: public and agent-readable
copy should stay tied to evidence, and every important claim should have a
clear way to verify it, gate it, downgrade it, fix it, or report that it is not
working.

## Promise Rule

A product promise is not green because it is intended, implemented in part, or
true in a narrow operator rehearsal. It is green only when the matching
evidence is available, current, public-safe, and checked by the right gate.

Until then, the promise should be written as planned, gated, partial, blocked,
or unavailable. Discovery docs may describe planned or gated routes, but they
must not imply authority to spend, settle, dispatch, moderate, deploy, publish,
or mutate provider accounts unless a separate authority gate says so.

## Audiences

- Users need to know what OpenAgents can do for them now.
- Contributors need to know what Pylon, Forum, Sites, and other participation
  paths can honestly offer.
- Agents need machine-readable boundaries: allowed actions, blocked actions,
  evidence refs, Forum report paths, and issue refs when maintainers open
  follow-up issues.
- Operators need release gates, proof gates, stale-state handling, and a way to
  downgrade copy before a claim becomes misleading.

## Directory Map

- [`source-set.md`](source-set.md): launch-promise audit docs found, plus the
  verified closed issue set that informed this product-promise system.
- [`registry.md`](registry.md): the promise record contract and current promise
  families across OpenAgents product areas.
- [`checks-and-gates.md`](checks-and-gates.md): state model, verification
  gates, copy gates, redaction gates, settlement gates, and release gates.
- [`reporting-and-rci.md`](reporting-and-rci.md): Forum-first report intake and
  a correction loop that can feed OpenAgents review, correction, and
  incorporation work.
- [`templates/promise-record.md`](templates/promise-record.md): a reusable
  promise record template.
- [`templates/promise-report.md`](templates/promise-report.md): a reusable
  report template for users, operators, and agents.

## Product Areas

These docs currently cover promises for:

- Autopilot workrooms, accepted outcomes, proof, review, and next actions.
- Pylon contributor compute, readiness, assignments, receipts, and settlement
  evidence.
- Forum posting, moderation, reports, tipping, and public-safe projections.
- Sites generation, revisions, deployments, commerce, and referral attribution.
- Agent-readable sheets, manifests, OpenAPI coverage, and route authority.
- Payment, wallet, payout, referral, revenue-share, and settlement claims.
- Provider capacity, training, benchmark, data, trace, and marketplace claims.

The promise system should move with the product. When a new product area makes
a user-facing or agent-facing claim, add a promise record before broadening
copy.
