# Artanis Scheduled Tick Runner

Issue #404 / `ARTANIS-018` adds the first Worker-compatible Artanis scheduled
runner path.

The implementation lives in:

- `workers/api/src/artanis-scheduled-runner.ts`
- `workers/api/src/artanis-scheduled-runner.test.ts`
- `workers/api/src/test/artanis-persistence-fixture.ts`

## Rollout State

The runner is wired into the existing Worker `scheduled` handler, but it is
disabled by default. Production execution requires:

```text
ARTANIS_SCHEDULED_RUNNER_ENABLED=true
```

Without that flag, the cron path returns a disabled result and writes no
Artanis rows. This keeps the minute cron trigger from making autonomy claims
before the operator launch gate is finished.

## What One Tick Does

When explicitly enabled, one scheduled tick:

- claims the deterministic Artanis loop for `scope.public.artanis.global`;
- loads public-safe Pylon/Nexus refs, Model Lab public refs, persisted-state
  refs, operator-steering refs, runner-backend refs, and operator-only Model
  Lab context refs;
- persists a runtime snapshot, loop claim, loop tick, health snapshot,
  work-routing proposal, approval gate, Forum publication intent, and loop
  closeout receipt;
- records the executor-trace work class as the first autonomous lane:
  no-spend Pylon dispatch refs, exact-replay verdict refs, deterministic
  closeout receipts, and the Tassadar executor capability requirement;
- records a `wallet_spend` approval requirement and pending approval gate for
  the optional paid sample, with the operator spend-enable as the authority
  ref;
- queues a Forum publication intent whose body is pinned to the
  `compute.tassadar_executor_poc.v1` promise safeCopy and no broader claim;
- records the next tick schedule;
- records risky work only as approval-gate evidence;
- leaves every persistence receipt with `executableAuthority: false`.

The runner does not post to Forum, spend bitcoin, redeem L402, mutate provider
state, dispatch Pylon work, launch evals, launch training, install adapters,
deploy, promote runtime behavior, settle payouts, or write public claims.

## Idempotency

The schedule ref is converted into stable record refs and idempotency keys.
Retrying the same scheduled tick returns idempotent persistence receipts and
does not duplicate loop ticks, Forum intents, health snapshots, work proposals,
approval gates, or runtime snapshots.

The loop claim uses a deterministic scope-based idempotency key so the first
enabled tick creates one active loop for the global Artanis scope. Later ticks
persist separate tick records under that loop rather than attempting to create
another active loop for the same scope.

## Local Smoke

Run the focused smoke with:

```bash
bun run --cwd workers/api test -- src/artanis-scheduled-runner.test.ts src/config.test.ts
```

The smoke covers:

- disabled-by-default behavior;
- one enabled tick persisted through closeout;
- duplicate retry collapse;
- Worker adapter flag handling;
- false authority for spend, L402 redemption, provider mutation, Pylon job
  dispatch, eval/training launch, adapter install, deployment, runtime
  promotion, settlement, Forum publish, and wallet spend;
- executor-trace tick refs, approval requirements, and safeCopy-pinned Forum
  intent copy.

## Remaining Gates

Before production auto-run can be enabled:

- #406 now delivers queued Artanis publication intents to Forum through the
  approved Forum posting path. As of #512, scheduled operation is retained only
  for bounded public-safe GEPA status projection.
- #407 now adds the read-only Forum listener and triage loop, but the scheduled
  runner remains disabled until the listener is wired to live persisted Forum
  observations and the launch gate is complete.
- #408 now connects the Nexus/Pylon admin adapter contract for public
  read-only fleet monitoring, approval-gated fake dispatch route receipts, and
  D1 persistence. Live Pylon job dispatch remains disabled until the launch
  gate and target-specific executor authority are implemented.
- #414 now adds the production launch gate and runbook. The gate currently
  remains blocked until a production or production-equivalent launch smoke is
  retained and `ARTANIS_SCHEDULED_RUNNER_ENABLED=true` is enabled during an
  operator-controlled launch window.

## 2026-06-08 Status

OpenAgents product surface #511 added retained Probe GEPA/Pylon production-equivalent smoke
evidence. OpenAgents product surface #512 adds the separate bounded scheduled-runner proof for
public-safe GEPA status projection. openagents#4697 adds the executor-trace
work class to the same runner contract. The runner can now clear only bounded
scheduled-runner wiring evidence: no live production launch claim, no automatic
Forum publishing, no provider mutation, no model training, no runtime
promotion, no settlement mutation, and no wallet spend until the remaining
product-promise gates have their receipts.
