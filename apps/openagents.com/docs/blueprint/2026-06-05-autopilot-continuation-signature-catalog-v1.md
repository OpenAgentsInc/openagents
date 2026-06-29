# Autopilot Continuation Program Signature Catalog v1

Issue: OPENAGENTS-BP-011 / #231

This note records the first seeded Autopilot continuation Program Signature
catalog. The source of truth is
`workers/api/src/blueprint/fixtures/autopilot-continuation-signatures.ts`.

## Covered Actions

- continue
- test
- fix
- summarize
- request context
- retry account
- stop
- prepare review
- route selection
- research policy
- email decisioning
- proof projection

Each action has a draft Program Signature, a candidate Module Version, and a
Release Gate placeholder.

## Promotion Boundary

The catalog is not production-promoted. All seeded signatures are draft, all
seeded module versions require operator promotion, and all release gates are not
yet promotable. Production use requires fixtures, scorecards, receipts, rollback
evidence, operator review, policy checks, and explicit release decisions.
