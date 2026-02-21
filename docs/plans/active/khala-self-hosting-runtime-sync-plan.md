# Khala Self-Hosting + Runtime Sync Plan

Date: 2026-02-19  
Status: Active  
Owner: Runtime + Web + Mobile + Desktop

Execution companion:
- `docs/plans/active/khala-runtime-codex-master-roadmap.md`

## Purpose

Define how OpenAgents should use self-hosted Khala without weakening runtime
correctness guarantees:

1. Keep `apps/runtime` + Postgres as kernel source-of-truth.
2. Use Khala as a reactive sync/read-model layer for web/mobile/desktop.
3. Make Codex admin/observability surfaces consistent across clients.

## Inputs Reviewed

This plan is based on direct review of local Khala repos under `~/code/khala`,
especially:

- `README.md` (repo index + upstream mapping)
- `khala-backend/self-hosted/README.md`
- `khala-backend/self-hosted/docker/docker-compose.yml`
- `khala-backend/self-hosted/advanced/*` (own infra, Postgres/MySQL, S3, upgrades)
- `khala-backend/npm-packages/docs/docs/cli/background-agents.mdx`
- `khala-backend/npm-packages/docs/docs/ai/khala-mcp-server.mdx`
- `khala-backend/npm-packages/docs/docs/auth/advanced/custom-jwt.mdx`
- `khala-js/CHANGELOG.md` (self-hosted + MCP + agent mode behavior)

And OpenAgents context from:

- `docs/local/convo.md` (Khala placement, single-writer, auth flow)
- `docs/codex/unified-runtime-desktop-plan.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`

## Decisions (Locked)

1. **Kernel truth stays in runtime Postgres**
   - Khala is not the authority for run events, spend, policy, leases, or
     settlement state.
2. **Khala is a projection/sync layer**
   - Khala stores reactive UI read models derived from runtime events.
3. **Runtime is the single writer into Khala**
   - Laravel and clients do not co-write the same Khala documents.
4. **Laravel remains identity/session authority**
   - WorkOS auth terminates in Laravel; Laravel mints Khala auth JWTs.
   - We do not depend on Khala Auth CLI flows for self-hosted deployments.
5. **Self-hosted Khala runs in OpenAgents GCP perimeter**
   - Backend + dashboard are self-hosted using Khala-supported containers.
6. **Codex cloud agents use constrained Khala access**
   - Default to `KHALA_AGENT_MODE=anonymous` for remote coding agents.
   - MCP production access is disabled by default; only explicitly enabled when
     needed.

## Why This Shape

From Khala docs + self-hosting docs:

- Self-hosting is real and supported (backend + dashboard + CLI compatibility).
- Production guidance favors SQL backends (Postgres/MySQL) and same-region
  placement.
- Khala remains excellent at subscriptions and live UI sync.

From OpenAgents runtime requirements:

- Runtime correctness requires durable event-log + policy/spend invariants that
  must not be split across multiple mutable sources.

This gives a clean split:

- Runtime/Postgres: correctness.
- Khala: reactivity.

## Target Architecture

### Source-of-truth plane

- `apps/runtime` writes canonical run/codex events to Postgres.
- Laravel reads runtime-owned state via internal APIs and read models.

### Sync plane

- Runtime projector publishes selected read models into self-hosted Khala.
- Clients subscribe to Khala for live list/status/progress surfaces.
- Runtime SSE remains the high-fidelity stream; Khala is catch-up/sync state.

### Control/auth plane

- WorkOS -> Laravel session/token.
- Laravel endpoint (for example `/api/khala/token`) mints short-lived JWTs for
  Khala custom JWT provider.
- Khala trusts OpenAgents issuer/JWKS, not WorkOS directly.

## GCP Deployment Shape (Self-Hosted Khala)

## Components

1. `khala-backend` container (`ghcr.io/get-khala/khala-backend:<rev>`)
2. `khala-dashboard` container (`ghcr.io/get-khala/khala-dashboard:<rev>`)
3. Cloud SQL Auth Proxy sidecar in backend service
4. SQL backend (Cloud SQL Postgres)
5. Persistent storage:
   - phase 1: attached persistent disk
   - phase 2: S3-compatible object store path for exports/files/modules/search
     if required

## Required baseline envs

- `KHALA_CLOUD_ORIGIN`
- `KHALA_SITE_ORIGIN`
- `NEXT_PUBLIC_DEPLOYMENT_URL`
- `POSTGRES_URL` (cluster URL, no db name, points at sidecar `localhost:5432`)
- optional hardening flags: `REDACT_LOGS_TO_CLIENT=true`, `DISABLE_BEACON=true`

## Operational requirements

- Pin backend/dashboard versions together.
- Keep Khala backend and SQL in same GCP region.
- Backup via `npx khala export`; prefer export before upgrade.
- Deploy scripts/runbook for OpenAgents non-prod:
  - `apps/runtime/deploy/khala/provision-nonprod-gcp.sh`
  - `apps/runtime/deploy/khala/check-nonprod-health.sh`
  - `apps/runtime/deploy/khala/README.md`
  - `apps/runtime/deploy/khala/OPERATIONS_RUNBOOK.md`

## Data Class Mapping

Kernel-only (runtime Postgres):

- run event log, stream cursors, run leases
- policy/spend authorizations and reservations
- settlement-affecting receipts and replay artifacts
- Codex worker authoritative lifecycle/events

Khala projections (reactive sync only):

- thread/run summary cards
- codex worker summary + live status badges
- notification inbox, presence-like UI state
- lightweight timeline mirrors derived from runtime event slices

## Projection Rules

1. Runtime projector is authoritative writer for Khala projection docs.
2. Projection payloads are deterministic transformations of runtime events.
3. Projection writes include runtime sequence references for traceability.
4. Rebuild is supported from runtime event log (drop + replay projector output).

## Codex Integration Rules

For Codex admin surfaces:

- Runtime remains authoritative for worker lifecycle.
- Khala mirrors worker summaries for low-latency multi-client sync.
- Web/mobile/desktop subscribe to Khala and fetch full detail/controls from
  Laravel/runtime APIs when needed.

For Codex coding agents working against Khala projects:

1. Default remote-agent setup:
   - `KHALA_AGENT_MODE=anonymous npx khala dev --once`
2. MCP defaults:
   - do not enable production access unless explicitly required
   - prefer disabling mutation-heavy tools unless task needs them
3. Self-hosted MCP usage should pass deployment via env file
   (`KHALA_SELF_HOSTED_URL`, `KHALA_SELF_HOSTED_ADMIN_KEY`).

## Security and Auth

1. Laravel-issued Khala JWTs are short-lived and scoped to OpenAgents user.
   - Endpoint: `POST /api/khala/token` (auth: `auth:sanctum`).
   - Required claims:
     - `iss` (config: `OA_KHALA_TOKEN_ISSUER`)
     - `aud` (config: `OA_KHALA_TOKEN_AUDIENCE`)
     - `sub` (`<subject_prefix>:<oa_user_id>`)
     - `iat`, `nbf`, `exp` (TTL via `OA_KHALA_TOKEN_TTL_SECONDS`)
2. Khala auth provider uses custom JWT/OIDC config with explicit issuer/audience.
3. Admin keys are operator secrets only; never issued to end-user clients.
4. No kernel authority checks rely on Khala-only state.
5. Self-hosted auth path avoids Khala Auth CLI coupling and uses OpenAgents
   issuer/JWKS as the identity bridge.

### Khala Claim Schema (`oa_khala_claims_v1`)

Laravel token bridge currently emits:

- `iss`: OpenAgents issuer (`OA_KHALA_TOKEN_ISSUER`)
- `aud`: Khala audience (`OA_KHALA_TOKEN_AUDIENCE`)
- `sub`: `<subject_prefix>:<oa_user_id>`
- `iat` / `nbf` / `exp`: short-lived validity window
- `jti`: per-token unique id
- `oa_user_id`: OpenAgents user id
- `oa_claims_version`: `oa_khala_claims_v1`
- optional `scope`: requested scope list
- optional `oa_workspace_id`: selected workspace context
- optional `oa_role`: one of `member|admin|owner`

TTL policy (server-enforced):

- target TTL: `OA_KHALA_TOKEN_TTL_SECONDS`
- minimum allowed: `OA_KHALA_TOKEN_MIN_TTL_SECONDS`
- maximum allowed: `OA_KHALA_TOKEN_MAX_TTL_SECONDS`

### Client Refresh Flow (Web/Mobile/Desktop)

1. Client calls `POST /api/khala/token` using current OA session/token.
2. Laravel returns short-lived Khala JWT.
3. Client initializes Khala with that JWT.
4. On expiry/reconnect, client requests a fresh token from Laravel.
5. If OA auth is expired, Laravel returns `401`; client must refresh/re-auth OA
   first, then request Khala token again.

## Phase Plan

### Phase 0: Environment and topology validation

- Stand up self-hosted Khala in non-prod GCP.
- Validate dashboard/admin key flow, deploy flow, and export/import flow.

Verification:

- `npx khala dev` with `KHALA_SELF_HOSTED_URL` + admin key
- dashboard login + table/function visibility
- `npx khala export` / `npx khala import` dry run in non-prod

### Phase 1: Runtime projection writer (minimal)

- Implement runtime-side projector for a small set of run/codex summaries.
- Add replayable sequence linkage in projection docs.

Verification:

- projector integration tests (deterministic projection from fixed runtime events)
- rebuild test: wipe projection tables/docs and rehydrate from runtime history

### Phase 2: Laravel token mint + client subscription wiring

- Add Laravel endpoint to mint Khala JWT from OA session.
- Web/mobile clients subscribe with Khala token and continue runtime API usage
  for privileged actions.

Verification:

- token mint auth tests
- subscription update tests for run/codex summary changes

### Phase 3: Codex surfaces

- Mirror codex worker summary projections.
- Keep detailed stream/control operations on runtime endpoints.
- Web admin now includes runtime-backed worker list/detail/actions plus stream
  event observability (`apps/openagents.com/resources/js/pages/admin/index.tsx`).

Verification:

- worker create/request/stop reflected in Khala summaries
- no divergence between runtime snapshot and Khala summary after replay

## Non-Goals

- Replacing runtime SSE with Khala for canonical execution logs.
- Moving spend/policy/settlement truth into Khala.
- Letting multiple services write overlapping Khala projection docs.

## References

- `docs/local/convo.md`
- `docs/codex/unified-runtime-desktop-plan.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `https://github.com/get-khala/khala-backend/tree/main/self-hosted`
- `https://docs.khala.dev/cli/background-agents`
- `https://docs.khala.dev/ai/khala-mcp-server`
