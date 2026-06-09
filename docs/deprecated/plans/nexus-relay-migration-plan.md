# Nexus Relay Migration Plan

Status: proposed
Date: 2026-03-06

## Goal

Replace the current in-repo in-memory Nexus relay with a durable, production-grade relay implementation while keeping:

- one public Nexus surface at `nexus.openagents.com`
- one Rust Nexus service that can accept both websocket relay traffic and HTTP API traffic
- OpenAgents-owned product behavior and branding

The outcome should be:

> Nexus remains our relay product, but the relay engine stops being an in-memory MVP harness.

## Recommendation

Do not deploy upstream `nostr-rs-relay` untouched as the long-term answer.

Do not keep extending the current `apps/nexus-relay` into a production relay.

Instead:

1. fork or absorb `nostr-rs-relay` into this repo as the durable relay engine
2. make Nexus a single Rust service that exposes both websocket relay routes and HTTP authority/API routes
3. keep OpenAgents-specific behavior as thin Nexus-owned extensions around that engine
4. fold the current `nexus-control` responsibilities into that owned Nexus service over time
5. keep one public host: `nexus.openagents.com`

This preserves a single Nexus product, host, and service while avoiding a ground-up relay rewrite.

## Why This Is The Right Move

The current `apps/nexus-relay` is an MVP gateway with relay behavior, not a real production relay:

- event storage is process memory only
- multi-instance deployment fragments subscriptions and history
- restarts lose history
- there is no durable store
- there is no NIP-11 relay info document
- there is no operator-grade maintenance story
- it is currently doing edge-proxy work and relay work in one small binary

`nostr-rs-relay` already has the hard boring parts we need:

- durable event repository
- SQLite support now
- Postgres path if we choose it
- NIP-11
- NIP-42
- broader protocol coverage
- rate limits
- metrics
- maintenance docs
- relay-page and operator config surfaces

The correct trade is to absorb a real relay engine and make it ours, not to copy random pieces into the current harness.

## Non-Goals

This migration should not:

- turn Nexus into two public products
- rebuild relay persistence/query logic from scratch
- block Autopilot MVP progress on full five-market functionality
- require a complete protocol-feature rewrite before cutover

## Target Architecture

The desired production shape is:

```text
                        nexus.openagents.com
                                |
                      Nexus Rust Service
       durable relay engine + OpenAgents HTTP authority/API
       websocket + NIP-11 + metrics + /api/* + /v1/*
                                |
                                |
                durable event store + authority state
                  sqlite first or postgres later
```

Publicly, this is still one Nexus.

Internally, it is still two roles:

- relay engine
- authority/API

That split remains desirable as a code boundary even if it is not a separate deployed service boundary. Relay persistence and Nostr fanout are not the same job as OpenAgents authority mutations.

## Repo Direction

Preferred repo target:

```text
apps/
  nexus-relay/

crates/
  nexus-relay-core/         # optional extraction later
  nexus-relay-config/       # optional extraction later
```

Short term, keep the new Nexus service as one app if that lands fastest.

Do not start by over-modularizing it. First land a real durable relay in-repo. Extract crates only after the relay is stable.

Do not restore archived backroom Nexus relay code wholesale. The backroom snapshot may still be useful for reference, but the migration target should be the durable relay engine from `nostr-rs-relay` plus thin Nexus-owned extensions in the current repo.

The current `apps/nexus-control` should be treated as transitional. Its routes and responsibilities can be migrated into the owned Nexus service once the durable relay core is in place.

## What To Import From `nostr-rs-relay`

Source reviewed: `/Users/christopherdavid/code/nostr-rs-relay/src/*`

The migration should absorb these areas substantially intact at first:

### Core server and websocket handling

- `src/server.rs`
- `src/conn.rs`
- `src/close.rs`
- `src/notice.rs`
- `src/subscription.rs`

Why:

- proper websocket handling
- relay info responses
- subscription lifecycle
- close/notice semantics
- metrics plumbing

### Event validation and protocol semantics

- `src/event.rs`
- `src/delegation.rs`
- `src/error.rs`

Why:

- this is where event admission and protocol rules already live
- reimplementing this in the current in-memory relay would be wasted work

### Durable repository layer

- `src/repo/mod.rs`
- `src/repo/sqlite.rs`
- `src/repo/sqlite_migration.rs`
- optionally later: `src/repo/postgres.rs` and `src/repo/postgres_migration.rs`
- `src/db.rs`

Why:

- this is the real heart of the migration
- Nexus must stop storing events in a `Vec<Event>`

### Relay info / operator surfaces

- `src/info.rs`
- relevant config sections from `src/config.rs`
- `/metrics` handling from `src/server.rs`

Why:

- we need real relay introspection and operator posture

### Runtime/config bootstrap

- `src/config.rs`
- `src/cli.rs`
- `src/lib.rs`
- `src/main.rs`

Why:

- current Nexus relay config is too thin for a real relay
- we need proper relay config, not just listen addr + control URL

## What To Defer Or Leave Out Initially

These parts should not block the first cutover:

- `src/payment/*`
- pay-to-relay
- `src/nip05.rs` enforcement if not needed immediately
- `src/nauthz.rs` external admission hooks
- verified-user policy modes beyond what we actively want
- postgres support, unless we choose postgres for first deploy
- extra binaries such as `src/bin/bulkloader.rs`

These can be reintroduced later behind explicit OpenAgents decisions.

## What To Keep From Current `apps/nexus-relay`

The current in-repo relay still contains Nexus-specific behavior worth preserving, but not as the persistence/query engine:

### Keep conceptually

- the Nexus homepage / landing copy
- the existing `/api/*` and `/v1/*` HTTP surface
- OpenAgents relay identity wiring
- managed-group requirements if they are still product-relevant
- any OpenAgents-specific auth or routing affordances

### Likely keep as thin Nexus-owned modules

- `apps/nexus-relay/src/managed_groups.rs`
- Nexus HTML/branding layer
- HTTP route wiring for authority endpoints

### Do not keep as the core relay engine

- `RelayStore`
- in-memory `events: Vec<Event>`
- in-memory client registry as the authoritative system
- ad hoc filter/query logic in `apps/nexus-relay/src/lib.rs`

Those parts should be retired, not upgraded.

## What To Delete Or Retire

After the durable relay lands, remove or substantially replace:

- in-memory event storage in `apps/nexus-relay/src/lib.rs`
- the current custom replay/filter loop
- current direct event fanout as the authoritative relay state model
- any code that assumes Cloud Run stateless instances can act like a single durable relay

If a temporary compatibility layer remains, it should be clearly marked transitional and not treated as the long-term relay core.

## Product Boundary

After migration, the ownership split should be internal to the Nexus service:

### Relay module

Owns:

- Nostr relay protocol handling
- websocket ingress
- durable event persistence
- relay info doc
- metrics
- OpenAgents-specific relay extensions such as managed groups

Must not own:

- kernel authority mutations
- wallet/session authority
- starter-demand business logic
- product workflows exposed through `/api/*` and `/v1/*`

### Authority/API module

Owns:

- desktop session bootstrapping
- sync token minting
- starter-demand APIs
- kernel authority APIs
- receipt/snapshot authority

Must not own:

- durable relay query/persistence pipeline
- websocket relay semantics

This is a module boundary, not necessarily a service boundary.

## Deployment Recommendation

### Preferred

Deploy one durable Nexus service on stateful infra, not serverless stateless infra.

Best first production shape:

- one `nexus-relay` service on a VM or stateful container host
- SQLite-backed durable storage on attached disk
- websocket relay routes and HTTP API routes served by the same Rust process

Why:

- a relay is fundamentally long-lived websocket + durable state infra
- SQLite is a clean first production store if the host is stateful
- one process keeps infra simple and matches the product intuition of "one Nexus"
- this avoids forcing the relay into Cloud Run scaling semantics it does not naturally fit

### Acceptable alternative

If we want to keep the Nexus service in Cloud Run, then switch it to Postgres and make that explicit from day one.

In that shape:

- the single Nexus service uses Postgres, not SQLite
- Cloud Run instances remain stateless
- one public Nexus host still fronts both websocket and HTTP entrypoints

What not to do:

- Cloud Run + in-memory relay
- Cloud Run + SQLite on ephemeral local filesystem

### Optional later hardening

If operations later demand it, we can still add:

- a reverse proxy or edge in front of Nexus
- a deployment split between relay and authority roles

That should be treated as later hardening, not as the default target architecture.

## Migration Phases

### Phase 0: Freeze The Current Harness

Goal:

Stop treating the current relay as the end state.

Actions:

- document `apps/nexus-relay` as transitional
- avoid adding new persistence/query features to the in-memory store
- keep only minimal fixes required for current uptime

Exit criteria:

- team alignment that the current relay is an edge/harness, not the long-term relay

### Phase 1: Fork And Land A Real Relay In-Repo

Goal:

Bring the durable relay engine into this repo under Nexus ownership.

Actions:

- vendor or fork `nostr-rs-relay` code into a new `apps/nexus-relay` baseline or adjacent import area
- preserve license and attribution
- compile it in this workspace
- keep OpenAgents naming and config wrappers
- keep the target shape as one Rust Nexus service

Recommended implementation approach:

1. import upstream relay code mostly intact
2. get it compiling and running in this workspace
3. only then begin Nexus-specific edits
4. do not split service boundaries yet

Exit criteria:

- workspace builds with the durable relay engine in-repo
- local relay run persists events across restart
- root responds to websocket and NIP-11 correctly

### Phase 2: Reapply Nexus-Specific Surface Area

Goal:

Make the durable relay look and behave like Nexus, not a generic upstream relay.

Actions:

- restore Nexus landing page and branding
- restore `nexus.openagents.com` operator identity
- add OpenAgents relay defaults
- move or inline current `nexus-control` HTTP routes into the owned Nexus service
- reintroduce managed-group behavior if still needed

Recommended rule:

Keep custom Nexus behavior at the HTTP route layer and event-admission boundaries. Do not fork deep repository logic unless necessary.

Exit criteria:

- local Nexus host works as one branded surface
- relay remains durable
- OpenAgents-specific extensions are thin and isolated
- websocket and HTTP authority routes are both served by one Rust service

### Phase 3: Choose Final Storage And Hosting

Goal:

Pick the durable production deployment shape deliberately.

Decision point:

- VM + SQLite
- or Cloud Run + Postgres

Recommendation:

- choose VM + SQLite unless there is a strong reason to force serverless

Actions:

- provision production storage
- add operator config and backup plan
- enable metrics scraping
- validate restart, retention, and restore behavior

Exit criteria:

- relay survives restart without history loss
- operator backup/restore path is documented and tested
- metrics and health checks are live

### Phase 4: Controlled Cutover

Goal:

Move `nexus.openagents.com` from transitional relay behavior to the durable relay.

Actions:

- deploy durable relay behind staging host first
- replay production-like websocket and event flow tests
- verify Autopilot desktop compatibility
- verify authority/API routes inside Nexus
- cut production traffic
- monitor connections, writes, query latency, and persistence

Exit criteria:

- new relay is live behind `nexus.openagents.com`
- events persist across restart
- clients reconnect cleanly
- relay info and metrics surfaces are healthy

### Phase 5: Decommission Legacy Harness Paths

Goal:

Remove the old in-memory relay core so it stops confusing architecture and ops.

Actions:

- delete retired `RelayStore` code
- remove old in-memory event query/fanout logic
- keep only explicit Nexus-owned extensions that still belong
- update docs and runbooks

Exit criteria:

- no production-critical code path depends on the old in-memory relay model

## Migration Map: Current To Target

### Current

`apps/nexus-relay`

- websocket relay
- landing page
- `/api/*` and `/v1/*` passthrough/proxy
- in-memory event store
- simple fanout
- custom group logic

### Target

`apps/nexus-relay`

- durable relay engine from `nostr-rs-relay`
- Nexus landing page
- NIP-11
- NIP-42
- metrics
- Nexus authority/API routes
- optional Nexus-managed group extensions
- optional proxy/edge logic later only

## Exact First Engineering Tasks

1. create a new migration branch for relay import work
2. import `nostr-rs-relay` source into a dedicated in-repo area
3. make it compile in this workspace without product customizations
4. stand it up locally with persistent SQLite
5. verify:
   - publish
   - replay
   - restart persistence
   - NIP-11
   - NIP-42
   - metrics
6. inline current `nexus-control` routes into the Nexus service
7. layer back Nexus branding and host behavior
8. decide final production storage/hosting
9. cut traffic
10. remove old harness core

## Proposed GitHub Issue Sequence

These issues should be executed in order. Later issues assume the earlier ones are already merged.

### Issue 1: Import `nostr-rs-relay` into the Nexus workspace

Summary:

- vendor or fork the upstream relay source into this repo
- preserve licensing and attribution
- make it compile in the OpenAgents workspace
- do not apply Nexus-specific behavioral changes yet

Why first:

- everything else depends on having a real durable relay engine in-repo

### Issue 2: Replace the in-memory relay core with the durable repository path

Summary:

- remove `RelayStore` as the authoritative event store
- wire the imported relay to SQLite-backed persistence first
- prove publish, replay, and restart persistence locally

Why second:

- this is the actual migration away from the MVP harness

### Issue 3: Expose the canonical relay operator surfaces

Summary:

- enable NIP-11 relay info
- enable NIP-42 auth behavior
- expose `/metrics`
- add production-worthy relay config loading instead of the current minimal env-only shape

Why third:

- once the relay is durable, it needs real protocol and operator posture before Nexus-specific UI work

### Issue 4: Convert Nexus into one Rust service with both websocket and HTTP routes

Summary:

- inline current `nexus-control` HTTP routes into the owned Nexus service
- keep relay and authority as internal module boundaries
- preserve existing `/api/*` and `/v1/*` semantics

Why fourth:

- this lands the desired single-service architecture before branding and cutover work

### Issue 5: Reapply Nexus branding and product defaults on top of the new core

Summary:

- restore the Nexus landing page and operator identity
- keep `nexus.openagents.com` behavior product-consistent
- retain the correct default relay URL and host assumptions for desktop clients

Why fifth:

- once the service shape is right, the user-visible surface can be made Nexus-native again

### Issue 6: Port or redesign managed-group behavior on the durable relay

Summary:

- evaluate `apps/nexus-relay/src/managed_groups.rs` against the imported relay
- either port it cleanly as a thin extension or explicitly defer it
- avoid forking deep repository/query internals just to preserve current MVP group behavior

Why sixth:

- managed-group behavior is important, but it should sit on top of the durable relay, not distort the migration order

Current resolution:

- defer managed-group enforcement during the durable relay cutover
- keep `apps/nexus-relay/src/managed_groups.rs` as legacy/reference code only
- do not wire it into the durable upstream by patching deep repository or query internals
- revisit only after we have explicit thin hooks for:
  - authenticated group read filtering
  - authenticated group write admission
  - relay-owned group snapshot emission

Near-term product truth:

- managed groups are not on the current production Nexus path
- the durable relay should advertise this honestly in operator/runtime surfaces instead of implying support

### Issue 7: Add production deployment config for the single Nexus service

Summary:

- choose the first real hosting path
- preferred: VM or stateful container host + SQLite
- alternative: Cloud Run + Postgres
- add runbooks/config/scripts for the chosen path

Why seventh:

- after the binary is real, the deployment model has to stop pretending stateless infra is enough

### Issue 8: Stand up a staging Nexus and run protocol + desktop compatibility validation

Summary:

- deploy the durable single-service Nexus behind a staging host
- test websocket relay flow, replay, restart persistence, NIP-11, NIP-42, and desktop connectivity
- verify authority/API routes still work end-to-end

Why eighth:

- this is the last safe place to find integration breakage before traffic moves

### Issue 9: Cut over `nexus.openagents.com` to the durable Nexus service

Summary:

- deploy the new Nexus service to the production hostname
- monitor live connections, event writes, replay, metrics, and API traffic
- verify persistence through restart or rollout

Why ninth:

- production cutover should happen only after the staging validation issue is complete

### Issue 10: Remove the legacy in-memory relay paths and transitional compatibility code

Summary:

- delete old in-memory event storage and custom replay logic
- remove obsolete proxy-only harness assumptions
- update docs and runbooks to match the new architecture

Why tenth:

- cleanup belongs after the cutover is proven, not before

### Issue 11: Harden the production relay with policy, limits, and maintenance operations

Summary:

- finalize rate limits and abuse controls
- validate backup/restore and retention procedures
- document operator maintenance for the chosen storage backend

Why eleventh:

- this closes the gap between “working production relay” and “operable production relay”

### Issue 12: Optional later split of relay and authority into separate services

Summary:

- only if operational pressure justifies it, split the internal relay and authority modules into separate deployed services
- keep one public Nexus host even if deployment splits later

Why last:

- this is hardening and scale work, not required for full integration of the migration itself

Decision as of March 6, 2026:

- defer the split
- keep relay and authority in one deployed Nexus service
- keep the split only as an internal code boundary and future operational option

Reason:

- the current VM + durable relay + in-process authority shape is working in production
- splitting now would add infra complexity without solving a current operational bottleneck
- the user-facing requirement is one Nexus host, and the current single-service deployment already satisfies it

## Acceptance Criteria

This migration is successful when all of the following are true:

- `nexus.openagents.com` still presents as one Nexus host
- websocket and HTTP authority traffic are both served by one Nexus Rust service
- relay history survives restart
- relay state is no longer instance-local memory
- multi-instance or restart behavior is no longer lossy/fractured
- Autopilot desktop connects without regressions
- the authority/API module remains the owner of `/api/*` and `/v1/*`
- the relay exposes real operator surfaces such as NIP-11 and metrics
- the current in-memory relay core is retired

## Open Questions

1. Do we rename the single service from `nexus-relay` to `nexus`, or keep the existing app name and just expand its role?
2. Is managed-group behavior still a product requirement for near-term Nexus, or should it be deferred until after durable relay cutover?
3. Do we want SQLite on stateful infra first, or Postgres to preserve Cloud Run symmetry?
4. Do we want to preserve any upstream optional features such as NIP-05 verification or external admission hooks?
5. If we later split infra, which module boundaries should become separate services first?

## Bottom Line

Nexus should remain the product and host.

The Nexus service underneath `nexus.openagents.com` should become a single Rust service built around a forked, OpenAgents-owned durable relay based on `nostr-rs-relay`, not the current in-memory harness and not an untouched upstream deployment.

That gives us one Nexus surface, one real relay, one HTTP authority surface, and a path to production without pretending the current relay is already there.
