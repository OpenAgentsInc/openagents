# business.marketing_agency_workspace_pack.v1 — gemini-fleet

Promise state: **yellow** (unchanged by this work — no green flip).

## What this run built

A verification function and fixture for a self-serve deliverability state, explicitly targeting the self-serve missing blocker for the marketing-agency workspace pack.

- `apps/openagents.com/workers/api/src/marketing-agency-self-serve-deliverability.ts`: Verification logic for checking if a marketing-agency workspace has both the publish channel (custom hostname) and send channel (DKIM/SPF) fully active.
- `apps/openagents.com/workers/api/src/marketing-agency-self-serve-deliverability.test.ts`: Verification unit tests testing various pending states.
- `apps/openagents.com/workers/api/src/marketing-agency-self-serve-fixture.ts`: A self-serve deliverability fixture demonstrating a fully active set of send and publish permissions without operator intervention.
- `apps/openagents.com/workers/api/src/marketing-agency-self-serve-claim-upgrade.ts` & `.test.ts`: Claim upgrade projection logic mapping the self-serve deliverability proof to the registry schema, clearing the `marketing_agency_pack_self_serve_missing` blocker when substantiated.

## Which blocker this advances

`blocker.product_promises.marketing_agency_pack_self_serve_missing`
— **advanced.** This provides the foundational schema, verifier, and the claim upgrade projection mapping needed to assess if the self-serve deliverability is fully active for a given workspace and project it as a substantiated claim.

## What remains for green

1. Wire the self-serve deliverability into the workspace creation or delivery flow to actually allow self-serve delivery.
2. Ensure the paid delivery receipt and self-serve proven status are exposed over public HTTP routes.

## Follow-up (Self-Serve Delivery Route)

A subsequent run exposed the self-serve deliverability fixture over a public HTTP route, advancing the exposure requirement:
- `apps/openagents.com/workers/api/src/marketing-agency-self-serve-public-routes.ts` & `.test.ts`: Exposes the self-serve deliverability proof at `GET /api/public/marketing-agency/self-serve/deliverability/{workspaceRef}`.
- Registered the new route into the `check-zero-debt-architecture.mjs` staleness ledger and `INVARIANTS.md`.
