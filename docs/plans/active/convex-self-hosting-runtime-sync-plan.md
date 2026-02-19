# Convex Self-Hosting + Runtime Sync Plan

Date: 2026-02-19  
Status: Active  
Owner: Runtime + Web + Mobile + Desktop

## Purpose

Define how OpenAgents should use self-hosted Convex without weakening runtime
correctness guarantees:

1. Keep `apps/openagents-runtime` + Postgres as kernel source-of-truth.
2. Use Convex as a reactive sync/read-model layer for web/mobile/desktop.
3. Make Codex admin/observability surfaces consistent across clients.

## Inputs Reviewed

This plan is based on direct review of local Convex repos under `~/code/convex`,
especially:

- `README.md` (repo index + upstream mapping)
- `convex-backend/self-hosted/README.md`
- `convex-backend/self-hosted/docker/docker-compose.yml`
- `convex-backend/self-hosted/advanced/*` (own infra, Postgres/MySQL, S3, upgrades)
- `convex-backend/npm-packages/docs/docs/cli/background-agents.mdx`
- `convex-backend/npm-packages/docs/docs/ai/convex-mcp-server.mdx`
- `convex-backend/npm-packages/docs/docs/auth/advanced/custom-jwt.mdx`
- `convex-js/CHANGELOG.md` (self-hosted + MCP + agent mode behavior)

And OpenAgents context from:

- `docs/local/convo.md` (Convex placement, single-writer, auth flow)
- `docs/codex/unified-runtime-desktop-plan.md`
- `apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`

## Decisions (Locked)

1. **Kernel truth stays in runtime Postgres**
   - Convex is not the authority for run events, spend, policy, leases, or
     settlement state.
2. **Convex is a projection/sync layer**
   - Convex stores reactive UI read models derived from runtime events.
3. **Runtime is the single writer into Convex**
   - Laravel and clients do not co-write the same Convex documents.
4. **Laravel remains identity/session authority**
   - WorkOS auth terminates in Laravel; Laravel mints Convex auth JWTs.
   - We do not depend on Convex Auth CLI flows for self-hosted deployments.
5. **Self-hosted Convex runs in OpenAgents GCP perimeter**
   - Backend + dashboard are self-hosted using Convex-supported containers.
6. **Codex cloud agents use constrained Convex access**
   - Default to `CONVEX_AGENT_MODE=anonymous` for remote coding agents.
   - MCP production access is disabled by default; only explicitly enabled when
     needed.

## Why This Shape

From Convex docs + self-hosting docs:

- Self-hosting is real and supported (backend + dashboard + CLI compatibility).
- Production guidance favors SQL backends (Postgres/MySQL) and same-region
  placement.
- Convex remains excellent at subscriptions and live UI sync.

From OpenAgents runtime requirements:

- Runtime correctness requires durable event-log + policy/spend invariants that
  must not be split across multiple mutable sources.

This gives a clean split:

- Runtime/Postgres: correctness.
- Convex: reactivity.

## Target Architecture

### Source-of-truth plane

- `apps/openagents-runtime` writes canonical run/codex events to Postgres.
- Laravel reads runtime-owned state via internal APIs and read models.

### Sync plane

- Runtime projector publishes selected read models into self-hosted Convex.
- Clients subscribe to Convex for live list/status/progress surfaces.
- Runtime SSE remains the high-fidelity stream; Convex is catch-up/sync state.

### Control/auth plane

- WorkOS -> Laravel session/token.
- Laravel endpoint (for example `/api/convex/token`) mints short-lived JWTs for
  Convex custom JWT provider.
- Convex trusts OpenAgents issuer/JWKS, not WorkOS directly.

## GCP Deployment Shape (Self-Hosted Convex)

## Components

1. `convex-backend` container (`ghcr.io/get-convex/convex-backend:<rev>`)
2. `convex-dashboard` container (`ghcr.io/get-convex/convex-dashboard:<rev>`)
3. SQL backend (Cloud SQL Postgres)
4. Persistent storage:
   - phase 1: attached persistent disk
   - phase 2: S3-compatible object store path for exports/files/modules/search
     if required

## Required baseline envs

- `CONVEX_CLOUD_ORIGIN`
- `CONVEX_SITE_ORIGIN`
- `NEXT_PUBLIC_DEPLOYMENT_URL`
- `POSTGRES_URL` (preferred for production)
- optional hardening flags: `REDACT_LOGS_TO_CLIENT=true`, `DISABLE_BEACON=true`

## Operational requirements

- Pin backend/dashboard versions together.
- Keep Convex backend and SQL in same GCP region.
- Backup via `npx convex export`; prefer export before upgrade.

## Data Class Mapping

Kernel-only (runtime Postgres):

- run event log, stream cursors, run leases
- policy/spend authorizations and reservations
- settlement-affecting receipts and replay artifacts
- Codex worker authoritative lifecycle/events

Convex projections (reactive sync only):

- thread/run summary cards
- codex worker summary + live status badges
- notification inbox, presence-like UI state
- lightweight timeline mirrors derived from runtime event slices

## Projection Rules

1. Runtime projector is authoritative writer for Convex projection docs.
2. Projection payloads are deterministic transformations of runtime events.
3. Projection writes include runtime sequence references for traceability.
4. Rebuild is supported from runtime event log (drop + replay projector output).

## Codex Integration Rules

For Codex admin surfaces:

- Runtime remains authoritative for worker lifecycle.
- Convex mirrors worker summaries for low-latency multi-client sync.
- Web/mobile/desktop subscribe to Convex and fetch full detail/controls from
  Laravel/runtime APIs when needed.

For Codex coding agents working against Convex projects:

1. Default remote-agent setup:
   - `CONVEX_AGENT_MODE=anonymous npx convex dev --once`
2. MCP defaults:
   - do not enable production access unless explicitly required
   - prefer disabling mutation-heavy tools unless task needs them
3. Self-hosted MCP usage should pass deployment via env file
   (`CONVEX_SELF_HOSTED_URL`, `CONVEX_SELF_HOSTED_ADMIN_KEY`).

## Security and Auth

1. Laravel-issued Convex JWTs are short-lived and scoped to OpenAgents user.
2. Convex auth provider uses custom JWT/OIDC config with explicit issuer/audience.
3. Admin keys are operator secrets only; never issued to end-user clients.
4. No kernel authority checks rely on Convex-only state.
5. Self-hosted auth path avoids Convex Auth CLI coupling and uses OpenAgents
   issuer/JWKS as the identity bridge.

## Phase Plan

### Phase 0: Environment and topology validation

- Stand up self-hosted Convex in non-prod GCP.
- Validate dashboard/admin key flow, deploy flow, and export/import flow.

Verification:

- `npx convex dev` with `CONVEX_SELF_HOSTED_URL` + admin key
- dashboard login + table/function visibility
- `npx convex export` / `npx convex import` dry run in non-prod

### Phase 1: Runtime projection writer (minimal)

- Implement runtime-side projector for a small set of run/codex summaries.
- Add replayable sequence linkage in projection docs.

Verification:

- projector integration tests (deterministic projection from fixed runtime events)
- rebuild test: wipe projection tables/docs and rehydrate from runtime history

### Phase 2: Laravel token mint + client subscription wiring

- Add Laravel endpoint to mint Convex JWT from OA session.
- Web/mobile clients subscribe with Convex token and continue runtime API usage
  for privileged actions.

Verification:

- token mint auth tests
- subscription update tests for run/codex summary changes

### Phase 3: Codex surfaces

- Mirror codex worker summary projections.
- Keep detailed stream/control operations on runtime endpoints.

Verification:

- worker create/request/stop reflected in Convex summaries
- no divergence between runtime snapshot and Convex summary after replay

## Non-Goals

- Replacing runtime SSE with Convex for canonical execution logs.
- Moving spend/policy/settlement truth into Convex.
- Letting multiple services write overlapping Convex projection docs.

## References

- `docs/local/convo.md`
- `docs/codex/unified-runtime-desktop-plan.md`
- `apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`
- `https://github.com/get-convex/convex-backend/tree/main/self-hosted`
- `https://docs.convex.dev/cli/background-agents`
- `https://docs.convex.dev/ai/convex-mcp-server`
