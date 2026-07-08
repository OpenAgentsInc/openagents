# Autopilot Coder Paid E2E Smoke

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Run from the repository root:

```sh
bun run --cwd apps/openagents.com/workers/api smoke:autopilot-coder:paid
```

The smoke drives the payable public Autopilot Coder path through HTTP route
handlers and the Pylon assignment API:

1. Submit a typed payable Autopilot work request.
2. Receive HTTP `402` with signed L402 retry metadata.
3. Retry the same idempotency key with a verifier-approved L402 proof ref.
4. Confirm the work order moves to funded `queued_or_running` state.
5. Confirm the selected requester Pylon receives a buyer-funded coding
   assignment payload.
6. Accept the assignment through the Pylon API.
7. Submit worker closeout refs through the Pylon API.
8. Recover Autopilot delivered detail and event projections.
9. Submit an owner-granted review decision.
10. Confirm worker payout, settlement, deploy, spend, and Forum publication
    authority remain blocked.
11. Scan the retained projection for private paths, wallet/payment material,
    provider payloads, raw prompts/logs/source archives, secret material, and
    forbidden hosted-infrastructure wording.

This is a CI-safe paid-route smoke. It uses the same signed L402 credential
contract and buyer-payment ledger verifier as production wiring. The test
records a public-safe matched ledger redemption bundle, then proves the route
will not fund work until that ledger state exists.

The staging/live paid smoke is still required before the product can honestly
claim that an agent can pay for Autopilot coding work end to end. That later
smoke must use a deployed endpoint, a real MDK/L402 verifier, an agent wallet
or checkout flow, live registered-agent credentials, and a real worker
execution lane. No external payment movement occurs in the CI-safe smoke.
