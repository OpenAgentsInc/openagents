# SpacetimeDB Architecture and OpenAgents/Autopilot Adoption Audit

Status: draft-audit snapshot
Date: 2026-02-25
Owner: repo audit (Codex)

## Question this answers

What SpacetimeDB is, how it is structured internally, and how OpenAgents/Autopilot can adopt it without violating current architecture and invariant gates.

## Method

This audit is docs-first and code-first across both repos.

OpenAgents authority sources reviewed:

1. `docs/core/ARCHITECTURE.md`
2. `docs/core/PROJECT_OVERVIEW.md`
3. `docs/core/ROADMAP.md`
4. `docs/adr/INDEX.md`
5. `docs/plans/rust-migration-invariant-gates.md`
6. ADRs `0001`, `0002`, `0003`, `0004`, `0005`, `0006`, `0008`

SpacetimeDB sources reviewed:

1. Docs under `/Users/christopherdavid/code/SpacetimeDB/docs/docs/`
2. Workspace structure via `/Users/christopherdavid/code/SpacetimeDB/Cargo.toml`
3. Runtime/control and protocol code in:
   - `/Users/christopherdavid/code/SpacetimeDB/crates/standalone`
   - `/Users/christopherdavid/code/SpacetimeDB/crates/client-api`
   - `/Users/christopherdavid/code/SpacetimeDB/crates/core`
   - `/Users/christopherdavid/code/SpacetimeDB/crates/subscription`
   - `/Users/christopherdavid/code/SpacetimeDB/crates/durability`
   - `/Users/christopherdavid/code/SpacetimeDB/crates/commitlog`

## Executive summary

SpacetimeDB is a stateful application runtime embedded into a database, not just a storage engine. It combines:

1. In-database module logic (`reducers`, `procedures`, `views`).
2. Realtime client replication via SQL subscriptions over websocket.
3. Durable commitlog-backed state with snapshots.

For OpenAgents/Autopilot, SpacetimeDB is a good fit as a bounded realtime projection/collaboration plane, but it is not a good immediate replacement for OpenAgents authority lanes (control/runtime HTTP authority, proto-governed contracts, Khala replay semantics).

Recommended posture:

1. Use SpacetimeDB for additive, non-authority, high-fanout realtime domains.
2. Do not place canonical auth, custody, treasury, or replay authority inside SpacetimeDB under current ADR/invariant constraints.
3. Adopt through a sidecar/projection model first, with strict boundary adapters.

## What SpacetimeDB is

At a system level, SpacetimeDB is a database + app-server runtime where module code executes next to data and pushes delta updates to clients.

Core conceptual model:

1. Tables are application state.
2. `reducers` are transactionally atomic mutation entrypoints.
3. `procedures` and `views` support additional callable/read patterns.
4. Clients subscribe to SQL queries and receive live updates as state changes.
5. Identity and token context are available to module code.

This differs from a classic OpenAgents service pattern (separate app service + DB + bespoke pub/sub) by collapsing execution and replication into one runtime substrate.

## Internal structure (SpacetimeDB repo)

## Workspace shape

The workspace is split into clear layers:

1. Host/runtime core (`crates/core`, `crates/vm`, `crates/schema`, `crates/table`, `crates/expr`).
2. API surface (`crates/client-api`, websocket protocol crates, HTTP routes).
3. Local orchestration (`crates/standalone`, control-db management).
4. Replication/subscription engine (`crates/subscription`).
5. Durability and logs (`crates/durability`, `crates/commitlog`, snapshot handling).
6. Tooling/dev UX (`crates/cli`, bindings/codegen crates).

This layering is coherent: API ingress -> module host/reducer execution -> commit/durability -> subscription fanout.

## Runtime/control path

Observed from `standalone`, `client-api`, and `core`:

1. `standalone` runs the local server process, tracks configured databases/replicas, and launches module hosts.
2. `client-api` exposes DB endpoints, reducer/procedure invocation paths, and websocket subscribe endpoints.
3. `core::host::HostController` manages module lifecycle, updates, and host coordination.
4. `core::host::ModuleHost` executes reducer/procedure/view logic and drives broadcast to subscribers.
5. `core::host::scheduler` executes scheduled table-driven tasks.

## Data/durability model

SpacetimeDB persists through commitlog + snapshots with in-memory serving characteristics.

Practical consequences:

1. Strong live-update responsiveness.
2. Deterministic mutation boundaries around reducer transactions.
3. Recovery and replay grounded in commit history.

For OpenAgents, this is useful for projection/read optimization, but commitlog semantics are not a drop-in match for existing `(topic, seq)` Khala replay contracts.

## Subscription engine details

`crates/subscription` compiles and executes SQL subscription plans with constraints.

Important properties:

1. Subscription semantics are first-class, not bolted on.
2. Query shape/indexing matter for scalability and accepted query forms.
3. Event-table use has specialized behavior and limitations vs normal tables.

This is a strength for collaborative realtime UIs where many clients need synchronized state projections.

## Auth/identity model

SpacetimeDB supports identity and token-based auth, including OIDC-based flows and claims propagation into reducer context.

Important distinction for OpenAgents:

1. Spacetime identity can authenticate usage of Spacetime modules.
2. It should not supersede OpenAgents canonical session/custody/economic authority under current architecture.

## Protocol and API surfaces

SpacetimeDB exposes both HTTP and websocket surfaces and has protocol evolution (`v1`/`v2`) with stronger delivery semantics in newer paths.

For integration design, treat websocket replication as a specialized data sync lane, not a universal authority bus.

## Migrations/operations

Schema evolution has both automatic and incremental migration patterns, with explicit docs guidance for breaking-change handling.

Operationally:

1. Managed cloud exists.
2. Self-hosting is supported.
3. PG-wire exists but is partial and not a full replacement for native semantics.

## Fit against OpenAgents invariants

Under OpenAgents canonical gates, SpacetimeDB must currently be bounded.

Constraint mapping:

1. `INV-01`/ADR-0002 (proto-first contracts):
   - Spacetime module schema/codegen is not the authority for OpenAgents cross-service contracts.
   - Any integration must translate to/from existing proto contracts at boundaries.

2. `INV-02` (HTTP-only authority mutations for control):
   - Control-plane canonical writes stay in existing HTTP authority lanes.
   - Spacetime reducers cannot become control authority without an ADR-level architecture change.

3. `INV-03`/ADR-0003 (Khala WS authority transport):
   - Khala remains authority live-transport lane.
   - Spacetime websocket streams can be used for non-authority projection lanes.

4. `INV-04`/`INV-05`/`INV-06` (authority ownership boundaries):
   - Control/runtime authority boundaries stay explicit.
   - Spacetime should not blur those by introducing a second canonical authority plane.

5. `INV-07` (deterministic replay by `(topic, seq)`):
   - Existing replay/idempotency contracts remain source of truth.
   - If Spacetime consumes or emits domain events, adapters must preserve deterministic ordering semantics required by OpenAgents.

## Recommended use for OpenAgents/Autopilot

## Where SpacetimeDB should be used

1. Realtime collaborative projection lanes:
   - Presence, cursors, typing, ephemeral shared agent context, live dashboard state.

2. High-fanout read models:
   - Derived state that many clients subscribe to, where low-latency push matters more than authority ownership.

3. Desktop/local-first sync adjunct:
   - Optional local Spacetime instance for fast local collaborative state and offline-friendly projection caches.

4. Marketplace observability overlays:
   - Non-authority market heatmaps, provider health views, transient matching signals.

## Where SpacetimeDB should not be used (current state)

1. Canonical auth/session authority.
2. Canonical treasury/custody/liquidity/credit state (Hydra/Spark lanes).
3. Canonical runtime/control mutation authority.
4. Replacement of proto-governed boundary contracts.
5. Replacement of Khala replay/idempotency guarantees.

## Recommended integration architecture

Adopt an additive projection architecture:

1. Existing OpenAgents authority services remain source of truth.
2. Authority events are projected into Spacetime via explicit adapter(s).
3. Clients subscribe to Spacetime for low-latency collaborative/read views.
4. Any user intent that mutates authority state still goes through canonical OpenAgents APIs.

Boundary rules:

1. No direct coupling from critical authority write path to Spacetime availability.
2. Projection lag is acceptable; authority correctness is not delegated.
3. Replay adapters must encode source `(topic, seq)` and preserve idempotent apply semantics.

## Adoption plan (phased)

## Phase 0: technical spike (bounded)

1. Build one projection-only domain (for example, desktop collaborative presence lane).
2. Implement adapter from OpenAgents authority event stream to Spacetime table reducers.
3. Validate latency, fanout, failure/restart behavior, and replay reconciliation.

Exit criteria:

1. No authority regression.
2. Deterministic rebuild from OpenAgents source events.
3. Measurable UX gain (latency/fanout) over current path.

## Phase 1: production pilot

1. Scope to one non-financial, non-custody, non-auth lane.
2. Add operational SLOs, backup/restore procedure, and observability for projection freshness.
3. Keep hard rollback path to existing OpenAgents-only read path.

Exit criteria:

1. Stable operations and acceptable pager load.
2. Proven containment of failure domain.
3. No drift in authority state semantics.

## Phase 2: selective expansion

1. Expand only to additional projection/collaboration domains that benefit from native subscriptions.
2. Maintain explicit contract adapters and schema governance.
3. Revisit authority usage only through new ADRs and invariant updates.

## Risks and mitigations

1. Risk: accidental second authority plane.
   - Mitigation: enforce adapter pattern and write-path guardrails; document forbidden domains.

2. Risk: schema/protocol drift between proto contracts and Spacetime schemas.
   - Mitigation: generated adapters + contract conformance tests in CI.

3. Risk: operational complexity (new datastore/runtime to run).
   - Mitigation: start with narrow pilot, clear oncall boundaries, explicit rollback.

4. Risk: replay mismatch with existing `(topic, seq)` semantics.
   - Mitigation: include source sequence metadata in projection tables and idempotent projector reducers.

5. Risk: team bandwidth split during Rust migration priorities.
   - Mitigation: gate adoption to bounded ROI lane and keep roadmap-critical migrations first.

## Concrete recommendation

OpenAgents should use SpacetimeDB now only as a bounded realtime projection layer for Autopilot/OpenAgents collaborative UX and high-fanout read models.

OpenAgents should not currently use SpacetimeDB as canonical authority for control/runtime, auth/session, or economic/custody domains.

If a future architecture wants deeper authority usage, that should be proposed as a dedicated ADR set with explicit updates to invariant gates (`INV-01` through `INV-07`) and replay semantics.
