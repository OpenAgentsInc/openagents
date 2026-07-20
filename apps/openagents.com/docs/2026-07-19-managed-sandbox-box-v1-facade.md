# Managed Sandbox Box-v1 Facade Runbook

Date: 2026-07-19
Issue: [SBX-03 #9025](https://github.com/OpenAgentsInc/openagents/issues/9025)

## Operational state

The OpenAgents Worker owns a bounded Box-v1 compatibility route at `/v1`.
It is armed only on the exact SBX-09 staging revision. Keep it default-off in
production until SBX-09 gives independent proof. That proof must cover the
deployed SDK, runtime, isolation, cleanup, cost, rollback, Desktop, and Sarah
journeys.

The facade is an HTTP adapter over `openagents.managed_sandbox.v1`. The native
Postgres store remains the sole lifecycle, command, event, projection-cursor,
and receipt authority. Box responses are an explicitly lossy projection and
never publish guest addresses, provider URLs, desktop endpoints, snapshots,
subdomains, raw topology, or credentials.

## Configuration

The route is absent unless all admission inputs are valid:

- `MANAGED_SANDBOX_BOX_V1_ENABLED=true` arms route selection. The default is
  off and must remain so through SBX-09.
- `OA_MANAGED_SANDBOX_IMAGE_DIGEST=sha256:<64 lowercase hex>` pins the admitted
  immutable guest image.
- The Worker must have its canonical Postgres/Cloud SQL configuration.
- Requests use an OpenAgents programmatic-agent bearer. The credential is
  resolved to its linked OpenAuth owner when present. Tenant scope is derived
  server-side. A caller cannot choose owner or tenant scope.

The current policy fixes `us-central1` and the OpenAgents-managed GCP target.
It fixes `profile.sbx.gce.e2-small.v1` and two active boxes per owner scope.
TTL is from 60 to 86,400 seconds. The native resource budget is bounded.
A change to those pins is a policy and invariant change, not a route tweak.

## Phase 1 route set

The admitted SDK methods are account/limits, list/create/get/update/delete,
stop/resume, prompt/status/events/interrupt, file read/write, command, and
artifact read. Snapshot, fork, desktop, SSH, repositories, API-key metadata,
and account-secret methods return a typed `501 capability_not_implemented`
envelope with a request ref.

Lifecycle mutations require `Idempotency-Key`. Exact retries read and return
the durable native reservation. A key with different bytes returns
`409 conflict`. Event pages use native order and an opaque cursor with resource
generation data. A cursor from a prior generation returns `409` after resume.

## Current runtime boundary

Create, stop, resume, and delete enter the same canonical owner broker used by
Desktop and Sarah. The compatibility response is not emitted from a stored
intent alone. The broker executes the private lifecycle adapter and settles the
provider outcome into native events and a native receipt. Exact retries replay
that settled result without a second provider effect.

TTL updates replace the native lease and budget lifetime together. When a TTL
is shortened, all retained capability expirations are clipped to the new lease
expiry in the same durable update. A capability cannot outlive its lease.

SBX-03 does not fabricate guest execution. SBX-04 connects prompt, status,
events, and interrupt to the private turn adapter. SBX-05 connects files,
commands, and artifacts to the private guest I/O adapter.

Both adapters require an exact configured private control dependency. They
fail typed and closed when it is absent. Lifecycle calls still use the native
Postgres authority, and the public flag remains off through SBX-09.

## Verification

From the repository root:

```sh
./node_modules/.bin/tsc -p packages/managed-sandbox-contract/tsconfig.json --noEmit
./node_modules/.bin/tsc -p packages/khala-sync-server/tsconfig.json --noEmit
./node_modules/.bin/tsc -p apps/openagents.com/workers/api/tsconfig.json --noEmit
./node_modules/.bin/vp test --run apps/openagents.com/workers/api/src/managed-sandbox-box-v1-routes.test.ts
```

The test imports the unmodified pinned SDK and runs one corpus twice. The first
run uses an in-process fetch adapter. The second run uses a loopback Node HTTP
server and normal SDK fetch. The test covers exact retry, changed-byte conflict,
cross-owner denial, and stale-generation cursor refusal. It also covers the
default-off state, typed runtime unavailability, unsupported operations, and
401/503 authentication behavior for every admitted method.

The loopback server is staged transport evidence, not an external deployment
or live-GCP acceptance result. The retained public-safe receipt is
`docs/sol/evidence/2026-07-19-sbx03-box-v1-conformance.json`.
