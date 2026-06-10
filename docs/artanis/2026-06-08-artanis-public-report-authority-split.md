# Artanis Public Report Authority Split

Issue: [#567](https://github.com/OpenAgentsInc/openagents/issues/567)

The Artanis public report can be used for launch copy only when it preserves the
authority split between public status projection and risky actions.

## Public Report Fields

`authoritySummary` exposes separate booleans for:

- `statusProjectionAllowed`;
- `dispatchAuthorityAllowed`;
- `spendAuthorityAllowed`;
- `settlementAuthorityAllowed`;
- `providerMutationAuthorityAllowed`;
- `forumAutoPublishAllowed`;
- `scheduledRunnerDispatchAllowed`;
- `dispatcherGateGreen`;
- `greenLaunchCopyAllowed`;
- `operatorApprovalRequired`.

The normal retained GEPA scheduled runner may make
`statusProjectionAllowed: true`, but it keeps dispatch, spend, settlement,
provider mutation, and Forum auto-publish false.

## Green Copy Gate

`greenLaunchCopyAllowed` is stricter than retained status projection. It is
blocked when Artanis health is stale, blocked, degraded, unavailable, or
unknown.

Stale health produces:

```text
blocker.public.artanis.green_launch_copy.health_stale
```

## Risky Authority Blockers

The public report emits stable blockers for risky actions that remain outside
the scheduled status runner:

- `blocker.public.artanis.dispatch_authority_not_granted`
- `blocker.public.artanis.spend_authority_not_granted`
- `blocker.public.artanis.settlement_authority_not_granted`
- `blocker.public.artanis.provider_mutation_not_granted`
- `blocker.public.artanis.forum_auto_publish_not_granted`

## Runbook And Idempotency Refs

The public report also exposes launch runbook command refs and Forum intent
idempotency refs so operators can verify pause, disable, revoke, and no-duplicate
publication safeguards without seeing private runner state.

## Verification

Regression coverage lives in:

- `workers/api/src/artanis-public-report.test.ts`
- `workers/api/src/artanis-production-launch-gate.test.ts`
- `workers/api/src/artanis-retained-launch-smoke.test.ts`
- `workers/api/src/artanis-gepa-scheduled-runner-proof.test.ts`
