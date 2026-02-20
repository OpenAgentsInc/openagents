# Convex Usage and Proto-First Sync Engine Audit (2026-02-20)

## Scope

This audit covers:

1. How Convex is currently used across the OpenAgents codebase.
2. What is actually active in code versus stated in docs/plans.
3. Feasibility of replacing Convex with an OpenAgents-owned sync engine on Postgres (Cloud SQL) + WebSockets.
4. How to move schema authority to proto (instead of TypeScript-defined schemas).
5. Whether to keep Convex, replace it, or run a hybrid migration.

Primary inputs reviewed:

- OpenAgents:
  - `README.md`
  - `docs/ARCHITECTURE.md`
  - `docs/adr/ADR-0029-convex-sync-layer-and-codex-agent-mode.md`
  - `docs/plans/active/convex-self-hosting-runtime-sync-plan.md`
  - `docs/plans/active/convex-runtime-codex-master-roadmap.md`
  - `apps/openagents-runtime/docs/CONVEX_SYNC.md`
  - runtime, web, mobile, desktop, lightning-ops source files listed throughout this audit
- Convex upstream local clone:
  - `/Users/christopherdavid/code/convex/README.md`
  - `/Users/christopherdavid/code/convex/convex-backend/self-hosted/README.md`
  - `/Users/christopherdavid/code/convex/convex-backend/self-hosted/advanced/postgres_or_mysql.md`
  - `/Users/christopherdavid/code/convex/convex-backend/npm-packages/docs/docs/database/schemas.mdx`
  - `/Users/christopherdavid/code/convex/convex-backend/npm-packages/docs/docs/functions/validation.mdx`
  - `/Users/christopherdavid/code/convex/convex-backend/npm-packages/docs/docs/auth/advanced/custom-jwt.mdx`
  - `/Users/christopherdavid/code/convex/convex-js/src/browser/sync/*`

## Executive Summary

OpenAgents currently uses Convex in two very different ways:

1. Runtime Codex sync plane (projection-only, non-authoritative), aligned with ADR-0029.
2. Lightning ops control plane (authoritative for paywall/policy compiler inputs), still directly dependent on Convex functions.

Key conclusion:

- If the goal is proto-first contracts and no hand-authored TypeScript schema definitions, OpenAgents can achieve that.
- Full Convex replacement is feasible but should not be done as a single cutover.
- Recommended path is a hybrid migration:
  1. Keep Convex for Lightning ops short-term.
  2. Build an OpenAgents sync service for runtime/Codex first (Postgres + WebSockets + proto contracts).
  3. Migrate Lightning ops off Convex as a second phase once runtime/Codex migration is stable.

## Current Convex Usage (Code Audit)

## 1) Runtime (`apps/openagents-runtime`) - projection/sync integration

Architecture intent is explicit and consistent:

- `apps/openagents-runtime/docs/CONVEX_SYNC.md` declares runtime as source-of-truth and Convex as projection-only.
- `docs/adr/ADR-0029-convex-sync-layer-and-codex-agent-mode.md` locks single-writer projection and Laravel auth bridge.

Implemented runtime pieces:

- Projector + sinks:
  - `apps/openagents-runtime/lib/openagents_runtime/convex/projector.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/convex/sink.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/convex/http_sink.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/convex/noop_sink.ex`
- Checkpoints + replay:
  - `apps/openagents-runtime/lib/openagents_runtime/convex/projection_checkpoint.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/convex/reprojection.ex`
  - `apps/openagents-runtime/lib/mix/tasks/runtime.convex.reproject.ex`
  - `apps/openagents-runtime/priv/repo/migrations/20260219101000_create_runtime_convex_projection_checkpoints.exs`
- Event append hooks trigger projection:
  - `apps/openagents-runtime/lib/openagents_runtime/runs/run_events.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/codex/workers.ex`

Important implementation details:

- Projected docs are deterministic summaries keyed as:
  - `runtime/run_summary:<run_id>`
  - `runtime/codex_worker_summary:<worker_id>`
- Runtime keeps idempotent checkpoints in Postgres (`runtime.convex_projection_checkpoints`) with hash + seq + version.
- Drift and replay tooling exists, with telemetry and tests.

Operational caveat found:

- Default runtime sink is `NoopSink` in `apps/openagents-runtime/config/config.exs`.
- `HttpSink` exists, but this repo does not show runtime env wiring in `config/runtime.exs` to switch sink per environment.
- Convex mutation targets (`runtime:upsertRunSummary`, `runtime:upsertCodexWorkerSummary`) are configured in sink defaults, but matching Convex function definitions are not present in this repo.

Net: runtime side is architecturally strong for a projection system, but deployment wiring appears partially external or incomplete in-repo.

## 2) Laravel web app (`apps/openagents.com`) - auth bridge + migration tooling

Active Convex usage in Laravel:

- Token bridge endpoint:
  - `POST /api/convex/token` in `apps/openagents.com/routes/api.php`
  - controller: `apps/openagents.com/app/Http/Controllers/Api/ConvexTokenController.php`
  - issuer: `apps/openagents.com/app/Support/Convex/ConvexTokenIssuer.php`
  - config: `apps/openagents.com/config/convex.php`
  - tests: `apps/openagents.com/tests/Feature/Api/ConvexTokenApiTest.php`

Behavior:

- Laravel mints short-lived JWTs for Convex client auth.
- Current implementation signs with HS256 shared secret.
- Convex custom JWT docs show RS256/ES256 examples as the standard path for JWKS-based verification.

Legacy migration path still present:

- Convex export -> Laravel import runbook and services:
  - `apps/openagents.com/docs/CONVEX_PROD_EXPORT_AND_LARAVEL_IMPORT.md`
  - `apps/openagents.com/app/Support/ConvexImport/*`
  - `apps/openagents.com/routes/console.php` (`convex:import-chat`)

## 3) Mobile app (`apps/mobile`) - Convex client bootstrapped, runtime APIs used for Codex features

Convex client/auth wiring is active:

- `apps/mobile/app/app.tsx` initializes `ConvexReactClient` + `ConvexProviderWithAuth`.
- `apps/mobile/app/convex/useConvexAuthFromContext.ts` fetches short-lived Convex token from Laravel.
- `apps/mobile/app/services/runtimeCodexApi.ts` calls `/api/convex/token` via `mintConvexToken`.

But Codex product behavior in mobile is runtime API-driven:

- `apps/mobile/app/screens/CodexWorkersScreen.tsx` uses runtime worker list/snapshot/stream/request/stop endpoints, not Convex query/mutation hooks.

Net: mobile currently carries Convex client dependency mostly for auth/session sync posture, while Codex admin data flow is runtime API + SSE style.

## 4) Desktop app (`apps/desktop`) - minimal Convex dependency, likely stale docs

Desktop still has Convex config/reachability checks:

- config: `apps/desktop/src/effect/config.ts`
- connectivity probe: `apps/desktop/src/effect/connectivity.ts`
- preload env exposure: `apps/desktop/src/preload.ts`

But task orchestration now uses Laravel/OpenAgents APIs:

- `apps/desktop/src/effect/taskProvider.ts` calls `/api/lightning/*` endpoints on `openAgentsBaseUrl`.
- `apps/desktop/src/effect/executorLoop.ts` uses that provider, not Convex APIs.

Doc drift identified:

- `apps/desktop/README.md` still claims "Convex is the command/result bus".
- Current code indicates this is no longer fully true.

## 5) Lightning ops (`apps/lightning-ops`) - direct, active Convex control-plane dependency

This is the strongest direct Convex dependency in the repo.

- Convex transport:
  - `apps/lightning-ops/src/controlPlane/convexTransport.ts` uses `ConvexHttpClient` from `convex/browser`.
- Convex function paths (query/mutations):
  - `apps/lightning-ops/src/controlPlane/convex.ts`
  - e.g. `lightning/ops:listPaywallControlPlaneState`, `lightning/security:*`, `lightning/settlements:*`
- Required env:
  - `OA_LIGHTNING_OPS_CONVEX_URL`
  - `OA_LIGHTNING_OPS_SECRET`
  - in `apps/lightning-ops/src/runtime/config.ts`
- Scope described in README:
  - `apps/lightning-ops/README.md`

Net: this lane is not merely "projection sync" today. It is operational control-plane state and writes.

## 6) iOS app (`apps/autopilot-ios`) - no direct Convex SDK usage found

- iOS models include `convex_projection` fields in runtime worker summaries:
  - `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexModels.swift`
- No direct Convex client integration detected in current app code.

## Summary Table

| Surface | Current Convex role | Risk if removed now |
|---|---|---|
| Runtime | Projection writer target (non-authoritative by design) | Medium |
| Laravel | Token mint bridge + legacy import utilities | Low-Medium |
| Mobile | Convex client/auth bootstrap; Codex data via runtime APIs | Low-Medium |
| Desktop | Config + connectivity probe; core task flow uses Laravel APIs | Low |
| Lightning ops | Active control-plane reads/writes via Convex functions | High |
| iOS | Projection fields only (from runtime APIs) | Low |

## What this means for "replace Convex with our own sync engine"

## Feasibility

Technically feasible. For runtime/Codex projection use cases, OpenAgents already has most foundational pieces:

- canonical event log in Postgres,
- deterministic projector,
- replay/rebuild semantics,
- checkpointing and drift detection,
- proto-first contract governance in ADR-0028.

Main complexity is not data storage. It is live subscription semantics and robust client sync protocol across reconnects.

## Critical constraint

Do not attempt to reimplement Convex wire compatibility while keeping Convex clients unchanged.

Why:

- Convex JS sync client has non-trivial protocol and state management in:
  - `convex-js/src/browser/sync/protocol.ts`
  - `convex-js/src/browser/sync/client.ts`
  - `convex-js/src/browser/sync/request_manager.ts`
  - `convex-js/src/browser/sync/web_socket_manager.ts`
- Reproducing this behavior exactly is high-risk and unnecessary if you own clients.

Prefer a clean OpenAgents protocol and client SDK generated from proto contracts.

## Proto-First Schema Strategy (No manual TypeScript schema authority)

OpenAgents already has the right governance foundation:

- `docs/adr/ADR-0028-layer0-proto-canonical-schema.md`
- `proto/README.md`
- `docs/protocol/LAYER0_PROTOBUF_MAPPING.md`

Recommended additions:

1. Add sync/read-model protos under `proto/openagents/protocol/v1/` (or `sync/v1/` if you want explicit versioning isolation).
2. Define proto messages for:
   - projection documents (`RunSummaryProjection`, `CodexWorkerSummaryProjection`)
   - subscription requests (`Subscribe`, `Unsubscribe`)
   - server push envelopes (`ProjectionUpdate`, `Heartbeat`, `SyncWatermark`)
   - resumable cursor/watermark fields
3. Generate language bindings for TS, Swift, PHP, Elixir from proto.
4. Keep JSON over WS/SSE initially, but enforce proto-compatible shape adapters (same pattern already used in runtime contracts).

Result:

- Schema authority is proto.
- TypeScript is generated or adapter-only, not authoritative.

## "Adapt code from Convex" guidance

## What is reasonable to adapt

You can reuse ideas and selected implementations from Apache-licensed Convex client packages (e.g. `convex-js`), with attribution and license compliance.

High-value patterns to adapt:

- reconnect/backoff/jitter strategy,
- transition chunking and assembly,
- inflight request lifecycle and idempotent mutation completion semantics,
- auth refresh timing behavior.

## What to avoid copying directly

Be careful with licensing boundaries:

- `convex-js` and many npm packages are Apache-2.0.
- `convex-backend` top-level license is FSL-1.1-Apache-2.0 future license (`convex-backend/LICENSE.md`), with "Competing Use" restrictions.

Practical recommendation:

- treat backend internals as design reference, not copy target,
- if you need direct code reuse beyond Apache-licensed subpaths, do legal review first.

## Options Analysis

## Option A: Stay on Convex, tighten proto governance

Description:

- Keep existing Convex architecture.
- Ensure all projection and control-plane payloads are proto-defined.
- Generate TS adapters/validators from proto where needed.

Pros:

- Lowest delivery risk.
- Fastest path to stability.
- Keeps mature subscriptions infra.

Cons:

- Keeps external Convex runtime dependency.
- "No TypeScript schema" goal is partially met only if generated TS remains required for Convex function layer.

Best when:

- Priority is shipping speed and operational simplicity over platform independence.

## Option B: Hybrid migration (recommended)

Description:

- Build OpenAgents Sync for runtime/Codex projections first.
- Keep Lightning ops on Convex temporarily.
- Migrate Lightning ops after OpenAgents Sync is proven.

Pros:

- Achieves proto-first, Postgres-backed sync goals incrementally.
- De-risks hardest migration lane (Lightning) by sequencing it later.
- Allows rollback and parallel validation.

Cons:

- Temporary dual-sync complexity.
- Needs explicit ownership of two systems during transition.

Best when:

- You want platform control without taking a single high-risk cutover.

## Option C: Full immediate Convex replacement

Description:

- Replace all Convex usage (runtime projection + Lightning ops) in one program.

Pros:

- Fastest theoretical path to zero Convex dependency.

Cons:

- Highest operational and delivery risk.
- Large scope blast radius across payments/security control plane and multi-client runtime sync.

Best when:

- Only if there is an urgent hard blocker requiring immediate vendor exit.

## Recommended Plan (Hybrid)

## Phase 1: Build OpenAgents Sync for runtime/Codex

1. Define proto contracts for projection docs and WS subscription envelopes.
2. Implement sync service on runtime stack (Phoenix Channels/WebSockets) over Postgres-backed read models.
3. Reuse existing projector/checkpoint logic; publish to Postgres read tables and broadcast updates.
4. Ship client adapters for web/mobile/iOS/desktop consuming new WS/SSE endpoints.
5. Run dual-publish (Convex + OA Sync) for a period; compare lag/drift metrics.

Verification gates:

- deterministic replay parity with existing projector tests,
- reconnect/resume correctness tests,
- p95 update latency and drop-rate SLOs,
- production shadow reads across clients.

## Phase 2: Migrate Lightning ops off Convex

1. Move paywall/security/settlement state authority to Postgres-owned control-plane tables.
2. Expose signed runtime/internal APIs for lightning-ops reads/writes.
3. Replace `ConvexHttpClient` transport in lightning-ops with OA API transport.
4. Keep convex path as rollback toggle until parity is proven.

Verification gates:

- no policy regression in `smoke:security` / `smoke:settlement` / full-flow checks,
- deterministic `configHash` parity,
- rollback drills with old/new backends.

## Phase 3: Decommission Convex per lane

1. Remove Convex token mint bridge once no client requires Convex auth.
2. Archive import/export-only tooling if no longer needed.
3. Remove Convex envs and deploy runbooks once fully unused.

## Additional Findings and Gaps

1. Runtime sink config appears defaulted to no-op in-repo, with deployment switching not fully visible in `runtime.exs`.
2. Convex mutation function definitions for runtime summaries are not in this repo.
3. Desktop README appears stale relative to current code path.
4. Lightning ops contracts are currently TypeScript Effect schemas (`apps/lightning-ops/src/contracts.ts`), not proto-governed yet.

## Final Recommendation

Proceed with Option B (hybrid migration).

- Short term: keep Convex where it is currently critical (Lightning ops), and continue using it as optional projection transport for runtime while OA Sync is built.
- Medium term: make proto the sole schema authority for sync/control-plane contracts and transition clients to OA Sync WebSockets on Postgres read models.
- Long term: retire Convex once Lightning ops and all client sync lanes have parity evidence and rollback confidence.

This path satisfies the core goal (proto-first, Postgres-backed sync engine, no manual TS schema authority) with the lowest realistic risk.
