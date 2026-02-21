# OpenAgents Rust-Only Architecture Roadmap

Status: Active execution backlog (GitHub-tracked)
Last updated: 2026-02-21
Owner: Architecture / Runtime / Client platform

## Purpose

This document is the buildout and sequencing companion to `docs/ARCHITECTURE-RUST.md`.

It moves implementation steps out of the architecture definition and expands them into a concrete, ordered GitHub issue backlog with proposed issue names and descriptions.

## Scope and Rules

1. This roadmap defines execution order, dependencies, and migration gates.
2. `docs/ARCHITECTURE-RUST.md` remains the source of architectural truth.
3. `proto/` remains universal schema authority for cross-process contracts.
4. Khala remains WS-only for live sync in endstate.
5. Rivet is harvested for subsystem patterns, not adopted wholesale as platform authority.
6. WorkOS remains the canonical identity provider and authentication source of truth.
7. Runtime path naming is canonicalized to `apps/runtime` (deprecated `apps/openagents-runtime` references are not allowed).

## Execution Control Plane

- Board: `https://github.com/orgs/OpenAgentsInc/projects/12`
- Usage, labels, swimlanes, and owner map:
  `docs/plans/active/rust-migration-execution-control-plane.md`

## GitHub Tracking Notes

`OA-RUST-001` through `OA-RUST-107` are tracked as GitHub issues.

For new roadmap additions, use `gh issue create` and apply required labels from the
execution control-plane conventions.

Suggested command pattern:

```bash
gh issue create \
  --title "<issue title>" \
  --body "<issue description + acceptance criteria + dependencies>" \
  --label "rust-migration"
```

## Proposed Backlog (Ordered)

## Phase 0: Program, Governance, and Baselines

### OA-RUST-001 — [Rust Endstate] Create migration board, labels, and ownership map
Description: Create a dedicated migration board with per-surface ownership (`openagents.com`, runtime, Khala, desktop, iOS, infra) and standard labels so every migration issue can be tracked consistently.

### OA-RUST-002 — [Rust Endstate] Baseline inventory of legacy apps, packages, and runtime dependencies
Description: Produce a complete inventory of legacy Laravel/React, `apps/mobile`, `apps/desktop`, and `apps/inbox-autopilot` dependencies to prevent hidden blockers during deletion/cutover.
Inventory artifact: `docs/plans/active/rust-migration-legacy-dependency-inventory.md`

### OA-RUST-003 — [Rust Endstate] Lock migration invariants from architecture doc
Description: Convert architecture invariants (proto-first, dual authority planes, Khala projection-only, WS-only sync) into explicit checklists and required gates for every implementation PR.
Invariant gates: `docs/plans/active/rust-migration-invariant-gates.md`

### OA-RUST-004 — [Rust Endstate] Archive existing ADR set
Description: Move current `docs/adr/` records to an archived path with index updates so the new Rust-era ADR series can begin cleanly.
Archive path: `docs/plans/archived/adr-legacy-2026-02-21/`

### OA-RUST-005 — [Rust Endstate] Reinitialize ADR index and numbering for Rust era
Description: Create new ADR index, numbering conventions, and templates focused on Rust-only service/client architecture and migration decisions.
Rust-era ADR process docs: `docs/adr/INDEX.md`, `docs/adr/README.md`, `docs/adr/TEMPLATE.md`

### OA-RUST-006 — [Rust Endstate] Define migration KPIs and reporting dashboard
Description: Define measurable migration KPIs (route parity, runtime parity, WS reliability, latency, regression count) and publish a recurring status artifact.
KPI artifacts: `docs/plans/active/rust-migration-kpi-dashboard.md`, `docs/plans/active/rust-migration-kpi-report-template.md`, `docs/plans/active/rust-migration-reports/`

## Phase 1: Proto and Contract Foundation

### OA-RUST-007 — [Proto] Publish Rust-era package map for control/runtime/sync domains
Description: Finalize proto package ownership and namespace boundaries for `control`, `runtime`, `sync/khala`, and Codex surfaces so contract growth is structured.
Package map artifact: `proto/PACKAGE_MAP.md`

### OA-RUST-008 — [Proto] Add Khala frame envelope contract
Description: Define/lock `KhalaFrame` envelope fields (`topic`, `seq`, `kind`, `payload_bytes`, `schema_version`) and publish compatibility rules.

### OA-RUST-009 — [Proto] Finalize runtime orchestration contracts
Description: Define run lifecycle, worker lifecycle, and durable receipt/replay proto messages used by runtime authority and clients.
Contract artifacts: `proto/openagents/runtime/v1/orchestration.proto`, `docs/protocol/fixtures/runtime-orchestration-v1.json`

### OA-RUST-010 — [Proto] Finalize control-plane auth/session/scope contracts
Description: Define control-plane messages for auth/session, org membership, WorkOS identity mapping, and sync token scope derivation used by web, desktop, and iOS.
Contract artifacts: `proto/openagents/control/v1/auth.proto`, `docs/protocol/fixtures/control-auth-session-v1.json`, `docs/protocol/control-auth-session-v1.md`

### OA-RUST-011 — [Proto] Finalize Codex worker/event contracts for all surfaces
Description: Lock Codex worker event envelopes, turn status updates, and replay metadata contracts so web/desktop/iOS consume the same wire protocol.
Contract artifacts: `proto/openagents/codex/v1/events.proto`, `proto/openagents/codex/v1/workers.proto`, `proto/openagents/codex/v1/auth.proto`, `docs/protocol/fixtures/codex-worker-events-v1.json`, `docs/protocol/codex-worker-events-v1.md`

### OA-RUST-012 — [Proto] Stand up Rust codegen workspace crate(s)
Description: Create shared generated Rust crates and enforce deterministic generation output across services and clients.
Codegen artifacts: `crates/openagents-proto/`, `scripts/verify-rust-proto-crate.sh`

### OA-RUST-013 — [Proto] Enforce buf lint/breaking/generate gates in local CI lanes
Description: Wire proto compatibility gates into local CI so breaking schema changes cannot land without explicit versioned evolution.
Gate artifacts: `scripts/local-ci.sh`, `scripts/verify-proto-generate.sh`, `scripts/verify-rust-proto-crate.sh`, `docs/LOCAL_CI.md`

### OA-RUST-014 — [Proto] Add wire-to-domain conversion test harness
Description: Add tests validating `TryFrom`/`From` conversions between proto wire types and Rust domain models, including invalid payload behavior.
Harness artifacts: `crates/openagents-proto/tests/conversion_harness.rs`, `crates/openagents-proto/tests/fixtures/conversion-harness-v1.json`, `crates/openagents-proto/src/lib.rs`

## Phase 2: Rust Control Service (`apps/openagents.com/service`)

### OA-RUST-015 — [Web Service] Bootstrap Rust control service skeleton
Description: Create Rust service foundation for auth, control APIs, sync token minting, and static wasm asset hosting at `apps/openagents.com`.

### OA-RUST-016 — [Web Service] Implement auth/session API parity
Description: Port auth/session endpoints and token lifecycle semantics required by current clients while keeping WorkOS as identity/auth source of truth, preserving security and audit invariants.

### OA-RUST-017 — [Web Service] Implement org membership and policy API parity
Description: Port org membership and policy enforcement APIs currently required for runtime access and scoped actions.

### OA-RUST-018 — [Web Service] Implement sync token minting and scope derivation
Description: Implement secure sync token issuance with topic scope validation for Khala subscriptions across web/desktop/iOS.

### OA-RUST-019 — [Web Service] Implement static host for versioned wasm assets
Description: Serve hashed JS/WASM artifacts and static manifest from Rust control service with cache-control/versioning strategy.

### OA-RUST-020 — [Web Service] Add request correlation, audit logs, and structured observability
Description: Add structured logs, request correlation IDs, and audit events equivalent to current operational requirements.

### OA-RUST-021 — [Web Service] Add feature-flagged route split between legacy and Rust web shell
Description: Implement routing controls to gradually shift traffic route-by-route to Rust/WGPUI while preserving rollback.

### OA-RUST-022 — [Web Service] Canary deploy and rollback runbook for Rust control service
Description: Add canary rollout strategy, rollback procedure, and verification checklist for production cutovers.

## Phase 3: In-Process Rust/WGPUI Web App (`apps/openagents.com/web-shell`)

### OA-RUST-023 — [WGPUI Web] Create wasm web-shell bootstrap and runtime entrypoint
Description: Build the browser wasm entrypoint that mounts WGPUI and owns application lifecycle in-process.

### OA-RUST-024 — [WGPUI Web] Implement minimal JS host shim boundary
Description: Restrict JavaScript to host concerns (bootstrapping/platform API bridges) and enforce no product logic outside Rust.

### OA-RUST-025 — [WGPUI Web] Build shared UI core crate for cross-surface components
Description: Create shared WGPUI component/theme primitives used by web, desktop, and iOS to prevent UI divergence.

### OA-RUST-026 — [WGPUI Web] Implement shared app state crate (routes, reducers, command intent)
Description: Implement route graph, reducer/state machine, and command intents in Rust for deterministic behavior across surfaces.

### OA-RUST-027 — [WGPUI Web] Implement auth/session UX in Rust shell
Description: Build signin/session/reauth user flows in WGPUI using control-plane APIs from Rust clients.

### OA-RUST-028 — [WGPUI Web] Implement command bus and HTTP API client layer
Description: Build typed command dispatch and API adapters used by all migrated web routes.

### OA-RUST-029 — [WGPUI Web] Implement Khala websocket client with replay/resume
Description: Build browser-side Rust WS client for subscription, replay, and deterministic resume from cached watermarks.

### OA-RUST-030 — [WGPUI Web] Implement local watermark/state persistence
Description: Persist per-topic watermarks and required local state to support reconnect and stale cursor recovery UX.

### OA-RUST-031 — [WGPUI Web] Migrate Codex thread route as first production Rust route
Description: Move a single high-value route to Rust/WGPUI as the pilot migration and validate parity/performance.

### OA-RUST-032 — [WGPUI Web] Remove legacy React/Inertia implementation for migrated pilot route
Description: Delete the legacy implementation for the pilot route once parity checks and soak tests pass.

## Phase 4: Rust Runtime Authority (`apps/runtime`)

### OA-RUST-033 — [Runtime Rust] Bootstrap runtime service foundation
Description: Create Rust runtime service skeleton with core wiring for authority writes, orchestration, and projector pipelines.

### OA-RUST-034 — [Runtime Rust] Port worker registry and lifecycle management
Description: Implement worker registration, status transitions, heartbeat semantics, and ownership checks in Rust runtime.

### OA-RUST-035 — [Runtime Rust] Port run state machine and transition rules
Description: Implement run creation/transition logic with explicit deterministic state transition validation.

### OA-RUST-036 — [Runtime Rust] Implement durable runtime event log write path
Description: Implement append-only authoritative event writes with idempotency, ordering, and conflict handling.

### OA-RUST-037 — [Runtime Rust] Implement receipts and replay artifact generation
Description: Generate deterministic receipts/replay artifacts from runtime events for verification and downstream consumption.

### OA-RUST-038 — [Runtime Rust] Port projection pipeline and checkpoint model
Description: Port read-model projectors and checkpoints to Rust while preserving replay correctness and idempotency.

### OA-RUST-039 — [Runtime Rust] Implement shadow-mode dual-write verification harness
Description: Run Rust runtime in shadow mode against existing runtime flows and produce parity diff reports.

### OA-RUST-040 — [Runtime Rust] Cut runtime authority writes to Rust and freeze legacy writes
Description: Complete cutover so Rust runtime becomes authority write path in production with rollback controls.

## Phase 5: Khala WS-Only and Rivet-Pattern Harvest

### OA-RUST-041 — [Khala] Implement internal fanout seam (UniversalPubSub-inspired)
Description: Introduce a pluggable fanout abstraction in runtime/Khala (memory first, external broker optional later) without changing external contracts.

### OA-RUST-042 — [Khala] Implement gateway reconnect/resume lifecycle hardening (Guard-inspired)
Description: Harden websocket lifecycle behavior across deploy/restart/sleep events to reduce reconnect storms and client churn.

### OA-RUST-043 — [Khala] Implement strict stale cursor and replay bootstrap semantics
Description: Enforce consistent `stale_cursor` handling and deterministic replay bootstrap paths for all clients.

### OA-RUST-044 — [Khala] Add websocket auth scope, ACL, and ownership test matrix
Description: Add comprehensive authz tests for topic subscription permissions, scope mismatches, and multi-org isolation.

### OA-RUST-045 — [Khala] Remove SSE/poll fallback from web, desktop, and iOS clients
Description: Remove non-WS live sync lanes so all clients exclusively use Khala websocket transport.

### OA-RUST-046 — [Runtime/Khala] Add workflow history compatibility tests (Gasoline-inspired)
Description: Add replay/history compatibility checks that fail changes which would break deterministic workflow/history re-execution.

### OA-RUST-047 — [Khala] Add backpressure and herd-protection controls
Description: Add rate controls, jittered reconnect policy, and bounded buffers to protect service under reconnect spikes.

### OA-RUST-048 — [Khala] Execute WS-only production rollout gate
Description: Run staged rollout and verification that WS-only sync is stable for web, desktop, and iOS before finalizing policy.

## Phase 6: Desktop Consolidation (`apps/autopilot-desktop`)

### OA-RUST-049 — [Desktop] Fold inbox-autopilot domain logic into desktop Rust crates
Description: Move inbox domain logic from standalone app into desktop-owned shared Rust crates with clear module boundaries.

### OA-RUST-050 — [Desktop] Implement inbox panes in WGPUI desktop shell
Description: Build inbox UI surfaces as desktop panes in the existing WGPUI shell.

### OA-RUST-051 — [Desktop] Integrate Codex and Inbox on shared app state and routing
Description: Unify desktop Codex/Inbox flows under one state model, command bus, and sync client.

### OA-RUST-052 — [Desktop] Remove `apps/inbox-autopilot` root and references
Description: Delete standalone inbox app root and clean up repo references/docs/build scripts.

### OA-RUST-053 — [Desktop] Remove `apps/desktop` legacy root after capability audit
Description: Complete migration of remaining assets/capabilities, then delete the legacy desktop root.

## Phase 7: iOS and Shared Client Core

### OA-RUST-054 — [Client Core] Extract shared Rust client core for web/desktop/iOS
Description: Create shared Rust client crates for auth, command transport, Khala subscriptions, and message projection.

### OA-RUST-055 — [iOS] Migrate autopilot iOS to shared Rust client core
Description: Replace iOS-specific sync/business logic with shared Rust core while preserving native host integration.

### OA-RUST-056 — [iOS] Complete control-plane auth/session parity on iOS
Description: Ensure iOS auth/session/token refresh behavior matches web and desktop semantics.

### OA-RUST-057 — [iOS] Implement robust background/resume watermark persistence
Description: Ensure iOS resume behavior restores sync state correctly without duplicate/jumbled events.

### OA-RUST-058 — [iOS] Remove `apps/mobile` legacy root and references
Description: Delete legacy mobile app root and remove associated scripts/docs once iOS Rust stack is authoritative.

## Phase 8: Web Route Migration and Legacy Web Removal

### OA-RUST-059 — [WGPUI Web] Migrate account/settings/admin route set to Rust shell
Description: Port account and administrative route surfaces from Laravel/React to Rust/WGPUI equivalents.

### OA-RUST-060 — [WGPUI Web] Migrate billing/lightning operator route set to Rust shell
Description: Port billing and lightning operator surfaces to Rust/WGPUI while preserving API and policy behavior.

### OA-RUST-061 — [WGPUI Web] Migrate onboarding/auth entry routes to Rust shell
Description: Port remaining onboarding/auth routes to Rust shell and retire legacy frontend entrypoints.

### OA-RUST-062 — [WGPUI Web] Switch default router to Rust shell
Description: Make Rust shell the default web route handler with feature-flag rollback guard.

### OA-RUST-063 — [WGPUI Web] Remove Laravel/Inertia/React runtime dependencies
Description: Delete legacy frontend runtime dependencies and simplify deployment/build graph for web.

### OA-RUST-064 — [WGPUI Web] Decommission legacy web deployment assets and pipelines
Description: Remove no-longer-needed deployment artifacts tied to legacy web runtime.

### OA-RUST-065 — [WGPUI Web] Run performance/soak signoff for wasm shell
Description: Execute and record load, latency, and client stability soak tests before hard cutover completion.

## Phase 9: Data, Deploy, and Reliability Gates

### OA-RUST-066 — [Data] Finalize control/runtime DB role isolation policy and tooling
Description: Implement and validate schema/role isolation so cross-plane writes remain impossible by default.

### OA-RUST-067 — [Ops] Automate runtime migration job post-deploy and enforce runbook usage
Description: Ensure every runtime deploy executes migration job consistently and update runbooks/agent guidance to prevent drift.

### OA-RUST-068 — [Ops] Add replay drift and projector hash alarms in production
Description: Add production alarms for projection drift and replay determinism regressions.

### OA-RUST-069 — [Testing] Build cross-surface e2e harness (web/desktop/iOS)
Description: Add one contract test harness that verifies shared behavior against a single runtime contract set.

### OA-RUST-070 — [Reliability] Run restart/reconnect chaos drills for Khala/runtime
Description: Validate reconnect/resume behavior through controlled failure drills and document remediations.

### OA-RUST-071 — [Ops] Publish incident runbooks for WS/auth/stale-cursor failures
Description: Publish operator runbooks for the highest-risk runtime/Khala failure classes.

### OA-RUST-072 — [Docs] Mark `docs/ARCHITECTURE.md` historical or replace with Rust architecture
Description: Complete architecture documentation transition so legacy architecture docs cannot be mistaken for current target.

### OA-RUST-073 — [Release Gate] Endstate readiness review and go/no-go decision
Description: Validate all mandatory endstate outcomes, reliability gates, and parity requirements before declaring migration complete.

## Phase 10: New ADR Set (Rust Era)

### OA-RUST-074 — [ADR] Author ADR-0001 Rust-only architecture baseline
Description: Capture foundational architecture, boundaries, and non-goals for Rust-only OpenAgents.

### OA-RUST-075 — [ADR] Author ADR-0002 Proto-first contract governance
Description: Capture proto-first authority policy, codegen requirements, and wire/domain boundary rules.

### OA-RUST-076 — [ADR] Author ADR-0003 Khala WS-only replay transport
Description: Capture WS-only transport policy, replay/watermark semantics, and failure handling expectations.

### OA-RUST-077 — [ADR] Author ADR-0004 Rivet harvest posture and adoption boundaries
Description: Capture exactly which Rivet patterns are adopted and which platform-level semantics are explicitly rejected.

## Phase 11: Critical Hardening and Edge Cases

### OA-RUST-078 — [Auth] Codify WorkOS as canonical auth source of truth
Description: Implement and document WorkOS-authoritative identity/auth semantics across web/desktop/iOS and ensure local control-plane records are derivative, not primary identity authority.
Acceptance criteria: All sign-in/session entrypoints validate through WorkOS integration; no standalone local credential authority remains; architecture/runbook docs explicitly state WorkOS authority.
Dependencies: OA-RUST-010, OA-RUST-015, OA-RUST-016.

### OA-RUST-079 — [Auth] Implement refresh-token rotation and device session model
Description: Add strict refresh token rotation, revocation list behavior, and stable per-install `device_id` semantics used by auth/session/sync.
Acceptance criteria: Rotated refresh tokens are single-use; per-device revoke and global revoke are supported; device-scoped session queries are auditable.
Dependencies: OA-RUST-016, OA-RUST-018.

### OA-RUST-080 — [Auth/Khala] Enforce live WS eviction on session invalidation
Description: Propagate session revocation to active Khala sockets so unauthorized sessions are disconnected and forced through reauth.
Acceptance criteria: Revoked sessions are evicted within bounded latency; reconnect returns deterministic `reauth_required` behavior; end-to-end tests cover revoke-during-stream.
Dependencies: OA-RUST-018, OA-RUST-044.

### OA-RUST-081 — [WGPUI Web] Implement IndexedDB persistence layer and schema migrations
Description: Define browser persistence for watermarks and local state using IndexedDB with versioned migrations and corruption handling.
Acceptance criteria: State survives app reload/version upgrade; migration failure triggers safe reset path; no localStorage-only authority state remains.
Dependencies: OA-RUST-029, OA-RUST-030.

### OA-RUST-082 — [WGPUI Web] Implement service worker asset pinning and rollback policy
Description: Add service worker/update controls for JS/WASM artifact pinning, compatibility-aware updates, and rollback-safe cache invalidation.
Acceptance criteria: Stale asset skew is detectable; rollback to previous bundle is supported without protocol deadlock; release runbook includes update order.
Dependencies: OA-RUST-019, OA-RUST-023.

### OA-RUST-083 — [Protocol] Define compatibility negotiation and support window policy
Description: Publish ADR-level compatibility rules for `schema_version`, server/client negotiation behavior, and supported-version window policy.
Acceptance criteria: Compatibility matrix is documented; negotiation failures return explicit upgrade errors; policy is referenced by control service and Khala services.
Dependencies: OA-RUST-008, OA-RUST-075, OA-RUST-076.

### OA-RUST-084 — [Protocol] Enforce minimum client version in control service and Khala
Description: Implement minimum supported client version checks and coordinated rollout controls in control-plane APIs and websocket handshake flow.
Acceptance criteria: Min-version gates are configurable per environment; older clients receive deterministic upgrade path responses; telemetry captures rejection reasons.
Dependencies: OA-RUST-083.

### OA-RUST-085 — [Khala] Define retention, compaction, and snapshotting policy
Description: Establish explicit per-topic retention windows, compaction rules, and snapshot generation strategy to keep replay bounded.
Acceptance criteria: Retention policy exists for all topic classes; snapshot format/versioning is documented; automated tests verify replay correctness across compaction boundaries.
Dependencies: OA-RUST-038, OA-RUST-043.

### OA-RUST-086 — [Khala] Define topic QoS tiers, replay budgets, and stale-cursor policy
Description: Introduce topic QoS tiers (hot/cold), replay budget controls, and deterministic stale-cursor trigger conditions.
Acceptance criteria: Each topic is assigned a tier and replay budget; stale-cursor reasons are explicit and surfaced to clients; operator dashboards expose budget pressure.
Dependencies: OA-RUST-085.

### OA-RUST-087 — [Khala] Implement fairness and slow-consumer handling policy
Description: Implement per-connection buffer limits, fair fanout across topics, and explicit slow-consumer actions (throttle/disconnect/resync).
Acceptance criteria: Hot topic traffic cannot starve other topics; slow consumers are handled by policy; integration tests verify fairness under load.
Dependencies: OA-RUST-047.

### OA-RUST-088 — [Khala] Enforce per-topic rate limits and frame size limits
Description: Add server-side guardrails for publish/fanout rate and payload size to prevent abuse and protect cluster stability.
Acceptance criteria: Limits are configurable and observable; violations emit audited reason codes; clients receive deterministic error semantics.
Dependencies: OA-RUST-087.

### OA-RUST-089 — [Khala] Specify multi-node ordering and delivery semantics
Description: Define and implement multi-node ordering behavior with Postgres/runtime as ordering oracle and at-least-once delivery semantics.
Acceptance criteria: Per-topic ordering by `seq` is provable across nodes; at-least-once semantics are documented; idempotent-apply client guidance is published.
Dependencies: OA-RUST-041, OA-RUST-042, OA-RUST-043.

### OA-RUST-090 — [Data] Publish zero-downtime schema evolution playbook
Description: Define online migration patterns for proto and DB evolution with backward-compatible rollout rules and explicit cutover gates.
Acceptance criteria: Playbook includes expand/migrate/contract path; runtime+control-service compatibility sequencing is documented; migration tests cover mixed-version deploys.
Dependencies: OA-RUST-013, OA-RUST-067.

### OA-RUST-091 — [Testing] Extend shadow-mode verification to control service and Khala
Description: Add shadow read/write and parity-diff harnesses for control service and Khala, not only runtime, to derisk production cutovers.
Acceptance criteria: Shadow harness runs in non-prod and staged prod; parity diff reports are generated per deploy; release gate blocks on critical divergence.
Dependencies: OA-RUST-039, OA-RUST-090.

### OA-RUST-092 — [Security] Publish Khala WS threat model and anti-replay policy
Description: Create a websocket threat model covering token replay, origin enforcement, session hijack scenarios, and subscription audit requirements.
Acceptance criteria: Threat model and controls are documented; jti/TTL/origin checks are tested; audit events exist for auth failures and denied joins.
Dependencies: OA-RUST-044, OA-RUST-080.

### OA-RUST-093 — [Observability] Implement Khala SLO dashboards and client telemetry schema
Description: Define and ship golden-signal dashboards for Khala/runtime plus a privacy-safe client telemetry schema for reconnect, replay, and auth failure diagnostics.
Acceptance criteria: SLOs are defined and alerting is active; key metrics are tagged by topic/app version; client telemetry contract is versioned.
Dependencies: OA-RUST-068, OA-RUST-071.

### OA-RUST-094 — [WGPUI Web] Define WASM boot performance budget and capability fallback policy
Description: Define bundle-size and first-paint budgets, boot instrumentation, compression/caching strategy, and WebGPU fallback UX behavior.
Acceptance criteria: Budgets are enforced in CI or release checks; fallback matrix is documented and tested; performance telemetry is visible in dashboards.
Dependencies: OA-RUST-023, OA-RUST-065.

### OA-RUST-095 — [Payments] Define wallet-executor auth, key custody, and receipt canonicalization
Description: Formalize wallet-executor identity/auth channel, secret and key custody/rotation policies, and canonical payment receipt hashing contract.
Acceptance criteria: Executor auth path is documented and enforced; key rotation runbook exists; receipt proto/hash compatibility tests pass.
Dependencies: OA-RUST-060, OA-RUST-075.

### OA-RUST-096 — [Onyx] Define allowed integration surface and non-goals
Description: Specify allowed OpenAgents APIs for Onyx, identity model constraints, and explicit non-goals for offline/sync behavior.
Acceptance criteria: Onyx integration contract doc exists and is linked from architecture docs; API allowlist is enforced; cross-surface auth model is unambiguous.
Dependencies: OA-RUST-054, OA-RUST-072.

## Phase 12: Migration Closure Gaps (Repository Audit)

### OA-RUST-097 — [Build] Restore workspace-wide Rust compile baseline
Description: Fix outstanding workspace compile failures and enforce `cargo check --workspace --all-targets` as a migration gate so Rust surfaces remain continuously buildable during cutover.
Acceptance criteria: Workspace check passes in a clean checkout; known unresolved imports/type errors are removed; release gate references the workspace check explicitly.
Dependencies: OA-RUST-033, OA-RUST-049, OA-RUST-054.

### OA-RUST-098 — [Proto] Enforce Rust codegen in buf templates and verification scripts
Description: Update `buf.gen.yaml` and proto verification scripts to generate and validate Rust outputs in addition to existing targets, then wire that into local CI gates.
Acceptance criteria: Rust proto outputs generate deterministically; proto verification fails when Rust generation breaks; docs/runbooks reflect Rust codegen as mandatory.
Dependencies: OA-RUST-012, OA-RUST-013, OA-RUST-075.

### OA-RUST-099 — [Runtime] Replace Elixir runtime app scaffolding with Rust service scaffolding
Description: Complete runtime migration by removing `mix`/Phoenix execution dependencies from `apps/runtime` and replacing them with Rust service entrypoints, build scripts, tests, and deploy runbooks.
Acceptance criteria: `apps/runtime` builds/tests via Cargo-only workflow; no production runtime dependency on `mix`/Phoenix remains; runtime deploy docs are Rust-native.
Dependencies: OA-RUST-033, OA-RUST-040, OA-RUST-067.

### OA-RUST-100 — [Web Service] Remove Laravel/PHP runtime from `apps/openagents.com`
Description: After route and API parity, remove Laravel/PHP runtime dependencies and finalize `apps/openagents.com` as Rust control service + static WGPUI host.
Acceptance criteria: `composer`/PHP runtime no longer required for production deploys; control APIs and static hosting are served by Rust service only; rollback plan and migration notes are published.
Dependencies: OA-RUST-015, OA-RUST-062, OA-RUST-063, OA-RUST-064.

### OA-RUST-101 — [Services] Migrate `apps/lightning-ops` to Rust
Description: Port `apps/lightning-ops` service logic from TypeScript/Effect to Rust while preserving policy/reconcile behavior and operator diagnostics.
Acceptance criteria: Rust service reaches feature parity for compile/reconcile/smoke workflows; TypeScript runtime path is removed or archived; ops runbooks point to Rust service only.
Dependencies: OA-RUST-060, OA-RUST-066, OA-RUST-095.

### OA-RUST-102 — [Services] Migrate `apps/lightning-wallet-executor` to Rust
Description: Port wallet executor HTTP service and payment execution flows to Rust, including Spark/mock modes, auth controls, and deterministic smoke coverage.
Acceptance criteria: Rust executor passes parity tests in mock and live modes; TS runtime path is removed or archived; receipt/security contracts remain unchanged.
Dependencies: OA-RUST-060, OA-RUST-066, OA-RUST-095.

### OA-RUST-103 — [Packages] Retire legacy TypeScript package runtime lanes
Description: Migrate or archive `packages/*` TypeScript runtime dependencies (Effuse/khala-sync/lightning-effect family) so no production-critical runtime paths depend on Node/TypeScript execution.
Acceptance criteria: Remaining TS packages are either archived or explicitly non-production tooling; production runtimes import Rust crates only; dependency graph audit is documented.
Dependencies: OA-RUST-053, OA-RUST-058, OA-RUST-063, OA-RUST-101, OA-RUST-102.

### OA-RUST-104 — [CI] Replace legacy local CI lanes with Rust-first gates
Description: Rewrite local CI entrypoints to remove mandatory Laravel/Elixir lanes and establish Rust-first verification commands for services/clients/proto contracts.
Acceptance criteria: `scripts/local-ci.sh` and hooks run Rust-native checks for migrated surfaces; legacy checks are isolated to archival compatibility lanes only; CI docs are updated.
Dependencies: OA-RUST-097, OA-RUST-098, OA-RUST-099, OA-RUST-100.

### OA-RUST-105 — [Docs] Remove stale legacy surface references from canonical docs
Description: Update root documentation (`README`, `AGENTS`, `docs/README`, `docs/PROJECT_OVERVIEW`, `docs/AGENT_MAP`, `docs/ROADMAP`) to reflect current migration state and Rust endstate sequencing without stale active-surface claims.
Acceptance criteria: Canonical docs no longer list removed surfaces as active; architecture docs and roadmap references are consistent; docs-check gates include stale-surface detection.
Dependencies: OA-RUST-052, OA-RUST-053, OA-RUST-058, OA-RUST-072.

### OA-RUST-106 — [Sync Docs] Align Khala surface contracts with Rust-era client set
Description: Update `docs/sync/*` to remove legacy mobile/desktop/inbox assumptions and define the authoritative Rust-era consumer matrix (`openagents.com` wasm shell, autopilot-desktop, autopilot-ios, onyx integration scope).
Acceptance criteria: Sync docs and runbooks match active client architecture; old lane references are archived; release drills use Rust-era surfaces only.
Dependencies: OA-RUST-048, OA-RUST-058, OA-RUST-062, OA-RUST-072.

### OA-RUST-107 — [iOS Platform] Add deterministic Rust core packaging pipeline for iOS host
Description: Add reproducible Rust-to-iOS packaging (FFI boundary, artifacts, build scripts) so iOS business/sync logic runs in shared Rust core with Swift limited to host integration.
Acceptance criteria: iOS build pipeline consumes versioned Rust artifacts deterministically; shared Rust client core owns sync/business logic paths; Swift host boundary is documented and minimal.
Dependencies: OA-RUST-054, OA-RUST-055, OA-RUST-056.

## Completion Criteria Summary

Migration is complete only when all of the following are true:

1. `apps/mobile/`, `apps/desktop/`, and standalone `apps/inbox-autopilot/` are removed.
2. `apps/openagents.com` runs Rust control service + Rust/WGPUI web shell in production.
3. Runtime authority writes and projections are Rust-owned.
4. Khala websocket is the sole live sync lane.
5. New ADR set is published and old ADRs are archived.
6. Cross-surface contract/e2e/replay gates pass in release workflow.
7. WorkOS is authoritative for identity/auth across all client surfaces.
8. Khala retention/compaction/snapshot policy is active and validated in production.
9. Proto verification gates enforce Rust code generation and compatibility policy.
10. `apps/runtime` and `apps/openagents.com` no longer require Elixir/PHP runtimes in production.
11. `apps/lightning-ops` and `apps/lightning-wallet-executor` run on Rust implementations.
12. Legacy TypeScript runtime packages are retired or explicitly non-production.
13. Local CI and hooks use Rust-first verification gates for migrated surfaces.
14. Canonical docs and sync runbooks are aligned to active Rust-era surfaces only.
