# Convex + Runtime + Codex Master Execution Roadmap

Date: 2026-02-19  
Status: Active  
Owner: Runtime + Web + Mobile + Desktop + Infra  
Primary audience: implementation teams and autonomous coding agents

## Goal

Ship a production-ready, self-hosted Convex sync layer that is integrated with
the OpenAgents runtime and Codex surfaces while preserving kernel correctness
invariants.

This roadmap is the execution companion to architecture docs. It does not
replace authority docs.

## Authority and Scope

Architecture and invariants are defined by:

- `docs/adr/ADR-0028-layer0-proto-canonical-schema.md`
- `docs/adr/ADR-0029-convex-sync-layer-and-codex-agent-mode.md`
- `docs/codex/unified-runtime-desktop-plan.md`
- `docs/plans/active/convex-self-hosting-runtime-sync-plan.md`
- `apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`
- `apps/openagents-runtime/docs/CONVEX_SYNC.md`

This roadmap covers:

- sequencing and delivery gates,
- backlog by workstream,
- verification and release criteria,
- rollout and rollback posture.

## Hard Constraints

1. Runtime/Postgres remains canonical for execution, policy/spend, and receipts.
2. Convex is projection-only for reactive sync.
3. Runtime is the single writer for Convex projection documents.
4. Laravel remains auth/session authority and mints Convex client JWTs.
5. Layer-0 schema authority remains in `proto/`.
6. No production MCP mutation path is enabled by default.
7. No phase is marked complete without explicit verification evidence.

## Current Baseline (as of 2026-02-19)

1. Runtime already uses Postgres/Ecto and has active migrations.
2. Local Postgres is reachable on `localhost:5432`.
3. `openagents_runtime_dev` and `openagents_runtime_test` both exist locally.
4. Convex integration architecture docs are in place, but infra/runtime writer
   implementation is still phased work.
5. Codex worker runtime contract exists; desktop/runtime sync remains in-progress.
6. Gate G1 infra baseline is now live in non-prod (`openagentsgemini/us-central1`)
   with passing checks from
   `apps/openagents-runtime/deploy/convex/check-nonprod-health.sh`.

## Delivery Gates

1. Gate G0: Local runtime + Postgres baseline green.
2. Gate G1: Non-prod self-hosted Convex environment healthy in GCP.
3. Gate G2: Runtime projection writer publishes deterministic run/codex summaries.
4. Gate G3: Laravel Convex JWT minting + client subscription auth flow validated.
5. Gate G4: Web Codex admin surfaces live from runtime + Convex summaries.
6. Gate G5: Desktop Codex events fully mirrored into runtime and reflected in Convex.
7. Gate G6: Mobile read/admin parity on same runtime/Convex contracts.
8. Gate G7: Production cutover with rollback drill completed.

## Phased Roadmap

## Phase 0: Local Baseline and Program Setup (G0)

Objectives:

- guarantee repeatable local runtime + Postgres setup,
- pin implementation checkpoints and owners,
- establish proof checklist for every later gate.

Backlog:

1. Verify local Postgres connectivity and credentials for runtime.
2. Ensure `openagents_runtime_dev` exists and migrations run cleanly.
3. Add/verify local setup section in runtime docs with exact commands.
4. Create a release checklist artifact for Gates G0-G7.
5. Define CI verification matrix for runtime/web/desktop/mobile touchpoints.

Verification:

- `cd apps/openagents-runtime && mix ecto.create`
- `cd apps/openagents-runtime && mix ecto.migrate`
- `cd apps/openagents-runtime && mix test`
- `cd apps/openagents-runtime && mix runtime.contract.check`

Exit criteria:

- local runtime boots against local Postgres,
- migration state is clean and reproducible,
- verification commands are documented and pass.

### Phase 0 Local CI Verification Matrix (Draft)

| Surface | Local gate command | Trigger scope | Success criteria |
|---|---|---|---|
| Runtime (Elixir) | `cd apps/openagents-runtime && mix ci` | `apps/openagents-runtime/**`, runtime contracts, projections | format + compile + contract + test pass |
| Web (Laravel) | `cd apps/openagents.com && composer test` | `apps/openagents.com/**` | php feature/unit suites pass |
| Web Codex API proxy | `cd apps/openagents.com && php artisan test --filter=RuntimeCodexWorkersApiTest` | runtime Codex API integration changes | runtime worker proxy tests pass |
| Comms replay matrix | `./scripts/comms-security-replay-matrix.sh all` | comms auth/secret/replay changes across Laravel/runtime | both `laravel` and `runtime` lanes pass |
| Proto compatibility | `buf lint && buf breaking --against '.git#branch=main,subdir=proto' && ./scripts/verify-proto-generate.sh` | `proto/**`, `buf.yaml`, `buf.gen.yaml` | lint + breaking + generation checks pass |
| Desktop (Rust) | `cargo check -p autopilot-desktop && cargo check -p pylon` | `apps/autopilot-desktop/**`, `crates/pylon/**` | both crates compile with no errors |
| Mobile | `cd apps/mobile && bun run compile && bun run test` | `apps/mobile/**` | compile and test pass |
| OpenClaw drift gate | `OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE=1 ./scripts/openclaw-drift-report.sh` | openclaw intake/fixture drift changes | report generated, actionable drift rows = 0 |

Execution wiring:

- Install hooks once with `./scripts/install-git-hooks.sh`.
- Pre-commit runs `./scripts/local-ci.sh changed`.
- Pre-push runs `./scripts/local-ci.sh changed` (override with
  `OA_LOCAL_CI_PUSH_MODE=all` when doing full-lane validation).

## Phase 1: Self-Hosted Convex in GCP Non-Prod (G1)

Objectives:

- stand up operational Convex backend + dashboard in OpenAgents GCP perimeter,
- validate lifecycle operations and backup posture.

Backlog:

1. Provision non-prod runtime-adjacent Convex environment.
2. Configure Convex backend + dashboard image pinning.
3. Configure SQL backend (`POSTGRES_URL`) in same GCP region.
4. Configure baseline env vars:
   - `CONVEX_CLOUD_ORIGIN`
   - `CONVEX_SITE_ORIGIN`
   - `NEXT_PUBLIC_DEPLOYMENT_URL`
   - `POSTGRES_URL`
5. Apply hardening env flags:
   - `REDACT_LOGS_TO_CLIENT=true`
   - `DISABLE_BEACON=true`
6. Validate admin key handling and secret distribution process.
7. Validate export/import backup posture in non-prod.
8. Write operational runbook for upgrades and rollback.

Implementation assets:

- `apps/openagents-runtime/deploy/convex/provision-nonprod-gcp.sh`
- `apps/openagents-runtime/deploy/convex/check-nonprod-health.sh`
- `apps/openagents-runtime/deploy/convex/README.md`
- `apps/openagents-runtime/deploy/convex/OPERATIONS_RUNBOOK.md`

Verification:

- `npx convex dev --once` with self-hosted deployment envs
- dashboard access and function/table visibility
- `npx convex export` and `npx convex import` dry runs

Exit criteria:

- non-prod Convex is healthy and observable,
- backup/restore test passes,
- upgrade path documented.

## Phase 2: Protocol and Data Contracts (G1 prerequisite for G2/G3)

Objectives:

- lock projection and Codex event contract shapes in Layer-0 schemas,
- eliminate ambiguous language-local DTO drift.

Backlog:

1. Add/expand proto files for Codex worker lifecycle and event envelopes:
   - `codex_workers.proto`
   - `codex_events.proto`
   - `codex_sandbox.proto`
   - `codex_auth.proto`
2. Define projection envelope fields:
   - runtime source IDs,
   - sequence references,
   - projection version,
   - projected timestamp.
3. Add compatibility checks for additive evolution.
4. Wire generation/verification in CI and local scripts.

Verification:

- `buf lint`
- `buf breaking --against '.git#branch=main'`
- `scripts/verify-proto-generate.sh`

Exit criteria:

- proto contracts merged and generated artifacts aligned,
- breaking-change checks enforced.

## Phase 3: Runtime Projection Writer (G2)

Objectives:

- implement deterministic runtime -> Convex projection flow,
- support replay-based rebuild from runtime durable history.

Backlog:

1. Build runtime-owned projector module for run/codex summary projections.
2. Add projection checkpoint tracking tied to runtime event sequence.
3. Implement idempotent upsert semantics for projection writes.
4. Add projection drift detector based on sequence/version markers.
5. Add drop + replay workflow for projection rebuild.
6. Add projector integration tests with fixed event fixtures.
7. Add metrics:
   - projection lag,
   - write failures,
   - replay duration,
   - drift incidents.

Verification:

- runtime integration tests for deterministic projection outputs
- drift simulation and replay recovery test
- observability signals emitted for projection health

Exit criteria:

- runtime is single-writer for target Convex projections,
- deterministic replay/rebuild is proven by tests.

## Phase 4: Laravel Auth Bridge and API Surface (G3)

Objectives:

- establish production-safe client auth path into Convex,
- keep Laravel as session authority.

Backlog:

1. Implement Laravel endpoint to mint short-lived Convex JWTs.
2. Map OpenAgents user/session claims to Convex identity claims.
3. Add explicit token TTL, issuer, audience, and scope enforcement.
4. Add tests for token mint denial paths and refresh behavior.
5. Document auth flow for web/mobile/desktop clients.

Verification:

- Laravel feature tests for `/api/convex/token` success/failure
- token validation against Convex custom JWT/OIDC setup

Exit criteria:

- clients can subscribe to Convex with Laravel-issued JWTs only,
- admin keys remain operator-only.

## Phase 5: Web Codex + Runtime Sync Surfaces (G4)

Objectives:

- expose stable Codex admin and observability in web,
- combine runtime authoritative controls with Convex reactive summaries.

Backlog:

1. Add/verify Laravel Codex worker stream proxy endpoint.
2. Build web worker list/detail surfaces from runtime snapshots + stream.
3. Add Convex subscription integration for summary/state badges.
4. Ensure destructive/admin actions always route through runtime APIs.
5. Add web E2E flows for create/request/stream/stop/reconnect.

Implementation status (2026-02-19):

- Laravel now exposes worker list/create/show/request/stream/stop at
  `/api/runtime/codex/workers*`.
- Admin UI in `apps/openagents.com/resources/js/pages/admin/index.tsx` now
  supports worker list/detail/actions plus live stream log.
- Worker cards include runtime-projected Convex status badges (`in_sync` /
  `lagging`) sourced from runtime projection checkpoints.

Verification:

- Laravel API tests for worker endpoints and stream proxy
- web E2E smoke and regression checks

Exit criteria:

- web can fully administer workers and observe live status,
- no control action depends on Convex-authoritative state.

## Phase 6: Desktop Runtime Sync Completion (G5)

Objectives:

- make desktop-first Codex execution fully visible and governable across surfaces.

Backlog:

1. Ensure desktop creates/reattaches runtime worker on session start.
2. Add runtime event ingest for desktop async notifications.
3. Map desktop local bridge events to stable runtime taxonomy.
4. Ensure heartbeat + stop semantics are durable and replayable.
5. Mirror worker summaries into Convex via runtime projector.
6. Validate reconnect/resume after desktop app restart.

Implementation status (2026-02-19):

- Items 1-4 are implemented:
  - Desktop session start/resume paths now ensure runtime worker create/reattach.
  - Runtime ingest endpoint is live at `POST /internal/v1/codex/workers/{worker_id}/events`.
  - Laravel proxy route is live at `POST /api/runtime/codex/workers/{workerId}/events`.
  - Desktop and Pylon local bridge now emit normalized runtime taxonomy mappings (`worker.started`, `worker.stopped`, `worker.error`, `worker.heartbeat`, `worker.event`, `worker.request.received`).
  - Runtime worker mutations now enforce terminal-state conflicts (`409`) until explicit reattach/resume, and worker snapshot/list include deterministic heartbeat-state policy fields (`heartbeat_state`, `heartbeat_age_ms`, `heartbeat_stale_after_ms`).
- Item 6 now has runtime-level restart/reattach resilience coverage through stop/resume replay tests and deterministic same-worker-id reattach behavior.
- Item 5 is live for status badges via runtime projection checkpoints in web admin.
- Remaining gap: add full desktop process restart E2E automation in CI.

Verification:

- `cargo check -p autopilot-desktop`
- `cargo check -p pylon`
- runtime controller/domain tests for codex worker ingest/stream

Exit criteria:

- desktop activity appears in runtime and Convex summaries in near real time,
- cross-client admin actions reflect actual desktop worker state.

## Phase 7: Mobile Read/Admin Parity (G6)

Objectives:

- bring mobile onto the same runtime/Convex contract model.

Backlog:

1. Ship read parity first: worker snapshot + status subscriptions.
2. Add scoped admin controls behind policy/role checks.
3. Reuse Laravel token mint and runtime proxy pathways.
4. Add mobile resilience paths for token refresh and reconnect.

Verification:

- mobile integration tests against staging runtime + Convex
- manual E2E: web/desktop/mobile consistency checks

Exit criteria:

- mobile sees the same worker lifecycle truth as web/desktop,
- controls are policy-safe and auditable.

## Phase 8: Production Hardening and Cutover (G7)

Objectives:

- cut over safely with rollback confidence and operational readiness.

Backlog:

1. Run load/chaos tests across runtime + Convex projection path.
2. Set production alert thresholds for lag/drift/error budgets.
3. Execute backup/restore and replay drills.
4. Execute staged rollout:
   - internal users,
   - limited cohort,
   - full exposure.
5. Run rollback drill and measure recovery time.
6. Finalize runbooks and on-call ownership map.

Verification:

- load test reports and alerting validation
- successful rollback and replay drill in production-like environment

Exit criteria:

- SLOs met during staged rollout,
- rollback is proven and documented.

## Cross-Workstream Operational Track

## Security

1. Keep Convex admin keys in secret manager only.
2. Enforce least privilege service identity between Laravel/runtime/Convex infra.
3. Keep MCP production access disabled by default.
4. Require explicit change control for production MCP enablement.
5. Validate no runtime receipt/log path leaks secrets.

## Observability

1. Dashboard panels for runtime event throughput and projection lag.
2. Alerts for projection drift, write failures, and auth token issuance failures.
3. Correlate request IDs across Laravel -> runtime -> projector writes.
4. Add replay/rebuild duration and success metrics.

## Data Governance

1. Maintain data class split:
   - kernel truth in runtime/Postgres,
   - projections in Convex.
2. Ensure additive projection schema evolution.
3. Retain replayability evidence for projection rebuilds.

## Risks and Mitigations

1. Risk: multi-writer drift in Convex projections.  
   Mitigation: runtime-only writer policy and explicit guardrails in code review.
2. Risk: token-mint path outage blocks subscriptions.  
   Mitigation: short-lived token refresh with graceful retry/backoff and alerts.
3. Risk: Convex upgrade regressions.  
   Mitigation: pinned versions + export-before-upgrade + non-prod canary.
4. Risk: desktop/runtime reconnect inconsistencies.  
   Mitigation: idempotent worker reattach and heartbeat/lease checks.
5. Risk: schema drift across languages.  
   Mitigation: proto-first workflow with `buf breaking` in CI.

## Definition of Done

The roadmap is complete only when:

1. Gates G0 through G7 are each closed with verification artifacts.
2. Runtime/Postgres authority remains intact with no Convex authority creep.
3. Web/mobile/desktop observe consistent Codex worker state from shared contracts.
4. Replay-based Convex projection rebuild is documented, tested, and exercised.
5. Operational runbooks and alerting are in place for production support.

## Execution Checklist (Agent-Friendly)

Use this checklist in order; do not claim completion without evidence.

1. Close G0 baseline and attach command outputs.
2. Stand up non-prod Convex and close G1.
3. Land proto contracts and close Phase 2 contract tasks.
4. Implement projector MVP and close G2.
5. Land Laravel Convex JWT endpoint and close G3.
6. Ship web Codex admin surfaces and close G4.
7. Finish desktop runtime sync and close G5.
8. Ship mobile parity and close G6.
9. Run production hardening/cutover and close G7.

## Decision Log

- 2026-02-19: Roadmap created as execution companion to ADR-0029 and Convex sync plan.
- 2026-02-19: Kept architecture authority in existing docs; this file is delivery and sequencing only.
