# SBX-08 managed-sandbox supervision

This record describes the default-off mobile and authenticated-web controller
components from SBX-08. It is not a production rollout record. SBX-09 owns the
live Google Cloud acceptance and rollout decision.

## Contract

Both clients decode
`@openagentsinc/managed-sandbox-contract/supervision`. The Worker serves the
same owner-scoped projection at these paths:

- `GET|POST /api/managed-sandboxes/mobile/supervision`
- `GET|POST /api/managed-sandboxes/web/supervision`

The projection contains the safe target, generations, lifecycle, effective
runtime and actor, attention, elapsed and idle time, lease, budget, last
structural event, safe outcome refs, and cleanup truth. It does not contain an
owner or tenant ref, prompt, runtime output, image digest, credential, provider
topology, raw path, PTY data, or shell capability.

The only client commands are interrupt, stop, resume, and delete. Each command
binds the surface, sandbox, version, resource generation, issue time, expiry,
and native idempotency ref. The route rejects a surface substitution, expired
command, stale generation, or unauthenticated caller before a target effect.

## Mobile composition

`MobileSyncHost` owns the access token and a table in the existing Expo SQLite
database. The controller writes the exact JSON body before send. An offline or
temporary failure leaves the row pending. The next reconciliation sends the
same bytes. A durable native outcome settles the row. The Effect Native screen
receives only the decoded projection and outcome.

## Web composition

The Start server proxy reads the existing Khala Sync HTTP-only owner cookies.
It adds the bearer only to its server-to-server request to the Worker. Browser
code never receives the bearer. Browser local storage retains the exact typed
command body before send and reuses it after an offline gap.

## Enablement and verification

The routes use `MANAGED_SANDBOX_BROKER_ENABLED`. Keep this flag off until
SBX-09 records independent live Google Cloud, isolation, cost, cleanup,
rollback, packaged Desktop, owner-thread, and cross-surface evidence.

Run the deterministic component checks from the repository root:

```sh
pnpm --dir packages/managed-sandbox-contract run test
pnpm --dir apps/openagents-mobile run test
pnpm --dir apps/openagents.com/apps/start run test
pnpm --dir apps/openagents.com/workers/api run test -- managed-sandbox-supervision-routes.test.ts
```

These checks prove the component boundary. They do not prove a deployed
target, cost observation, installed physical device, or production rollout.
