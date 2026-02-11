# OpenAgents Local LND Neutrino (Electron) Implementation Plan

Status: Active plan, phases N1-N9 implemented, N10 synchronization in progress
Date: 2026-02-11
Parent epic: #1605
Related hosted epic: #1595

## 1) Objective

Implement a pre-Voltage local-node track where the desktop app runs LND in Neutrino mode and executes real L402 agent payments for the same authenticated OpenAgents user. This gives us a production-shaped local execution boundary before hosted node infrastructure rollout.

This plan is intentionally synchronized with the current repo reality:

- Desktop app exists but still runs a demo task provider (`apps/desktop/src/effect/taskProvider.ts`).
- Web/Convex already has a typed Lightning task state machine (`apps/web/convex/lightning/tasks.ts`).
- Buyer-side L402 library exists (`packages/lightning-effect`) and now includes seller contracts for hosted work.
- Hosted infra issues (#1597-#1604) are open and should be sequenced around this local-node track, not ignored.

## 1.1) Phase Status Snapshot (2026-02-11)

Completed and closed:

1. N1: #1606
2. N2: #1607
3. N3: #1608
4. N4: #1609
5. N5: #1610
6. N6: #1611
7. N7: #1612
8. N8: #1613
9. N9: #1614

Current synchronization phase:

1. N10: #1615
   - Synchronize hosted-track assumptions (`#1594`, `#1595`, `#1599`, `#1604`) to local-node outputs.
   - Ensure dual-path verification matrix remains explicit and non-conflicting.

## 2) Target Runtime Shape

### Control/orchestration

- `openagents.com` web app and Worker remain orchestration and policy surfaces.
- Convex remains the source of truth for task lifecycle and payment metadata.

### Execution boundary

- `apps/desktop` main process manages local `lnd` process lifecycle.
- Renderer remains sandboxed (`contextIsolation: true`, `sandbox: true`, `nodeIntegration: false`) and only receives safe status/state projections.

### Shared packages

- New package `packages/lnd-effect` provides typed Effect services/contracts for LND RPC (app-agnostic, reusable by others).
- `packages/lightning-effect` consumes `lnd-effect` via adapters rather than embedding app logic.

## 3) Sequencing Rules vs Existing Open Issues

1. `#1597` and `#1598` can proceed in parallel.
2. `#1599` remains gated on completion of local-node baseline validation (`#1614`) plus sync phase (`#1615`).
3. `#1594` must include local desktop-node observability fields and dual-path rehearsal runbooks.
4. `#1604` must verify both hosted-L402 and local-node-L402 paths in one CI matrix.
5. Any hosted phase that introduces new correlation IDs must keep artifact parity with local-node smoke outputs.

## 4) Issue Breakdown (Create These Under #1605)

## Issue Phase N1: Create `packages/lnd-effect` scaffold and public API boundaries

Create a new Effect-first package at `packages/lnd-effect` with strict TS settings, subpath exports, typed errors, and initial service tags. This issue defines stable package boundaries so desktop and future external consumers can depend on one canonical LND interface.

The package should mirror current package conventions (`packages/lightning-effect`, `packages/effuse*`) and explicitly avoid `apps/*` imports. Include README usage examples and a migration note describing how `lnd-effect` complements (not replaces) `lightning-effect`.

Primary touchpoints: `packages/lnd-effect/package.json`, `packages/lnd-effect/src/{contracts,errors,services,layers,adapters}`, `packages/lnd-effect/README.md`.

## Issue Phase N2: Implement `lnd-effect` contracts, transports, and deterministic test adapters

Implement typed contracts and services for the first required RPC surface: node info/sync status, wallet state, invoice create/list/lookup, payment send/track, and basic balances. Add REST transport first for pragmatic Electron integration, with gRPC planned as follow-up transport in the same service contracts.

Add deterministic in-memory/mock adapters so contract tests do not require a live node. Include typed decode/transport/auth errors and contract-focused coverage gates, matching the rigor already used in `packages/lightning-effect`.

Primary touchpoints: `packages/lnd-effect/src/contracts/*`, `packages/lnd-effect/src/adapters/*`, `packages/lnd-effect/test/*`, `packages/lnd-effect/vitest.contracts.config.ts`.

## Issue Phase N3: Add desktop LND binary packaging and runtime path resolution

Package `lnd` as an Electron-side binary resource with pinned version and integrity verification strategy. Implement platform-specific binary resolution in the main process and wire it into Electron Forge packaging so installers include the correct binary artifacts.

This issue should define where binaries come from (pinned release artifacts and checksums) and where local development may optionally build from `/Users/christopherdavid/code/lnd/` without hardcoding that path into product code.

Primary touchpoints: `apps/desktop/forge.config.ts`, `apps/desktop/scripts/*`, `apps/desktop/src/main.ts` (or extracted main-process runtime modules), `apps/desktop/README.md`.

## Issue Phase N4: Implement LND Neutrino lifecycle manager in desktop main process

Add a main-process Effect service for `lnd` lifecycle: generate deterministic config, start/stop/restart process, health checks, log capture, and crash backoff. Configure Neutrino mode by default and isolate app data under OS app-data directories.

The service should expose a typed runtime status model to renderer-safe state (no raw secrets) and support non-interactive process control for test automation.

Primary touchpoints: `apps/desktop/src/effect/*` (new main-process services), `apps/desktop/src/main.ts`, `apps/desktop/tests/*`.

## Issue Phase N5: Wallet bootstrap/unlock/recovery and secure secret handling

Implement wallet lifecycle flows (init wallet, unlock wallet, detect locked state) through `lnd-effect`, with passphrase and sensitive material stored via OS keychain/secure storage abstractions. Ensure no secret-bearing values are sent to renderer or web layers.

Include explicit recovery paths (seed backup acknowledgement, restore flow state) and deterministic failure taxonomy surfaced to desktop state and Convex task results.

Primary touchpoints: `apps/desktop/src/effect/*`, `apps/desktop/src/preload.ts`, `apps/desktop/src/renderer.ts`, `packages/lnd-effect/src/services/*`.

## Issue Phase N6: Wire `lightning-effect` invoice/payment adapters to `lnd-effect`

Add integration adapters so `packages/lightning-effect` can use `lnd-effect` as its payment backend in desktop execution mode. Preserve current `lightning-effect` contracts and ensure proof material (preimage reference) remains first-class for receipts and L402 authorization.

This issue should keep adapters composable so later hosted/Voltage executors can reuse the same `lightning-effect` higher-level flows.

Primary touchpoints: `packages/lightning-effect/src/adapters/*`, `packages/lightning-effect/src/layers/*`, `packages/lightning-effect/test/*`, `packages/lnd-effect/*`.

## Issue Phase N7: Replace desktop demo task provider with Convex-backed real executor

Replace the in-memory demo queue in `apps/desktop/src/effect/taskProvider.ts` with a real provider that consumes the existing Convex task lifecycle (`queued/approved/running/paid/cached/blocked/failed/completed`) for the same signed-in user. Execute real L402 tasks through `lightning-effect` + `lnd-effect` and write deterministic task transitions/results back.

This issue closes the runtime loop from `openagents.com` to local node payment execution and is the core product milestone for local agent payments.

Primary touchpoints: `apps/desktop/src/effect/taskProvider.ts`, `apps/desktop/src/effect/executorLoop.ts`, `apps/web/convex/lightning/tasks.ts` (as needed), `apps/web/src/effect/lightning.ts` (status alignment).

## Issue Phase N8: Add/upgrade panes for node, wallet, transactions, and executor state

Expand desktop pane surfaces (and web L402 pane projections where needed) so operators can see local node status, sync progress, wallet balances, payment history, and executor failures clearly. Reuse existing Effuse pane system patterns in both desktop and web surfaces.

The goal is operational clarity: if a payment fails, the user should see whether it is policy, node sync, wallet lock, transport, or endpoint failure.

Primary touchpoints: `apps/desktop/src/renderer.ts`, `apps/desktop/src/index.css`, `packages/effuse-panes/*`, `apps/web/src/effuse-app/controllers/home/openChatPaneController.ts`.

## Issue Phase N9: Programmatic full-flow testing and CI-safe smoke harness

Add non-interactive tests that verify the full path: web task creation -> Convex orchestration -> desktop executor -> local LND payment flow -> task/result projection. Include deterministic test adapters and a smoke harness for real local-node runs.

This issue must add clear machine-runnable commands for agents and contributors. It should include CI-safe deterministic tests plus an opt-in local Neutrino smoke command for real-device validation.

Primary touchpoints: `apps/desktop/tests/*`, `packages/lnd-effect/test/*`, `packages/lightning-effect/test/*`, `apps/web/tests/*`, `apps/desktop/scripts/*`.

## Issue Phase N10: Hosted-track handoff and issue graph synchronization

Once N1-N9 are complete, update hosted-track sequencing so `#1599` and downstream hosted rollout proceed with validated local execution experience and shared contracts. Explicitly update `#1594` and `#1604` acceptance criteria to include desktop executor/local-node observability and verification where appropriate.

This issue is the control-plane governance step that prevents local and hosted tracks from drifting into incompatible models.

Primary touchpoints: issue updates for `#1594`, `#1595`, `#1599`, `#1604`, plus any roadmap docs under `docs/lightning/` that require sequencing updates.

## 5) Verification Contract by Phase

Each issue above must include explicit, non-interactive verification commands in its description. Minimum expected command set by the end of the plan:

1. `cd packages/lnd-effect && npm run typecheck && npm test`
2. `cd packages/lightning-effect && npm run typecheck && npm test`
3. `cd apps/desktop && npm run typecheck && npm test`
4. `cd apps/desktop && npm run test:l402-local-node-smoke` (new)
5. `cd apps/web && npm test` (task/control-plane integration assertions)

## 6) Definition of Done for #1605

1. Desktop app can launch and manage local LND Neutrino runtime with deterministic state transitions.
2. `lnd-effect` exists as a reusable Effect package and is usable outside OpenAgents apps.
3. Web-triggered L402 tasks execute via local desktop node for the same authenticated user.
4. Wallet/node secrets remain outside browser/worker code paths.
5. Programmatic end-to-end verification exists and is runnable by agents without manual clicking.
6. Hosted track issues remain compatible and explicitly synchronized for post-local rollout.

## 7) Handoff to Hosted Track (#1595)

After N10 closes, hosted rollout phases should follow this order:

1. Continue control/compiler foundation in parallel (`#1597`, `#1598`).
2. Execute staging deploy/reconcile (`#1599`) with correlation parity to local artifacts.
3. Land settlement/security/tooling panes phases (`#1600`-`#1603`).
4. Close with dual-path CI/programmatic verification (`#1604`), including local-node regression checks.
