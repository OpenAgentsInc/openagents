# Autopilot Coder No-Spend E2E Smoke

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
