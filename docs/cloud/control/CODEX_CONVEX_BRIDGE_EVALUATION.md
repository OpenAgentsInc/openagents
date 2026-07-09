# Codex Convex Bridge Evaluation

Status: design decision
Last updated: 2026-06-01

## Decision

Keep the Vortex HTTP callback path as the Codex VM MVP transport.

A direct Rust Convex bridge from SHC is useful for MVP+ reliability and lower
latency, but it is not safe to enable until Vortex exposes narrow
runner-scoped Convex functions and a dedicated runner service identity. Do not
put a broad Convex admin key, production admin token, user token, or general
Vortex mutation authority on SHC.

The bridge, when built, is a transport optimization only. Vortex remains the
durable product authority for run validation, org/project visibility,
retention policy, public/private projection, acceptance, and claim state.

## Convex Rust Client Findings

The official Rust crate is viable for a future bridge:

- `convex = "0.10.4"` is the current docs.rs version reviewed for this note.
- `ConvexClient::new` creates a Tokio-backed client for a deployment URL.
- `ConvexClient` supports `query`, `mutation`, `action`, `subscribe`, and
  `watch_all`, which covers event append plus pending command watches.
- `ConvexClient::set_auth_callback` can fetch fresh auth on WebSocket
  reconnect, which is the right shape for a runner service token that can
  expire and rotate.
- `AuthenticationToken` supports `Admin`, `User`, and `None`; the SHC runner
  path should use a runner-service JWT/OIDC shape and must not store a broad
  Convex admin token on SHC.
- The 0.10.4 changelog includes a query-subscription memory leak fix and a
  Rust MSRV bump to 1.85, which is compatible with the current Cloud toolchain.

References:

- https://docs.rs/convex/latest/convex/
- https://docs.rs/convex/latest/convex/struct.ConvexClient.html
- https://docs.rs/convex/latest/convex/type.AuthTokenFetcher.html
- https://docs.rs/convex/latest/convex/enum.AuthenticationToken.html
- https://docs.rs/crate/convex/latest/source/CHANGELOG.md

## Safe Bridge Shape

The direct bridge should be a companion worker or optional transport inside
`oa-codex-control`, guarded by an explicit feature/env flag. It should be
disabled by default while HTTP callbacks are the MVP path.

Suggested env:

```text
OA_CONVEX_BRIDGE_ENABLED=1
OA_CONVEX_DEPLOYMENT_URL=https://<deployment>.convex.cloud
OA_CONVEX_RUNNER_ID=oa-shc-katy-01
OA_CONVEX_RUNNER_TOKEN_COMMAND=/usr/local/bin/oa-runner-token
```

`OA_CONVEX_RUNNER_TOKEN_COMMAND` should return a short-lived runner token from
Vortex or a broker. The command must not print raw ChatGPT/Codex credentials,
provider secrets, wallet material, GCP credentials, or a broad Convex admin
key.

Allowed Convex surface:

```text
internal.codexRunner.appendEvent
internal.codexRunner.heartbeat
internal.codexRunner.updateExternalStatus
internal.codexRunner.attachArtifactRef
internal.codexRunner.pendingCommands
internal.codexRunner.ackCommand
```

Every function must validate:

- token subject is a runner service identity, not a user browser session;
- `runnerId` matches the token claims;
- `externalRunId` or `runId` is assigned to that runner;
- `externalEventId` is unique/idempotent for the run;
- event detail is already redacted and bounded;
- artifact and receipt fields are refs/digests only;
- local-only or opt-out runs cannot leak raw transcript data into Convex.

## Command Subscription

The bridge may subscribe to a query for pending commands assigned to the
runner:

```text
cancel
continue
steer
checkpoint
archive
destroy
```

The runner must persist command receipt locally before acting, then call
`ackCommand` after it records the resulting event. If the WebSocket disconnects
or the process restarts, the local HTTP/SSE status path remains the recovery
surface and the bridge resubscribes after startup.

## Idempotency

The bridge should submit the `openagents.runner_event.v1` event emitted by
`oa-workroomd` without changing the event type. Convex should dedupe by:

```text
runId + externalEventId
```

If a retry races with a previous successful mutation, the mutation should
return the existing canonical event sequence instead of appending a duplicate.

## Retention And Opt-Out

Vortex owns the retention policy. The bridge must read a run-level retention
mode before streaming details:

```text
convex_retained
convex_redacted_only
local_only
```

For `local_only`, SHC can continue to serve status/events through
`oa-codex-control` local APIs, but it must not append message/tool/shell detail
to Convex. It may append a minimal redacted heartbeat/status receipt if Vortex
has explicitly modeled that as part of the opt-out contract.

## Threat Model

Do not enable the direct bridge if any of these would be true:

- SHC has a Convex deploy/admin key with broad mutation access.
- The runner token can call general thread, user, project, billing, or
  projection functions.
- The runner can assign itself new runs.
- The runner can override acceptance, grading, claim state, or visibility.
- Raw Codex `auth.json`, provider tokens, wallet material, or cloud credentials
  can pass through events.
- Vortex does not dedupe external event IDs.

## Implementation Recommendation

Do not add the `convex` crate to the production binary in the current MVP
commit. The current HTTP callback path is simpler, already implemented, and
keeps all Convex authority inside Vortex.

Build the direct bridge after Vortex lands the runner-scoped Convex function
surface and service-token minting. At that point Cloud should add:

1. `oa-convex-bridge` or an optional `oa-codex-control` module behind a Cargo
   feature.
2. A small trait for event append and pending-command watch so tests can use a
   fake client.
3. Token-command based auth refresh wired through `set_auth_callback`.
4. Idempotent append tests using duplicate `externalEventId` fixtures.
5. Reconnect/resubscribe tests for cancel/continue command watching.

Until then, `OA_VORTEX_CODEX_INGEST_URL` remains the supported ingestion path.
