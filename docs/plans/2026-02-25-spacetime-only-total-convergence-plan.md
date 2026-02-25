# Spacetime-Only Total Convergence Plan (Zero Legacy)

Status: active execution plan
Date: 2026-02-25
Owner lanes: `owner:runtime`, `owner:autopilot`, `owner:control`, `owner:protocol`, `owner:docs`

## 1) Mandate

Move retained OpenAgents runtime and client sync surfaces to Spacetime-only operation, and delete all legacy sync transport paths.

Target posture:

1. Runtime sync publication and client live sync run through Spacetime only.
2. No legacy websocket protocol lanes remain (`/sync/socket/websocket`, Phoenix frame semantics, topic poll/fanout leftovers).
3. No compatibility aliases remain unless there is a documented, time-bounded, firm technical hurdle.

## 2) Firm Technical Hurdle Policy

A temporary exception is allowed only if all of these are true:

1. There is a concrete blocker not solvable within the issue scope (protocol/runtime limitation, safety/custody risk, or deterministic replay breakage).
2. The exception has an owner, expiry date, and deletion issue linked in the PR.
3. The exception is bounded to a single surface, with explicit blast radius and rollback steps.

Any exception older than 14 days without renewal evidence is treated as non-compliant and must be removed.

## 3) Comparison with Existing Plan and Prior Issues

Compared artifacts:

1. `docs/plans/2026-02-25-spacetimedb-autopilot-primary-comms-integration-plan.md`
2. Prior issue set `OA-SPACETIME-001` through `OA-SPACETIME-038` (all currently closed)

Comparison result:

1. The prior plan and issue set covered many correct migration themes (schema, reducers, tokening, desktop lifecycle, rollout, docs).
2. Current retained code still shows non-converged legacy behavior in critical paths (legacy websocket pathing and in-memory runtime publication).
3. Therefore this backlog is a remediation/convergence set focused on hard acceptance gates, deletion guarantees, and anti-regression enforcement.

What changed in this plan after comparison:

1. Every issue is framed as enforceable end-state work, not transition-only activity.
2. Legacy deletion and regression prevention are first-class outcomes, not optional cleanup.
3. The backlog assumes prior issues are historical context; this set defines the required final convergence standard.

## 4) Current Gap Snapshot (Code Evidence)

As of this plan date:

1. Desktop still builds websocket URL as `/sync/socket/websocket` in `apps/autopilot-desktop/src/main.rs:818`.
2. Desktop still mints sync token via `/api/spacetime/token` in `apps/autopilot-desktop/src/main.rs:778`.
3. Runtime state currently wires `SpacetimePublisher::in_memory()` in `apps/runtime/src/lib.rs:62`.
4. Runtime publisher still uses local in-process `ReducerStore` in `apps/runtime/src/spacetime_publisher.rs:31`.
5. `crates/autopilot-spacetime` is not yet a fully wired network client crate (`Cargo.toml` dependencies are currently empty).

## 5) End State Definition (Done Means Done)

The program is complete only when all are true:

1. Runtime writes sync projection events to real Spacetime reducers on configured host(s), not in-memory stores.
2. Desktop connects directly to Spacetime subscribe endpoint semantics (`/v1/database/:name_or_identity/subscribe`) and applies `SubscribeApplied` + `TransactionUpdate`.
3. Connected-user visibility is backed by Spacetime presence tables and includes Nostr-facing identity fields where applicable.
4. Legacy transport code and routes are physically deleted, not only disabled behind flags.
5. CI/verification gates fail on any reintroduction of legacy transport symbols/paths.

## 6) Ordered GitHub Issue Backlog (Title + Body)

### Issue 1: `OA-SPACETIME-TOTAL-001` ADR Supersession for Spacetime-Only Runtime Transport

Create and land a superseding ADR that explicitly mandates Spacetime-only runtime/client sync transport and retirement of legacy websocket lanes. The ADR must define the no-legacy policy, allowed temporary exception process, and final compliance criteria so all implementation PRs are auditable against one canonical decision record.

### Issue 2: `OA-SPACETIME-TOTAL-002` Invariant Gate Update for Zero-Legacy Sync Policy

Update `docs/plans/rust-migration-invariant-gates.md` to remove ambiguous transitional language and encode hard gates for Spacetime-only live sync transport. Add explicit checks that legacy fallback lanes are forbidden unless attached to an approved exception with expiry and owner.

### Issue 3: `OA-SPACETIME-TOTAL-003` Canonical Spacetime Module in Repo (No More `/tmp` Bootstrap)

Add a first-class Spacetime module in the repository (schema, reducers, lifecycle hooks, migrations) so production/dev behavior is versioned and reviewable. This replaces temporary local bootstrap modules and becomes the only source of truth for sync presence, stream events, checkpoints, and handshake state.

### Issue 4: `OA-SPACETIME-TOTAL-004` Environment Canonicalization for Dev/Staging/Prod Spacetime Targets

Define and enforce canonical environment variable contracts for all retained surfaces that connect to Spacetime. Implement strict startup validation so missing base URL/database/scope claims fail fast with actionable errors instead of silently falling back to legacy paths.

### Issue 5: `OA-SPACETIME-TOTAL-005` Automated Spacetime Publish/Promote Script Lane

Implement repeatable publish scripts for module build/publish/promote across environments with deterministic version tagging and post-publish verification. The lane must support Maincloud and chosen non-dev hosting path, with explicit checks for schema drift and rollback safety.

### Issue 6: `OA-SPACETIME-TOTAL-006` Runtime Publisher: Replace In-Memory Store With Real Spacetime Client Writes

Refactor runtime publication so all sync messages are written to real Spacetime reducers over network transport, removing `SpacetimePublisher::in_memory` from retained runtime execution. Ensure idempotency keys, stream sequencing, and payload hashing remain deterministic and test-covered.

### Issue 7: `OA-SPACETIME-TOTAL-007` Runtime Durable Outbox and Retry Semantics for Spacetime Reducer Calls

Introduce durable outbound buffering/retry behavior for runtime-to-Spacetime publication to survive transient failures without duplication or ordering regression. This issue must include explicit retry class handling, backoff limits, and replay-safe idempotency guarantees.

### Issue 8: `OA-SPACETIME-TOTAL-008` Runtime Legacy Endpoint Deletion (Topic Poll/Fanout/Legacy Spacetime Routes)

Delete legacy runtime sync endpoints and route handlers that are no longer part of the Spacetime-only model. Any retained compatibility route must be justified through the exception policy; otherwise delete code, tests, and docs in the same PR.

### Issue 9: `OA-SPACETIME-TOTAL-009` Control Token API Canonicalization for Spacetime Session Claims

Define one canonical control endpoint contract for issuing short-lived Spacetime session claims with explicit scope and refresh policy. Remove ambiguity between legacy aliases and ensure client integrations consume one authoritative schema.

### Issue 10: `OA-SPACETIME-TOTAL-010` Remove Convex/Legacy Naming and Alias Drift in Control/Web Surfaces

Eliminate stale naming and route artifacts (`convex`, deprecated aliases, historical scaffolding) that create confusion around active Spacetime auth/session flows. This issue includes code, generated client actions, docs, and tests to align terminology and behavior.

### Issue 11: `OA-SPACETIME-TOTAL-011` Desktop Transport Cutover to Spacetime Subscribe Endpoint Semantics

Replace desktop’s legacy websocket transport construction and handshake assumptions with direct Spacetime subscribe semantics, including protocol negotiation and query set lifecycle. The implementation must remove dependence on legacy path construction and prove live subscription correctness.

### Issue 12: `OA-SPACETIME-TOTAL-012` Desktop Parser Cutover: Remove Phoenix Frame Compatibility Code

Delete Phoenix-style frame parsing/building and replace with typed Spacetime message handling (`SubscribeApplied`, `TransactionUpdate`, and structured error handling). Keep replay/resume behavior deterministic and covered by contract tests.

### Issue 13: `OA-SPACETIME-TOTAL-013` Desktop Checkpoint/Resume Hardening for Stream-Based Replay

Finalize checkpoint persistence and reconnect logic using stream-based sequence semantics with stale-cursor recovery behavior. The result must guarantee idempotent apply, bounded retry loops, and explicit recovery paths when resume points are invalid.

### Issue 14: `OA-SPACETIME-TOTAL-014` Desktop Presence Pane: Connected Users Count Backed by Spacetime

Add/finish desktop UI surface that shows live connected-user count directly from Spacetime presence state, with clear refresh/connection status. This issue should provide immediate operator visibility for handshake validation and runtime health.

### Issue 15: `OA-SPACETIME-TOTAL-015` Nostr Identity Binding in Spacetime Presence (`npub`/hex)

Implement explicit Nostr identity binding in Spacetime presence tables so connected users can be displayed as Nostr pubkeys (`hex` and/or `npub`) rather than internal Spacetime identities. Include signature proof workflow to prevent arbitrary spoofed key claims.

### Issue 16: `OA-SPACETIME-TOTAL-016` Promote `autopilot-spacetime` to Real Network Client Crate

Add concrete Spacetime client dependencies and production-grade connect/subscribe/reducer call APIs in `crates/autopilot-spacetime`. This crate must become the shared typed client layer for runtime/desktop usage instead of simulation-only helpers.

### Issue 17: `OA-SPACETIME-TOTAL-017` Proto/Contract Alignment for Stream Envelope, Checkpoint, and Errors

Align proto contracts with final Spacetime stream semantics and remove legacy topic/frame assumptions from protocol surfaces. Generated contracts must encode replay keys, checkpoint shape, and typed failure classes used by runtime and desktop.

### Issue 18: `OA-SPACETIME-TOTAL-018` Two-Client Handshake Integration Test in Repo (Automated)

Land an automated integration test that establishes two concurrent clients, verifies live `connected_clients=2`, then verifies cleanup to `0` on disconnect. This test becomes a required gate for any transport/auth/session change.

### Issue 19: `OA-SPACETIME-TOTAL-019` Runtime-to-Desktop End-to-End Sync Test Suite (Spacetime-Only)

Implement E2E tests proving runtime publication reaches desktop via Spacetime only, with deterministic ordering and dedupe behavior. Tests must fail if legacy routes are exercised or if in-memory publisher behavior is used in retained runtime paths.

### Issue 20: `OA-SPACETIME-TOTAL-020` Chaos Suite for Reconnect, Stale Cursor, and Duplicate Delivery

Add chaos drills targeting network churn, reconnect storms, stale cursors, and duplicate deliveries under Spacetime-only operation. Promotion to later phases requires green chaos evidence and documented rollback posture.

### Issue 21: `OA-SPACETIME-TOTAL-021` Observability Baseline for Spacetime-Only Runtime

Add metrics and dashboards for connection counts, sequence lag, replay gap, duplicate suppression, and publish failure classes. Alerts must be wired to runbooks and include actionable thresholds for on-call response.

### Issue 22: `OA-SPACETIME-TOTAL-022` Remove Feature Flags and Fallback Toggles for Legacy Sync Lanes

Once parity gates are green, delete fallback feature flags and conditional branches that can route traffic to legacy sync code. This issue must remove both runtime toggles and desktop toggles so Spacetime-only operation is enforced by code, not config preference.

### Issue 23: `OA-SPACETIME-TOTAL-023` Repo-Wide Legacy Transport Symbol Purge

Perform a hard removal of legacy transport symbols, constants, and helper names that represent old websocket/topic protocol behavior. Include a verification script that fails CI if blocked strings or paths are reintroduced.

### Issue 24: `OA-SPACETIME-TOTAL-024` Active Docs Realignment to Spacetime-Only Canonical Architecture

Update all active architecture, protocol, sync, and runbook docs to reflect final Spacetime-only behavior and remove contradictory legacy references. Archive superseded materials to backroom and remove stale pointers from active indexes.

### Issue 25: `OA-SPACETIME-TOTAL-025` Deployment Runbook Finalization for Spacetime-Only Operation

Finalize one canonical deployment/promotion/rollback runbook for Spacetime-only runtime behavior across environments, with explicit verification commands and go/no-go gates. Ensure operators can cut over and recover without invoking legacy lanes.

### Issue 26: `OA-SPACETIME-TOTAL-026` Final Legacy Code Deletion and Compliance Signoff

Execute final deletion pass for any remaining legacy transport code and produce a signoff artifact proving no retained runtime/client path depends on legacy sync behavior. Signoff must include code search evidence, green test gates, and documentation parity confirmation.

## 7) Execution Order and Exit Gates

Execute strictly in order:

1. Issues 1-5 (governance + deploy scaffolding)
2. Issues 6-10 (runtime/control canonicalization)
3. Issues 11-17 (desktop + client + identity binding)
4. Issues 18-21 (verification + observability hardening)
5. Issues 22-26 (legacy deletion + final signoff)

Exit gates:

1. No legacy websocket path/symbols remain in retained runtime/desktop code.
2. Runtime uses real Spacetime publication path in production code.
3. Two-client handshake and E2E sync suites are green.
4. Active docs and runbooks are aligned with final behavior.
