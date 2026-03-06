<!-- Historical note: This audit is a point-in-time snapshot from its date. Current product and architecture authority lives in `README.md`, `docs/MVP.md`, and `docs/OWNERSHIP.md`. Deployment state, endpoint behavior, and repo structure may have changed after this audit. -->

# Audit: Nexus Relay Vs `nostr-rs-relay`

Date: 2026-03-06

## Scope

This audit answers four questions:

1. What is actually serving `nexus.openagents.com` now?
2. How capable is the current in-repo `apps/nexus-relay` service?
3. What does `/Users/christopherdavid/code/nostr-rs-relay` provide that the current relay does not?
4. What should the OpenAgents production Nexus setup become?

Sources reviewed:

- current repo:
  - `apps/nexus-relay/src/lib.rs`
  - `apps/nexus-relay/src/managed_groups.rs`
  - `apps/nexus-control/src/lib.rs`
  - `apps/autopilot-desktop/src/app_state.rs`
- local full relay repo:
  - `/Users/christopherdavid/code/nostr-rs-relay/README.md`
  - `/Users/christopherdavid/code/nostr-rs-relay/config.toml`
  - `/Users/christopherdavid/code/nostr-rs-relay/src/config.rs`
  - `/Users/christopherdavid/code/nostr-rs-relay/src/server.rs`
  - `/Users/christopherdavid/code/nostr-rs-relay/docs/reverse-proxy.md`
  - `/Users/christopherdavid/code/nostr-rs-relay/docs/database-maintenance.md`
- live deployment checks run on 2026-03-06:
  - `curl https://nexus.openagents.com/`
  - `curl https://nexus.openagents.com/api/stats`
  - websocket upgrade probe to `wss://nexus.openagents.com/`
  - `gcloud run services describe openagents-nexus-relay ...`
  - `gcloud run services describe openagents-nexus-control ...`
  - `gcloud beta run domain-mappings describe --domain=nexus.openagents.com ...`

## Executive Summary

`nexus.openagents.com` is now running the current repo's relay/control stack, but the relay portion is still an MVP relay harness, not a production-grade Nostr relay.

The current deployed shape is:

- `nexus.openagents.com` -> `openagents-nexus-relay`
- `openagents-nexus-relay` -> websocket relay surface plus HTML landing page plus `/api/*` and `/v1/*` proxying to `openagents-nexus-control`
- `openagents-nexus-control` -> desktop session bootstrap, sync token minting, starter-demand APIs, `/api/stats`, and kernel authority APIs

That is a valid Nexus host shape.

It is not a valid long-term production relay shape.

The core problem is simple:

> The current `apps/nexus-relay` keeps relay state in process memory (`Vec<Event>` + connected client map), while the deployed service is configured like a stateless Cloud Run web service.

That means:

- relay history is lost on restart or revision rollout,
- subscriptions and in-flight fanout are instance-local,
- scaling above one instance fragments the relay into disconnected islands,
- there is no durable event store,
- there is no NIP-11 relay info document,
- there is no relay metrics/admin/maintenance surface,
- and there is no serious abuse-control or operator posture beyond a small custom subset.

By contrast, `nostr-rs-relay` is an actual relay:

- persistent storage,
- broader NIP coverage,
- NIP-11,
- NIP-42,
- rate limits,
- metrics,
- database maintenance story,
- reverse-proxy deployment guidance,
- configurable relay page,
- optional external authorization hooks.

## Current Live Nexus State

As of 2026-03-06:

- `nexus.openagents.com` resolves to `ghs.googlehosted.com`
- Cloud Run domain mapping is ready
- `https://nexus.openagents.com/` serves the current repo's Nexus Relay HTML page
- `https://nexus.openagents.com/api/stats` serves the current repo's control stats JSON
- websocket upgrade on `/` returns `101 Switching Protocols`

Current deployed services:

- `openagents-nexus-relay`
  - latest revision: `openagents-nexus-relay-00001-kvz`
  - image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/nexus-relay:20260306-181422-e24fd104e`
  - env: `NEXUS_RELAY_CONTROL_BASE_URL=https://openagents-nexus-control-ezxz4mgdsq-uc.a.run.app`
  - max scale: `10`
- `openagents-nexus-control`
  - latest revision: `openagents-nexus-control-00001-zvx`
  - image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/nexus-control:20260306-181422-e24fd104e`
  - max scale: `10`

Live `/api/stats` currently reports:

- `service = nexus-control`
- `authority = openagents-hosted-nexus`
- `hosted_nexus_relay_url = wss://nexus.openagents.com/`
- `receipt_count = 0`
- `receipt_persistence_enabled = false`
- `sessions_active = 0`
- `sync_tokens_active = 0`

That means the control service is live, but still running with no persisted receipts.

## What The Current In-Repo Relay Actually Is

`apps/nexus-relay` currently provides:

- websocket handling on `/` and `/ws`
- relay HTML landing page on `/`
- proxying of `/api/*` and `/v1/*` to `nexus-control`
- basic `REQ`, `EVENT`, `CLOSE`, and `AUTH` frame handling
- in-memory replay and live fanout
- simple filter matching:
  - `kinds`
  - `authors`
  - `ids`
  - `since`
  - `until`
  - `#tag`
  - `limit`
- replaceable and addressable upsert behavior
- custom managed-group logic for restricted group reads/writes
- NIP-42 auth challenge/validation for the restricted managed-group path

It does **not** currently provide a durable relay backend.

### Exact structural limitation

The relay store is:

- `events: Vec<Event>`
- `clients: HashMap<u64, ConnectedClient>`
- `managed_groups: ManagedGroupsState`

all held inside process memory behind a Tokio `RwLock`.

There is no SQLite, no Postgres, no disk-backed append log, and no replication layer.

### Consequences

1. **Restart loss**
   - Relay history disappears on process restart, revision rollout, crash, or scale-to-zero resume.

2. **Multi-instance inconsistency**
   - Cloud Run can scale `openagents-nexus-relay` above one instance.
   - Each instance would have its own isolated `Vec<Event>` and its own connected clients.
   - A client connected to one instance would not see events accepted on another instance unless the sender happened to hit the same instance.

3. **No durable relay semantics**
   - Replay works only for the events currently stored in the running instance.
   - Historical queries are bounded by in-memory retention, not durable relay history.

4. **No operator-grade maintenance surface**
   - no database backup flow
   - no vacuum/maintenance flow
   - no metrics endpoint
   - no retention tooling beyond trimming the in-memory vector

5. **Protocol incompleteness**
   - There is no NIP-11 `application/nostr+json` relay info response at root.
   - There is no evidence of a serious, broad protocol implementation beyond the custom subset in this file.

6. **The current service is doing two jobs at once**
   - It is acting as relay
   - and as edge proxy for the control APIs
   - which makes it more of a Nexus edge/gateway than a dedicated relay

## What `nostr-rs-relay` Provides

`/Users/christopherdavid/code/nostr-rs-relay` is a proper relay codebase.

From the checked-in README and config/docs, it provides:

- persistent SQLite storage by default
- experimental Postgres support
- NIP-11 relay info document
- NIP-42 relay authentication
- NIP-01 / NIP-12 / NIP-15 / NIP-16 / NIP-20 / NIP-28 / NIP-33 / NIP-40 support
- NIP-05 verification features
- deletion handling (`NIP-09`)
- configurable relay info and relay page
- rate limiting and message size limits
- optional pay-to-relay features
- optional gRPC event-admission hooks
- Prometheus metrics at `/metrics`
- reverse-proxy deployment guidance
- database maintenance documentation

### Why this matters

This is the difference between:

- an app-specific relay harness

and

- an actual relay product.

`nostr-rs-relay` already has the boring but essential production surfaces:

- database model
- query/persistence pipeline
- maintenance guidance
- reverse-proxy model
- admin/operator tuning knobs

Those are exactly the things the current in-repo relay does not have.

## What `nostr-rs-relay` Does Not Replace

It does **not** replace `nexus-control`.

`nexus-control` owns the OpenAgents-specific HTTP authority surface:

- desktop session bootstrap
- sync token minting
- starter-demand buyer flow
- `/api/stats`
- `/v1/kernel/*`

So the right comparison is not:

- `apps/nexus-relay` vs `nexus-control`

It is:

- `apps/nexus-relay` vs `nostr-rs-relay`

And the answer is:

> `nostr-rs-relay` is the real relay; `apps/nexus-relay` is a lightweight OpenAgents-specific gateway/harness.

## Recommended Target Setup

### Strong recommendation

The production Nexus stack should become:

1. **Dedicated durable relay**
   - `nostr-rs-relay` (or equivalent full relay) should be the actual websocket/Nostr engine.

2. **Separate authority API**
   - `nexus-control` should remain the OpenAgents HTTP authority service.

3. **One public Nexus host**
   - `nexus.openagents.com` should stay the single public host.

4. **Edge router / reverse proxy in front**
   - Route relay traffic to the real relay.
   - Route `/api/*` and `/v1/*` to `nexus-control`.

### Recommended path layout

- `/`:
  - websocket upgrade for relay clients
  - normal GET serves relay page or relay info behavior
- `/ws`:
  - optional explicit websocket alias to relay
- `/api/*`:
  - `nexus-control`
- `/v1/*`:
  - `nexus-control`

That preserves the current product contract while replacing the relay engine with a real one.

## Recommended Hosting Model

### Best option: VM or stateful host for the relay

If OpenAgents wants `nostr-rs-relay` in its intended shape, the best deployment target is a stateful host:

- Compute Engine VM
- or a stateful Kubernetes/GKE deployment
- with a real persistent disk
- and a reverse proxy such as Nginx, HAProxy, or Caddy

Why this is the best option:

- SQLite works naturally on local persistent disk
- long-lived websocket relay behavior fits a stateful host better than scale-to-zero HTTP infrastructure
- relay persistence and operator maintenance are simpler
- the upstream docs are already written for this shape

### Acceptable but weaker option: Cloud Run plus Postgres

If OpenAgents insists on keeping the relay on Cloud Run, then:

- do **not** use SQLite-backed local storage there
- use Postgres-backed persistence
- accept that this is a more complex and less proven path for this codebase
- keep a front proxy or edge service because `nostr-rs-relay` itself will not replace the `/api/*` proxy layer

This is viable only if the team explicitly chooses the extra complexity.

## What To Do With `apps/nexus-relay`

`apps/nexus-relay` should not remain the production public relay.

It should become one of:

1. **Local/dev harness**
   - for desktop development
   - integration tests
   - managed-group experiments

2. **Nexus edge/gateway**
   - if you still want a tiny OpenAgents-specific edge that proxies `/api/*`
   - but then it should stop pretending to be the real durable relay

If kept, I would rename it conceptually to something like:

- `nexus-edge`
- or `nexus-gateway`

That would be more truthful than calling it the production relay.

## OpenAgents-Specific Compatibility Questions

There is one real question before replacement:

### Managed groups

The current in-repo relay has custom managed-group behavior in `managed_groups.rs`.

That is not the same thing as saying OpenAgents needs this for the current public Nexus relay.

Questions to answer before replacement:

1. Are those managed-group semantics required for the current marketplace path?
2. Or are they future chat/group features that do not need to live on the public MVP relay now?

My read from the current MVP shape is:

- they are **not** the critical requirement for the hosted Nexus path today
- the critical requirement today is a durable public relay for marketplace traffic plus the control APIs

If managed groups do matter later, port them deliberately:

- either as relay extensions,
- or as policy/admission hooks,
- or as a separate specialized service.

They should not be the reason to keep an in-memory public relay in production.

## Recommended Migration Sequence

1. **Stop treating `apps/nexus-relay` as the long-term production relay.**
   - It can remain live temporarily as the current edge.

2. **Stand up `nostr-rs-relay` as the real relay backend.**
   - preferred: stateful host with persistent disk
   - fallback: Cloud Run with Postgres

3. **Put an explicit edge in front of both relay and control.**
   - route `/api/*` and `/v1/*` to `nexus-control`
   - route websocket and relay root to `nostr-rs-relay`

4. **Move `nexus.openagents.com` to that edge.**

5. **Downgrade `apps/nexus-relay` to dev/test or gateway-only status.**

6. **Add in-repo deploy assets for the real relay topology.**
   - docker image
   - reverse proxy config
   - runbook
   - backup/restore/maintenance docs

## Bottom Line

OpenAgents now has a live Nexus host on current repo infrastructure.

That solves the "old backroom deployment" problem.

It does **not** solve the "real production relay" problem.

The current `apps/nexus-relay` is still:

- in-memory
- instance-local
- lightweight
- and better understood as a Nexus edge harness than as a durable Nostr relay

If OpenAgents wants a real production Nexus relay, the setup should change to:

> **`nostr-rs-relay` (or equivalent durable relay) for websocket/Nostr duty, `nexus-control` for OpenAgents authority APIs, and a reverse proxy/edge in front of both on `nexus.openagents.com`.**

That is the clean setup.
