# Artanis Autonomous Tick, Claim, And Closeout Loop

Date: 2026-06-06

Issue: #387 / `ARTANIS-002`

Status: implemented as a read-only schema/projection contract in
`workers/api/src/artanis-loop.ts`.

## Purpose

This contract models the durable autonomous Artanis loop:

1. wake,
2. claim a scoped loop,
3. select allowed context,
4. propose safe or approval-gated actions,
5. record blockers and approval requirements,
6. record receipts and artifacts,
7. prepare Forum publication intents,
8. close out the tick,
9. schedule the next pass.

It does not run a scheduler by itself. It gives OpenAgents product surface a tested, typed loop
ledger that a future runtime, `/autopilot`, `/artanis`, and Forum projection
can consume without inventing authority from log text.

## Loop Rules

- Loop ledgers must use `agent_artanis`.
- Only one active Artanis loop is allowed per scope.
- Loops require goal refs and ticks.
- Ticks carry stable idempotency keys.
- Duplicate ticks with the same idempotency key are suppressed in projection
  while their duplicate tick refs remain visible for audit.
- Blocked loops and ticks require blocker refs.
- Approval-waiting ticks require approval requirement records.
- Completed ticks require closeout receipt refs, artifact refs, Forum
  publication intent refs, and a next tick schedule.

## Risk Rules

Risky action kinds include:

- wallet spend,
- provider mutation,
- training launch,
- eval launch,
- runtime promotion,
- Forum publication.

Risky actions cannot be marked safe. They require approval requirement refs and
separate authority receipt refs. The loop contract itself still denies
deployment, eval launch, Forum publish, payment spend, provider mutation,
runtime promotion, training launch, and wallet spend.

## Projection

`projectArtanisLoopLedger(ledger, audience, nowIso)` returns an
`ArtanisLoopLedgerProjection` with:

- friendly time labels,
- loop and tick state,
- canonical idempotent ticks,
- duplicate tick refs for audit,
- blocker and approval refs,
- selected context refs,
- proposed actions,
- artifacts,
- receipts,
- closeout receipt refs,
- Forum publication intent refs,
- hard false authority booleans.

Public, agent, and customer projections redact private refs. Team and operator
projections can inspect more refs after validation, but raw provider, runner,
wallet, payment, customer, private repo, secret, raw prompt, raw log, or raw
timestamp material is rejected before projection.

## Tests

Coverage lives in `workers/api/src/artanis-loop.test.ts`. The tests cover:

- closeout refs, artifacts, Forum publication intents, and next schedules,
- duplicate tick suppression,
- one active loop per scope,
- blocked and waiting-for-approval state requirements,
- completed tick closeout requirements,
- risky action denial without approval and authority refs,
- unsafe material and false-authority rejection.
