# Blueprint Optimizer Run And Candidate Module Records v1

Issue: OPENAGENTS-BP-012 / #232

This note records the typed Optimizer Run and candidate Module Version model.
The source of truth is `workers/api/src/blueprint/schemas/optimizer-run.ts`.

## Purpose

Optimizer Runs let retained failures produce candidate module versions and
scorecards without changing production behavior.

The v1 model records:

- optimizer kind;
- run status;
- retained failure refs;
- evidence refs;
- scorecard refs;
- candidate module refs;
- candidate review state;
- release gate refs.

## No Self-Promotion

Optimizer output is evidence-only until release-gated. Candidate module versions
must remain candidates that require operator promotion. If a candidate appears
as production or self-promoted, the optimizer output predicate rejects it.

## Current Limits

This is a contract and predicate layer only. Persistence, optimizer execution,
candidate diff rendering, retained-failure replay, release-gate promotion, and
Program Registry UI are separate roadmap issues.
