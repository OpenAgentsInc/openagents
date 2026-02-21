# Khala Usage and Proto-First Sync Engine Audit (2026-02-20)

## Scope

This audit covers:

1. How Khala is currently used across the OpenAgents codebase.
2. What is actually active in code versus stated in docs/plans.
3. Feasibility of replacing Khala with an OpenAgents-owned sync engine on Postgres (Cloud SQL) + WebSockets.
4. How to move schema authority to proto (instead of TypeScript-defined schemas).
5. Whether to keep Khala, replace it, or run a hybrid migration.

Primary inputs reviewed:

- OpenAgents:
  - `README.md`
  - `docs/ARCHITECTURE.md`
  - `docs/adr/ADR-0029-khala-sync-layer-and-codex-agent-mode.md`
  - `docs/plans/active/khala-self-hosting-runtime-sync-plan.md`
  - `docs/plans/active/khala-runtime-codex-master-roadmap.md`
  - `apps/runtime/docs/KHALA_SYNC.md`
  - runtime, web, mobile, desktop, lightning-ops source files listed throughout this audit
- Khala upstream local clone:
  - `/Users/christopherdavid/code/khala/README.md`
  - `/Users/christopherdavid/code/khala/khala-backend/self-hosted/README.md`
  - `/Users/christopherdavid/code/khala/khala-backend/self-hosted/advanced/postgres_or_mysql.md`
  - `/Users/christopherdavid/code/khala/khala-backend/npm-packages/docs/docs/database/schemas.mdx`
  - `/Users/christopherdavid/code/khala/khala-backend/npm-packages/docs/docs/functions/validation.mdx`
  - `/Users/christopherdavid/code/khala/khala-backend/npm-packages/docs/docs/auth/advanced/custom-jwt.mdx`
  - `/Users/christopherdavid/code/khala/khala-js/src/browser/sync/*`

## Executive Summary

OpenAgents currently uses Khala in two very different ways:

1. Runtime Codex sync plane (projection-only, non-authoritative), aligned with ADR-0029.
2. Lightning ops control plane (authoritative for paywall/policy compiler inputs), still directly dependent on Khala functions.

Key conclusion:

- If the goal is proto-first contracts and no hand-authored TypeScript schema definitions, OpenAgents can achieve that.
- Full Khala replacement is feasible but should not be done as a single cutover.
- Recommended path is a hybrid migration:
  1. Keep Khala for Lightning ops short-term.
  2. Build an OpenAgents sync service for runtime/Codex first (Postgres + WebSockets + proto contracts).
  3. Migrate Lightning ops off Khala as a second phase once runtime/Codex migration is stable.

## Current Khala Usage (Code Audit)

## 1) Runtime (`apps/runtime`) - projection/sync integration

Architecture intent is explicit and consistent:

- `apps/runtime/docs/KHALA_SYNC.md` declares runtime as source-of-truth and Khala as projection-only.
- `docs/adr/ADR-0029-khala-sync-layer-and-codex-agent-mode.md` locks single-writer projection and Laravel auth bridge.

Implemented runtime pieces:

- Projector + sinks:
  - `apps/runtime/lib/openagents_runtime/khala/projector.ex`
  - `apps/runtime/lib/openagents_runtime/khala/sink.ex`
  - `apps/runtime/lib/openagents_runtime/khala/http_sink.ex`
  - `apps/runtime/lib/openagents_runtime/khala/noop_sink.ex`
- Checkpoints + replay:
  - `apps/runtime/lib/openagents_runtime/khala/projection_checkpoint.ex`
  - `apps/runtime/lib/openagents_runtime/khala/reprojection.ex`
  - `apps/runtime/lib/mix/tasks/runtime.khala.reproject.ex`
  - `apps/runtime/priv/repo/migrations/20260219101000_create_runtime_khala_projection_checkpoints.exs`
- Event append hooks trigger projection:
  - `apps/runtime/lib/openagents_runtime/runs/run_events.ex`
  - `apps/runtime/lib/openagents_runtime/codex/workers.ex`

Important implementation details:

- Projected docs are deterministic summaries keyed as:
  - `runtime/run_summary:<run_id>`
  - `runtime/codex_worker_summary:<worker_id>`
- Runtime keeps idempotent checkpoints in Postgres (`runtime.khala_projection_checkpoints`) with hash + seq + version.
- Drift and replay tooling exists, with telemetry and tests.

Operational caveat found:

- Default runtime sink is `NoopSink` in `apps/runtime/config/config.exs`.
- `HttpSink` exists, but this repo does not show runtime env wiring in `config/runtime.exs` to switch sink per environment.
- Khala mutation targets (`runtime:upsertRunSummary`, `runtime:upsertCodexWorkerSummary`) are configured in sink defaults, but matching Khala function definitions are not present in this repo.

Net: runtime side is architecturally strong for a projection system, but deployment wiring appears partially external or incomplete in-repo.

## 2) Laravel web app (`apps/openagents.com`) - auth bridge + migration tooling

Active Khala usage in Laravel:

- Token bridge endpoint:
  - `POST /api/khala/token` in `apps/openagents.com/routes/api.php`
  - controller: `apps/openagents.com/app/Http/Controllers/Api/KhalaTokenController.php`
  - issuer: `apps/openagents.com/app/Support/Khala/KhalaTokenIssuer.php`
  - config: `apps/openagents.com/config/khala.php`
  - tests: `apps/openagents.com/tests/Feature/Api/KhalaTokenApiTest.php`

Behavior:

- Laravel mints short-lived JWTs for Khala client auth.
- Current implementation signs with HS256 shared secret.
- Khala custom JWT docs show RS256/ES256 examples as the standard path for JWKS-based verification.

Legacy migration path still present:

- Khala export -> Laravel import runbook and services:
  - `apps/openagents.com/docs/KHALA_PROD_EXPORT_AND_LARAVEL_IMPORT.md`
  - `apps/openagents.com/app/Support/KhalaImport/*`
  - `apps/openagents.com/routes/console.php` (`khala:import-chat`)

## 3) Mobile app (`apps/mobile`) - Khala client bootstrapped, runtime APIs used for Codex features

Khala client/auth wiring is active:

- `apps/mobile/app/app.tsx` initializes `KhalaReactClient` + `KhalaProviderWithAuth`.
- `apps/mobile/app/khala/useKhalaAuthFromContext.ts` fetches short-lived Khala token from Laravel.
- `apps/mobile/app/services/runtimeCodexApi.ts` calls `/api/khala/token` via `mintKhalaToken`.

But Codex product behavior in mobile is runtime API-driven:

- `apps/mobile/app/screens/CodexWorkersScreen.tsx` uses runtime worker list/snapshot/stream/request/stop endpoints, not Khala query/mutation hooks.

Net: mobile currently carries Khala client dependency mostly for auth/session sync posture, while Codex admin data flow is runtime API + SSE style.

## 4) Desktop app (`apps/desktop`) - minimal Khala dependency, likely stale docs

Desktop still has Khala config/reachability checks:

- config: `apps/desktop/src/effect/config.ts`
- connectivity probe: `apps/desktop/src/effect/connectivity.ts`
- preload env exposure: `apps/desktop/src/preload.ts`

But task orchestration now uses Laravel/OpenAgents APIs:

- `apps/desktop/src/effect/taskProvider.ts` calls `/api/lightning/*` endpoints on `openAgentsBaseUrl`.
- `apps/desktop/src/effect/executorLoop.ts` uses that provider, not Khala APIs.

Doc drift identified:

- `apps/desktop/README.md` still claims "Khala is the command/result bus".
- Current code indicates this is no longer fully true.

## 5) Lightning ops (`apps/lightning-ops`) - direct, active Khala control-plane dependency

This is the strongest direct Khala dependency in the repo.

- Khala transport:
  - `apps/lightning-ops/src/controlPlane/khalaTransport.ts` uses `KhalaHttpClient` from `khala/browser`.
- Khala function paths (query/mutations):
  - `apps/lightning-ops/src/controlPlane/khala.ts`
  - e.g. `lightning/ops:listPaywallControlPlaneState`, `lightning/security:*`, `lightning/settlements:*`
- Required env:
  - `OA_LIGHTNING_OPS_KHALA_URL`
  - `OA_LIGHTNING_OPS_SECRET`
  - in `apps/lightning-ops/src/runtime/config.ts`
- Scope described in README:
  - `apps/lightning-ops/README.md`

Net: this lane is not merely "projection sync" today. It is operational control-plane state and writes.

## 6) iOS app (`apps/autopilot-ios`) - no direct Khala SDK usage found

- iOS models include `khala_projection` fields in runtime worker summaries:
  - `apps/autopilot-ios/Autopilot/Autopilot/RuntimeCodexModels.swift`
- No direct Khala client integration detected in current app code.

## Summary Table

| Surface | Current Khala role | Risk if removed now |
|---|---|---|
| Runtime | Projection writer target (non-authoritative by design) | Medium |
| Laravel | Token mint bridge + legacy import utilities | Low-Medium |
| Mobile | Khala client/auth bootstrap; Codex data via runtime APIs | Low-Medium |
| Desktop | Config + connectivity probe; core task flow uses Laravel APIs | Low |
| Lightning ops | Active control-plane reads/writes via Khala functions | High |
| iOS | Projection fields only (from runtime APIs) | Low |

## What this means for consolidating on runtime-owned Khala

## Feasibility

Technically feasible. For runtime/Codex projection use cases, OpenAgents already has most foundational pieces:

- canonical event log in Postgres,
- deterministic projector,
- replay/rebuild semantics,
- checkpointing and drift detection,
- proto-first contract governance in ADR-0028.

Main complexity is not data storage. It is live subscription semantics and robust client sync protocol across reconnects.

## Critical constraint

Do not attempt to preserve third-party wire compatibility while keeping legacy clients unchanged.

Why:

- Khala JS sync client has non-trivial protocol and state management in:
  - `khala-js/src/browser/sync/protocol.ts`
  - `khala-js/src/browser/sync/client.ts`
  - `khala-js/src/browser/sync/request_manager.ts`
  - `khala-js/src/browser/sync/web_socket_manager.ts`
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

## "Adapt code from Khala" guidance

## What is reasonable to adapt

You can reuse ideas and selected implementations from Apache-licensed Khala client packages (e.g. `khala-js`), with attribution and license compliance.

High-value patterns to adapt:

- reconnect/backoff/jitter strategy,
- transition chunking and assembly,
- inflight request lifecycle and idempotent mutation completion semantics,
- auth refresh timing behavior.

## What to avoid copying directly

Be careful with licensing boundaries:

- `khala-js` and many npm packages are Apache-2.0.
- `khala-backend` top-level license is FSL-1.1-Apache-2.0 future license (`khala-backend/LICENSE.md`), with "Competing Use" restrictions.

Practical recommendation:

- treat backend internals as design reference, not copy target,
- if you need direct code reuse beyond Apache-licensed subpaths, do legal review first.

## Options Analysis

## Option A: Stay on Khala, tighten proto governance

Description:

- Keep existing Khala architecture.
- Ensure all projection and control-plane payloads are proto-defined.
- Generate TS adapters/validators from proto where needed.

Pros:

- Lowest delivery risk.
- Fastest path to stability.
- Keeps mature subscriptions infra.

Cons:

- Keeps external Khala runtime dependency.
- "No TypeScript schema" goal is partially met only if generated TS remains required for Khala function layer.

Best when:

- Priority is shipping speed and operational simplicity over platform independence.

## Option B: Hybrid migration (recommended)

Description:

- Build OpenAgents Sync for runtime/Codex projections first.
- Keep Lightning ops on Khala temporarily.
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

## Option C: Full immediate Khala replacement

Description:

- Replace all Khala usage (runtime projection + Lightning ops) in one program.

Pros:

- Fastest theoretical path to zero Khala dependency.

Cons:

- Highest operational and delivery risk.
- Large scope blast radius across payments/security control plane and multi-client runtime sync.

Best when:

- Only if there is an urgent hard blocker requiring immediate vendor exit.

## Recommended Plan (Hybrid)

## Phase 1: Build runtime-owned Khala for runtime/Codex

1. Define proto contracts for projection docs and WS subscription envelopes.
2. Implement sync service on runtime stack (Phoenix Channels/WebSockets) over Postgres-backed read models.
3. Reuse existing projector/checkpoint logic; publish to Postgres read tables and broadcast updates.
4. Ship client adapters for web/mobile/iOS/desktop consuming new WS/SSE endpoints.
5. Run dual-path parity (legacy lane + Khala) for a period; compare lag/drift metrics.

Verification gates:

- deterministic replay parity with existing projector tests,
- reconnect/resume correctness tests,
- p95 update latency and drop-rate SLOs,
- production shadow reads across clients.

## Phase 2: Migrate Lightning ops off legacy sync dependency

1. Move paywall/security/settlement state authority to Postgres-owned control-plane tables.
2. Expose signed runtime/internal APIs for lightning-ops reads/writes.
3. Replace legacy sync transport in lightning-ops with OA API transport.
4. Keep legacy path as rollback toggle until parity is proven.

Verification gates:

- no policy regression in `smoke:security` / `smoke:settlement` / full-flow checks,
- deterministic `configHash` parity,
- rollback drills with old/new backends.

## Phase 3: Decommission legacy sync dependency per lane

1. Remove legacy sync token mint bridge once no client requires legacy auth.
2. Archive import/export-only tooling if no longer needed.
3. Remove legacy envs and deploy runbooks once fully unused.

## Additional Findings and Gaps

1. Runtime sink config appears defaulted to no-op in-repo, with deployment switching not fully visible in `runtime.exs`.
2. Khala mutation function definitions for runtime summaries are not in this repo.
3. Desktop README appears stale relative to current code path.
4. Lightning ops contracts are currently TypeScript Effect schemas (`apps/lightning-ops/src/contracts.ts`), not proto-governed yet.

## Final Recommendation

Proceed with Option B (hybrid migration).

- Short term: keep Khala where it is currently critical (Lightning ops), and continue using it as optional projection transport for runtime while runtime-owned Khala is hardened.
- Medium term: make proto the sole schema authority for sync/control-plane contracts and transition clients to Khala WebSockets on Postgres read models.
- Long term: retire legacy reactive/sync dependencies once Lightning ops and all client sync lanes have parity evidence and rollback confidence; keep runtime-owned Khala as the steady-state sync plane.

Clarification:

- Older wording that said “retire Khala” was a terminology regression from earlier drafts.
- The intended target for retirement is the legacy lane Khala replaced (the prior vendor-managed sync dependency), not Khala itself.

This path satisfies the core goal (proto-first, Postgres-backed sync engine, no manual TS schema authority) with the lowest realistic risk.
