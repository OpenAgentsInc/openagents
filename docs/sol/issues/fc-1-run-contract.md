# FC-1: authenticated Sarah fleet tool and durable run contract

Parent: #8638

## Outcome

An authenticated owner can tell Sarah to run a bounded issue list or plan DAG
on their coding fleet. Sarah creates one durable FleetRun request through a
typed tool; no CLI invocation is required to create the run.

## Current seams

- Sarah's owned runtime tool inventory lives in
  `apps/sarah/src/agent-runtime/owned-runtime.ts` and the realtime tool bridge.
- Fleet types and orchestration live in the Pylon orchestration store and
  `@openagentsinc/khala-fleet-intents`.
- Khala caller-owned delegation already supports pinned public repository work
  for Codex/Claude targeted to an owner-linked Pylon.
- The desktop `fleet_run_start` tool owns a useful implementation, but its
  manager and SQLite store are process-local and not reachable from Sarah.

## Scope

1. Define a narrow Sarah tool such as `coding_fleet_start` using the existing
   FleetRun/plan-DAG and `FleetWorkerKind` vocabulary. Do not create a second
   steering schema.
2. Require authenticated owner scope. Prospect mode cannot see or start coding
   work.
3. Accept public-safe objective, pinned repo/branch/commit, bounded verifier,
   issue list or plan DAG, target concurrency, and typed worker policy.
4. Persist the run request and work units under owner + run refs in a durable
   server authority. Record explicit local-Pylon versus managed-cloud target
   preference without allowing another owner's capacity.
5. Project only bounded run metadata back to Sarah; keep prompts, diff content,
   shell output, and local paths private.
6. Add idempotency and duplicate-run protection.
7. Return a stable `runRef` immediately so Sarah can narrate and subscribe.

## Verification

- Schema decode/refusal tests for unauthenticated, malformed pins, unsafe
  prompts, unknown worker kinds, and duplicate idempotency.
- Owner-isolation tests across two users/Pylons.
- Fixture E2E: Sarah tool → durable run row → one work unit claimed → bounded
  closeout projection.
- Behavior contract: a tool utterance is never treated as execution authority.

## Exit

One authenticated `/sarah` fixture conversation creates a durable run that a
Pylon can claim without a supervising CLI process. The response carries the
stable run ref and no private execution material.
