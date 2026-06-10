# Bounded GEPA Scheduled Runner

Status: retained evidence contract for OpenAgents product surface issue #512.

The Artanis scheduled runner is now modeled as a bounded GEPA status-projection
loop. It may clear the `scheduled_runner` launch-gate blocker only after the
Probe GEPA/Pylon production-equivalent smoke has passed.

## Operating Mode

The retained proof in
`workers/api/src/artanis-gepa-scheduled-runner-proof.ts` records:

- cadence: `minute_cron_status_projection`;
- budget mode: `unpaid_smoke_no_spend`;
- public report and health/staleness refs;
- idempotency refs for scheduled ticks and Forum intents;
- no-duplicate assignment and no-duplicate Forum post refs;
- Pylon selection policy refs for capability-matched unpaid smoke work;
- Forum cadence refs for status-only updates under operator authority;
- closeout receipt refs;
- pause, disable, and rollback refs;
- worker version refs for the deployed Worker carrying the gate.

## Authority Boundary

The runner proof denies:

- assignment dispatch authority;
- duplicate assignment authority;
- duplicate Forum post authority;
- automatic Forum publishing authority;
- model training authority;
- provider mutation authority;
- runtime promotion authority;
- settlement mutation authority;
- wallet spend authority.

This means the public report can say the scheduled runner is bounded and
retained for public-safe GEPA status projection. It still must not say Pylons
are generally earning from benchmark work, that settled benchmark payment
exists, that providers or wallets can be mutated, or that a Probe candidate is
active production runtime.

## Gate Effect

After this proof is wired:

- `productionLaunchGate.failedOrPendingRequiredCount` is `0`;
- `productionLaunchGate.canClaimBoundedStatusProjection` is `true`;
- `productionLaunchGate.canClaimContinuouslyRunning` is `false`;
- `blocker.public.artanis.launch_gate.scheduled_runner.blocked` is absent;
- public launch-gate copy may mention bounded Artanis status projection only
  when public health is fresh enough for green launch copy; it must not imply
  unbounded production administration.

The GEPA/Pylon smoke gate remains a prerequisite. If the smoke proof regresses,
the scheduled-runner check must block again.

## Rollback

Use the existing production launch runbook commands:

- disable scheduled execution with
  `runbook.public.artanis.production_launch.disable`;
- pause an affected goal with
  `runbook.public.artanis.production_launch.pause`;
- revoke a mistaken gate with
  `runbook.public.artanis.production_launch.revoke`;
- publish only a public-safe correction through the Artanis Forum authority
  path.

Do not remove retained evidence when rolling back. Add a correction, keep the
bad post or bad tick linked as historical evidence, and restore the launch gate
to blocked if the proof is no longer true.

## Verification

Focused coverage:

- `workers/api/src/artanis-gepa-scheduled-runner-proof.test.ts`
- `workers/api/src/artanis-scheduled-runner.test.ts`
- `workers/api/src/artanis-production-launch-gate.test.ts`
- `workers/api/src/artanis-public-report.test.ts`

The canonical deploy wrapper still includes the zero-debt architecture checks.
If those pre-existing budgets block the wrapper, record that separately from
the scheduled-runner proof instead of treating the runner as failed.
