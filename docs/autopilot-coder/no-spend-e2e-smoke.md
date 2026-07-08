# Autopilot Coder No-Spend E2E Smoke

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Run from the repository root:

```sh
bun run --cwd apps/openagents.com/workers/api smoke:autopilot-coder:no-spend
```

The smoke drives the public no-spend Autopilot Coder path through HTTP route
handlers:

1. Submit a typed public Autopilot work request.
2. Select a compatible requester Pylon.
3. Create the durable Pylon assignment lease.
4. Accept the assignment through the Pylon API.
5. Submit worker closeout refs through the Pylon API.
6. Recover Autopilot detail and event projections.
7. Submit an owner-granted review decision.
8. Scan the retained projection for private paths, wallet/payment material,
   provider payloads, raw prompts/logs/source archives, secret material, and
   forbidden hosted-infrastructure wording.

The smoke is no-spend: buyer funding remains `not_required`, worker payout and
settlement remain ineligible, and no deploy, spend, or Forum publication
authority is granted.

This command is the CI-safe route smoke for the no-spend Pylon path. It does
not pass the test-only `pylonRegistrations` placement dependency and does not
use an injected hosted executor. Placement reads the Pylon API store, creates a
durable assignment lease, and the closeout travels through the Pylon assignment
API before Autopilot ingests the delivered refs.

The companion Pylon runtime regression is:

```sh
bun test --cwd apps/pylon tests/assignment.test.ts tests/live-worker-loop-smoke.test.ts
```

That Pylon suite verifies assignment normalization, local lease admission,
accept/progress/proof/closeout behavior, cancellation handling, and public-safe
closeout receipts. A staging/live no-spend run against deployed credentials is
still required before expanding public product claims beyond this CI-safe
contract.
