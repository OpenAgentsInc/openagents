# OA-WEBPARITY-001 Program Charter and Canonical Checklist

Date: 2026-02-22  
Status: Active (frozen baseline for parity execution)  
Tracking: OA-WEBPARITY-001

Canonical source plan:
- `apps/openagents.com/docs/20260222-laravel-rust-wgpui-full-parity-master-plan.md`

Baseline manifest artifacts:
- `apps/openagents.com/docs/parity-manifests/baseline/api-routes.json`
- `apps/openagents.com/docs/parity-manifests/baseline/web-routes.json`
- `apps/openagents.com/docs/parity-manifests/baseline/page-entries.json`
- `apps/openagents.com/docs/parity-manifests/baseline/artisan-commands.json`
- `apps/openagents.com/docs/parity-manifests/baseline/manifest-index.json`
- Regenerate with: `php artisan ops:export-parity-manifests --output=docs/parity-manifests/baseline`

## Charter

1. Port or explicitly retire all active Laravel product behavior for `apps/openagents.com` into Rust service + Rust/WGPUI web shell.
2. Preserve externally visible contracts unless retirement behavior is explicitly approved and documented.
3. Use Khala WebSocket replay/live as the only streaming transport for live delivery.
4. Keep Codex app-server protocol as the only production chat/thread authority path.
5. Complete cutover with Rust-only serving and archive legacy Laravel lanes.

## Ownership Matrix

| Lane | Owner of Record | Primary Code Paths | Evidence/Signoff |
| --- | --- | --- | --- |
| Program governance and checklist health | openagents.com platform | `apps/openagents.com/docs/` | parity issue closure notes + final audit |
| API/middleware/persistence parity | Rust service lane | `apps/openagents.com/service/`, `crates/` | contract harness, API parity tests |
| Web route/UI parity | Rust web-shell lane | `apps/openagents.com/web-shell/` | UI parity tests, route-shell tests |
| Runtime/Codex/Khala integration parity | Runtime integration lane | `apps/runtime/`, `docs/protocol/`, Rust service adapters | stream/replay conformance evidence |
| Migration/cutover safety | Platform ops lane | deploy scripts/runbooks in `docs/` + `scripts/` | canary + rollback drill reports |

## Pass/Fail Criteria

All items below must be true for overall parity to pass:

1. API route manifest coverage is fully accounted for (ported or approved retirement behavior).
2. Web routes/pages are Rust-owned with deep-link, redirect, and error-state parity.
3. Cookie/session/CORS/cache/throttle semantics match baseline behavior.
4. Khala WS replay/live ordering and resume semantics match captured contracts.
5. Operator command replacements exist for required Artisan workflows.
6. Data migration/backfill and mixed-version rollback safety checks are validated.
7. Static asset hashing/cache/compression/service-worker behavior matches target policy.
8. Staging dual-run + production canary + rollback drills are completed and documented.
9. No production Vercel-protocol chat authority remains.
10. Active product implementation lanes are Rust-only at terminal gate.

## Canonical Route Checklist

### API Routes (manifest coverage target: 77)

Auth and identity:
- [ ] `POST /api/auth/register`
- [ ] `POST /api/auth/email`
- [ ] `POST /api/auth/verify`
- [ ] `GET /api/me`
- [ ] `GET /api/tokens`
- [ ] `POST /api/tokens`
- [ ] `DELETE /api/tokens/current`
- [ ] `DELETE /api/tokens/{tokenId}`
- [ ] `DELETE /api/tokens`
- [ ] `POST /api/khala/token`
- [ ] `POST /api/sync/token`
- [ ] `GET /api/settings/profile`
- [ ] `PATCH /api/settings/profile`
- [ ] `DELETE /api/settings/profile`

Chat consolidation (legacy migration scope):
- [ ] `GET /api/chat/guest-session`
- [ ] `POST /api/chat`
- [ ] `POST /api/chat/stream`
- [ ] `GET /api/chats`
- [ ] `POST /api/chats`
- [ ] `GET /api/chats/{conversationId}`
- [ ] `GET /api/chats/{conversationId}/messages`
- [ ] `GET /api/chats/{conversationId}/runs`
- [ ] `GET /api/chats/{conversationId}/runs/{runId}/events`
- [ ] `POST /api/chats/{conversationId}/stream`

Runtime tools, skills, and Codex workers:
- [ ] `POST /api/runtime/tools/execute`
- [ ] `GET /api/runtime/skills/tool-specs`
- [ ] `POST /api/runtime/skills/tool-specs`
- [ ] `GET /api/runtime/skills/skill-specs`
- [ ] `POST /api/runtime/skills/skill-specs`
- [ ] `POST /api/runtime/skills/skill-specs/{skillId}/{version}/publish`
- [ ] `GET /api/runtime/skills/releases/{skillId}/{version}`
- [ ] `GET /api/runtime/codex/workers`
- [ ] `POST /api/runtime/codex/workers`
- [ ] `GET /api/runtime/codex/workers/{workerId}`
- [ ] `GET /api/runtime/codex/workers/{workerId}/stream`
- [ ] `POST /api/runtime/codex/workers/{workerId}/requests`
- [ ] `POST /api/runtime/codex/workers/{workerId}/events`
- [ ] `POST /api/runtime/codex/workers/{workerId}/stop`
- [ ] `POST /api/internal/runtime/integrations/secrets/fetch`

Autopilot:
- [ ] `GET /api/autopilots`
- [ ] `POST /api/autopilots`
- [ ] `GET /api/autopilots/{autopilot}`
- [ ] `PATCH /api/autopilots/{autopilot}`
- [ ] `GET /api/autopilots/{autopilot}/threads`
- [ ] `POST /api/autopilots/{autopilot}/threads`
- [ ] `POST /api/autopilots/{autopilot}/stream`

Social and messaging:
- [ ] `GET /api/shouts`
- [ ] `GET /api/shouts/zones`
- [ ] `POST /api/shouts`
- [ ] `GET /api/whispers`
- [ ] `POST /api/whispers`
- [ ] `PATCH /api/whispers/{id}/read`

L402 and payments:
- [ ] `GET /api/l402/wallet`
- [ ] `GET /api/l402/transactions`
- [ ] `GET /api/l402/transactions/{eventId}`
- [ ] `GET /api/l402/paywalls`
- [ ] `POST /api/l402/paywalls`
- [ ] `PATCH /api/l402/paywalls/{paywallId}`
- [ ] `DELETE /api/l402/paywalls/{paywallId}`
- [ ] `GET /api/l402/settlements`
- [ ] `GET /api/l402/deployments`
- [ ] `GET /api/agent-payments/wallet`
- [ ] `POST /api/agent-payments/wallet`
- [ ] `GET /api/agent-payments/balance`
- [ ] `POST /api/agent-payments/invoice`
- [ ] `POST /api/agent-payments/pay`
- [ ] `POST /api/agent-payments/send-spark`
- [ ] `GET /api/agents/me/wallet`
- [ ] `POST /api/agents/me/wallet`
- [ ] `GET /api/agents/me/balance`
- [ ] `POST /api/payments/invoice`
- [ ] `POST /api/payments/pay`
- [ ] `POST /api/payments/send-spark`
- [ ] `POST /api/internal/lightning-ops/control-plane/query`
- [ ] `POST /api/internal/lightning-ops/control-plane/mutation`

Webhooks and utility:
- [ ] `POST /api/webhooks/resend`
- [ ] `GET /api/smoke/stream`

### Web Routes and Pages

- [ ] `GET /`
- [ ] `GET /feed`
- [ ] `GET /openapi.json`
- [ ] `GET /login`
- [ ] `POST /login/email`
- [ ] `POST /login/verify`
- [ ] `POST /logout`
- [ ] `GET /internal/test-login` (local/testing)
- [ ] `GET /settings/profile`
- [ ] `GET /settings/autopilot`
- [ ] `GET /settings/integrations`
- [ ] `PATCH /settings/profile`
- [ ] `PATCH /settings/autopilot`
- [ ] `POST /settings/integrations/resend`
- [ ] `DELETE /settings/integrations/resend`
- [ ] `POST /settings/integrations/resend/test`
- [ ] `GET /settings/integrations/google/redirect`
- [ ] `GET /settings/integrations/google/callback`
- [ ] `DELETE /settings/integrations/google`
- [ ] `GET /l402`
- [ ] `GET /l402/transactions`
- [ ] `GET /l402/transactions/{eventId}`
- [ ] `GET /l402/paywalls`
- [ ] `GET /l402/settlements`
- [ ] `GET /l402/deployments`
- [ ] `GET /admin`
- [ ] Retire `/aui` (deleted and not recreated)

## Operator Command Checklist

Artisan command replacements required before cutover:

- [x] `demo:l402` -> Rust CLI replacement
- [x] `khala:import-chat` -> Rust CLI replacement
- [x] `ops:test-login-link` -> Rust CLI replacement
- [x] `runtime:tools:invoke-api` -> Rust CLI replacement
- [x] `ops:create-api-token` -> Rust CLI replacement

## Test Target Checklist

Baseline and migration evidence targets:

- [ ] API route manifest coverage test remains authoritative (`tests/Feature/Api/V1/ApiRouteCoverageManifestTest.php`)
- [ ] Contract capture harness evidence for JSON + Khala WS (`OA-WEBPARITY-003`)
  Capture command: `./apps/openagents.com/scripts/capture-parity-contract-fixtures.sh`
  Artifact path: `apps/openagents.com/docs/parity-fixtures/baseline/`
- [ ] Parity CI scoreboard and required checks (`OA-WEBPARITY-004`)
  Scoreboard command: `./apps/openagents.com/scripts/run-parity-scoreboard.sh`
  CI workflow check name: `web-parity-scoreboard / parity-scoreboard`
- [x] Full parity regression lane (`OA-WEBPARITY-056`)
  Regression command: `./apps/openagents.com/scripts/run-full-parity-regression.sh`
  CI workflow check name: `web-parity-regression / parity-regression`
- [ ] Rust compile baseline (`cargo check --workspace --all-targets`)
- [ ] Rust/web changed-files gate (`./scripts/local-ci.sh changed`)
- [ ] Web-shell lane (`./scripts/local-ci.sh web-shell`)
- [ ] Runtime lane (`./scripts/local-ci.sh runtime`)
- [ ] Cross-surface harness when stream/chat paths change (`./scripts/local-ci.sh cross-surface`)
- [x] Staging dual-run diff report (`OA-WEBPARITY-057`)
  Harness/runbook: `apps/openagents.com/service/scripts/run-staging-dual-run-shadow-diff.sh`, `apps/openagents.com/service/docs/STAGING_DUAL_RUN_SHADOW_DIFF.md`
- [x] Production canary + rollback drill report (`OA-WEBPARITY-058`)
  Drill runner/runbook/report: `apps/openagents.com/service/deploy/run-canary-rollback-drill.sh`, `apps/openagents.com/service/docs/CANARY_ROLLBACK_DRILL_AUTOMATION.md`, `apps/openagents.com/docs/20260222-oa-webparity-058-production-canary-rollback-drill.md`
- [x] Production Rust-only route target flip report (`OA-WEBPARITY-059`)
  Flip runner/runbook/report: `apps/openagents.com/service/scripts/run-production-rust-route-flip.sh`, `apps/openagents.com/service/docs/PRODUCTION_RUST_ROUTE_FLIP.md`, `apps/openagents.com/docs/20260222-oa-webparity-059-production-rust-route-flip.md`
- [x] Laravel serving lane retirement report (`OA-WEBPARITY-060`)
  Retirement verifier/report: `apps/openagents.com/service/scripts/verify-laravel-serving-retired.sh`, `.github/workflows/web-verify-laravel-serving-retired.yml`, `apps/openagents.com/docs/20260222-oa-webparity-060-retire-laravel-serving-path.md`

## Checklist Freeze Rules

1. This document is the canonical execution checklist for OA-WEBPARITY program scope.
2. Route/page/command additions require explicit checklist update in the same PR.
3. Item completion requires linked evidence (tests, runbook output, or production report) in the owning issue.
4. Any approved retirement must document migration behavior and client impact.
