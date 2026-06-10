# Controlled Pylon Assignment Dispatch Gate

Issue: <https://github.com/OpenAgentsInc/openagents/issues/553>

## Contract

`POST /api/operator/pylons/assignments` is the live DB-backed assignment lease
path for Pylon work. It is now guarded by
`gate.public.pylon.assignment_dispatch.controlled.v1` before a new assignment is
persisted.

The route accepts assignment dispatch only when the request includes public-safe
evidence for:

- campaign ref and campaign policy;
- selection policy;
- explicit payment mode;
- idempotency policy;
- pause policy and explicit `campaignPaused:false`;
- rollback path;
- closeout path;
- no-duplicate policy;
- no-Forum-publish policy and explicit `forumAutoPublishAllowed:false`;
- required Pylon capability refs; and
- spend-cap refs for paid modes.

The target Pylon must also be registered, active, wallet-ready, on a minimum
client version, freshly online within the five-minute dispatch window, and
capability-matched. The route denies duplicate unexpired active assignments for
the same Pylon.

## Authority Boundary

Assignment dispatch remains separate from spend and settlement:

- `walletSpendAllowed:false`;
- `settlementMutationAllowed:false`;
- `forumAutoPublishAllowed:false`; and
- `unpaid_smoke` responses carry `noSpendDispatch:true`.

Wallet readiness is receive/readiness evidence only. It is not outbound
capacity, accepted work, payout, settlement, or Forum publishing authority.

## Machine Checks

Regression coverage lives in `workers/api/src/pylon-api-routes.test.ts` and
covers:

- missing Pylon denial;
- offline Pylon denial;
- stale heartbeat denial;
- paused campaign denial;
- wrong capability denial;
- duplicate active lease denial;
- paid mode without spend-cap denial;
- missing campaign policy denial;
- automatic Forum publish denial; and
- no-spend assignment success with no wallet spend or settlement authority.

Run:

```bash
bun run --cwd workers/api test -- src/pylon-api-routes.test.ts
bun run typecheck:api
```
