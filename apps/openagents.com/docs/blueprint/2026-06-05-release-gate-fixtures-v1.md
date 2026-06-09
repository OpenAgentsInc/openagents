# Blueprint Release Gate And Eval Fixtures v1

Issue: OPENAGENTS-BP-010 / #230

This note records the typed Release Gate and eval fixture model. The source of
truth is `workers/api/src/blueprint/schemas/release-gate.ts`.

## Purpose

Release Gates prevent Program Signatures, Module Versions, route selectors,
email policies, and proof projectors from self-promoting into production.

Promotion requires:

- fixture pass state;
- review approval;
- policy compliance;
- rollback posture;
- scorecard;
- receipt evidence;
- explicit gate decision;
- no self-promotion attempt.

## Current Limits

This issue defines contracts and predicates only. Persistence, fixture runner
services, release approval routes, rollback execution, and Program Registry UI
are separate roadmap issues.
