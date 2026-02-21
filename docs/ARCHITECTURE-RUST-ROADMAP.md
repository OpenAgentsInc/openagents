# OpenAgents Rust-Only Architecture Roadmap

Status: Draft issue backlog source (do not create issues yet)
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

## How To Use This For GitHub Issues Later

When ready, each item below can be created with `gh issue create`.

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

### OA-RUST-003 — [Rust Endstate] Lock migration invariants from architecture doc
Description: Convert architecture invariants (proto-first, dual authority planes, Khala projection-only, WS-only sync) into explicit checklists and required gates for every implementation PR.

### OA-RUST-004 — [Rust Endstate] Archive existing ADR set
Description: Move current `docs/adr/` records to an archived path with index updates so the new Rust-era ADR series can begin cleanly.

### OA-RUST-005 — [Rust Endstate] Reinitialize ADR index and numbering for Rust era
Description: Create new ADR index, numbering conventions, and templates focused on Rust-only service/client architecture and migration decisions.

### OA-RUST-006 — [Rust Endstate] Define migration KPIs and reporting dashboard
Description: Define measurable migration KPIs (route parity, runtime parity, WS reliability, latency, regression count) and publish a recurring status artifact.

## Phase 1: Proto and Contract Foundation

### OA-RUST-007 — [Proto] Publish Rust-era package map for control/runtime/sync domains
Description: Finalize proto package ownership and namespace boundaries for `control`, `runtime`, `sync/khala`, and Codex surfaces so contract growth is structured.

### OA-RUST-008 — [Proto] Add Khala frame envelope contract
Description: Define/lock `KhalaFrame` envelope fields (`topic`, `seq`, `kind`, `payload_bytes`, `schema_version`) and publish compatibility rules.

### OA-RUST-009 — [Proto] Finalize runtime orchestration contracts
Description: Define run lifecycle, worker lifecycle, and durable receipt/replay proto messages used by runtime authority and clients.

### OA-RUST-010 — [Proto] Finalize control-plane auth/session/scope contracts
Description: Define control-plane messages for auth/session, org membership, and sync token scope derivation used by web, desktop, and iOS.

### OA-RUST-011 — [Proto] Finalize Codex worker/event contracts for all surfaces
Description: Lock Codex worker event envelopes, turn status updates, and replay metadata contracts so web/desktop/iOS consume the same wire protocol.

### OA-RUST-012 — [Proto] Stand up Rust codegen workspace crate(s)
Description: Create shared generated Rust crates and enforce deterministic generation output across services and clients.

### OA-RUST-013 — [Proto] Enforce buf lint/breaking/generate gates in local CI lanes
Description: Wire proto compatibility gates into local CI so breaking schema changes cannot land without explicit versioned evolution.

### OA-RUST-014 — [Proto] Add wire-to-domain conversion test harness
Description: Add tests validating `TryFrom`/`From` conversions between proto wire types and Rust domain models, including invalid payload behavior.

## Phase 2: Rust Edge/Control Service (`apps/openagents.com/service`)

### OA-RUST-015 — [Web Service] Bootstrap Rust edge/control service skeleton
Description: Create Rust service foundation for auth, control APIs, sync token minting, and static wasm asset hosting at `apps/openagents.com`.

### OA-RUST-016 — [Web Service] Implement auth/session API parity
Description: Port auth/session endpoints and token lifecycle semantics required by current clients, preserving security and audit invariants.

### OA-RUST-017 — [Web Service] Implement org membership and policy API parity
Description: Port org membership and policy enforcement APIs currently required for runtime access and scoped actions.

### OA-RUST-018 — [Web Service] Implement sync token minting and scope derivation
Description: Implement secure sync token issuance with topic scope validation for Khala subscriptions across web/desktop/iOS.

### OA-RUST-019 — [Web Service] Implement static host for versioned wasm assets
Description: Serve hashed JS/WASM artifacts and static manifest from Rust edge service with cache-control/versioning strategy.

### OA-RUST-020 — [Web Service] Add request correlation, audit logs, and structured observability
Description: Add structured logs, request correlation IDs, and audit events equivalent to current operational requirements.

### OA-RUST-021 — [Web Service] Add feature-flagged route split between legacy and Rust web shell
Description: Implement routing controls to gradually shift traffic route-by-route to Rust/WGPUI while preserving rollback.

### OA-RUST-022 — [Web Service] Canary deploy and rollback runbook for Rust edge
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

## Phase 4: Rust Runtime Authority (`apps/openagents-runtime`)

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

## Completion Criteria Summary

Migration is complete only when all of the following are true:

1. `apps/mobile/`, `apps/desktop/`, and standalone `apps/inbox-autopilot/` are removed.
2. `apps/openagents.com` runs Rust edge service + Rust/WGPUI web shell in production.
3. Runtime authority writes and projections are Rust-owned.
4. Khala websocket is the sole live sync lane.
5. New ADR set is published and old ADRs are archived.
6. Cross-surface contract/e2e/replay gates pass in release workflow.
