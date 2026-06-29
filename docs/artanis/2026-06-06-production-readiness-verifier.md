# Artanis Production Readiness Verifier

Issue: #416 / `ARTANIS-030`

Status: implemented as a read-only verifier contract and scriptable JSON
command.

## Purpose

The deployment-readiness audit found that Artanis has source-level contracts
and local tests, but production was still missing the evidence needed before a
scheduled production runner can be enabled.

The verifier makes that state repeatable. It summarizes whether production has
the required source, deploy parity, D1 persistence, Pylon release, smoke, and
scheduler-readiness evidence.

It does not deploy, apply migrations, write D1 rows, post to Forum, mutate
GitHub releases, dispatch Pylon work, change scheduler state, spend bitcoin,
or upgrade public claims.

## Implementation

Code lives in:

- `workers/api/src/artanis-production-readiness-verifier.ts`
- `workers/api/src/artanis-production-readiness-verifier.test.ts`
- `scripts/artanis-production-readiness.mjs`
- `scripts/artanis-production-readiness.test.ts`

The root command alias is:

```bash
bun run artanis:readiness -- --help
```

## What It Checks

The typed verifier projects these readiness stages:

- `source_ready`
- `deployed_parity_ready`
- `persistence_ready`
- `release_ready`
- `smoke_ready`
- `scheduler_ready`

The concrete checks are:

- source commit ref;
- public Artanis report fields, including `productionLaunchGate`,
  `pylonLaunchCommunication`, and `forumRewardSmoke`;
- `/artanis` page reachability;
- expected `artanis_*` D1 table names;
- canonical Artanis Forum status topic evidence;
- `/api/public/pylon-stats` availability;
- Pylon v0.2 release tag and asset evidence;
- retained production-equivalent smoke ref;
- scheduler enablement state.

The retained Forum delivery/listener verification contract from #418 is
documented in
`docs/artanis/2026-06-06-forum-delivery-listener-verification.md`. Use it to
retain the canonical topic, delivered-post, idempotency, listener, triage, and
blocker evidence that supports the smoke-ready stage.

## Safe Command

The script fetches public routes and accepts read-only operator observations as
flags. It does not call Wrangler itself.

```bash
bun run artanis:readiness -- \
  --d1-tables "artanis_runtime_snapshots,artanis_loop_records" \
  --latest-pylon-release-tag "pylon-v0.1.23" \
  --scheduled-runner disabled \
  --source-commit "commit.public.autopilot_openagents.3b24bf68"
```

When an operator has a read-only D1 table list, pass it through
`--d1-tables`. When a production-equivalent smoke exists, pass its public-safe
ref through `--production-smoke-ref`. The retained-smoke contract for creating
that evidence is documented in
`docs/artanis/2026-06-06-retained-production-launch-smoke.md`. When Pylon v0.2
release evidence exists, pass both `--pylon-v02-release-tag pylon-v0.2.0` and
`--pylon-v02-release-assets <count>`.

## Claim Boundary

The verifier can say:

```text
Artanis production readiness is blocked or ready according to retained
read-only evidence.
```

The verifier cannot say:

```text
Artanis is continuously running autonomously.
Pylon v0.2 is shipped.
The scheduled runner is approved for production.
Accepted work is paid or settled.
```

Those stronger claims require retained production-equivalent smoke,
Forum/listener verification, Pylon release-parity evidence, controlled deploy
and scheduler enablement, and public receipt chains.

## Verification

Focused checks:

```bash
bun run --cwd workers/api test -- src/artanis-production-readiness-verifier.test.ts
bun test scripts/artanis-production-readiness.test.ts
bun run --cwd workers/api typecheck
```
