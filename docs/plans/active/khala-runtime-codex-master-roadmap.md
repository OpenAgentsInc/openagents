# Khala + Runtime + Codex Master Execution Roadmap

Date: 2026-02-19  
Status: Active  
Owner: Runtime + Web + iOS + Desktop + Infra  
Primary audience: implementation teams and autonomous coding agents

## Goal

Ship a production-ready, self-hosted Khala sync layer that is integrated with
the OpenAgents runtime and Codex surfaces while preserving kernel correctness
invariants.

This roadmap is the execution companion to architecture docs. It does not
replace authority docs.

## Authority and Scope

Architecture and invariants are defined by:

- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0028-layer0-proto-canonical-schema.md`
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0029-khala-sync-layer-and-codex-agent-mode.md`
- `docs/codex/unified-runtime-desktop-plan.md`
- `docs/plans/active/khala-self-hosting-runtime-sync-plan.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `apps/runtime/docs/KHALA_SYNC.md`

This roadmap covers:

- sequencing and delivery gates,
- backlog by workstream,
- verification and release criteria,
- rollout and rollback posture.

## Hard Constraints

1. Runtime/Postgres remains canonical for execution, policy/spend, and receipts.
2. Khala is projection-only for reactive sync.
3. Runtime is the single writer for Khala projection documents.
4. Laravel remains auth/session authority and mints Khala client JWTs.
5. Layer-0 schema authority remains in `proto/`.
6. No production MCP mutation path is enabled by default.
7. No phase is marked complete without explicit verification evidence.

## Current Baseline (as of 2026-02-19)

1. Runtime already uses Postgres/Ecto and has active migrations.
2. Local Postgres is reachable on `localhost:5432`.
3. `openagents_runtime_dev` and `openagents_runtime_test` both exist locally.
4. Khala integration architecture docs are in place, but infra/runtime writer
   implementation is still phased work.
5. Codex worker runtime contract exists; desktop/runtime sync remains in-progress.
6. Gate G1 infra baseline is now live in non-prod (`openagentsgemini/us-central1`)
   with passing checks from
   `apps/runtime/deploy/khala/check-nonprod-health.sh`.

## Delivery Gates

1. Gate G0: Local runtime + Postgres baseline green.
2. Gate G1: Non-prod self-hosted Khala environment healthy in GCP.
3. Gate G2: Runtime projection writer publishes deterministic run/codex summaries.
4. Gate G3: Laravel Khala JWT minting + client subscription auth flow validated.
5. Gate G4: Web Codex admin surfaces live from runtime + Khala summaries.
6. Gate G5: Desktop Codex events fully mirrored into runtime and reflected in Khala.
7. Gate G6: iOS read/admin parity on same runtime/Khala contracts.
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
5. Define CI verification matrix for runtime/web/desktop/iOS touchpoints.

Verification:

- `cd apps/runtime && mix ecto.create`
- `cd apps/runtime && mix ecto.migrate`
- `cd apps/runtime && mix test`
- `cd apps/runtime && mix runtime.contract.check`

Exit criteria:

- local runtime boots against local Postgres,
- migration state is clean and reproducible,
- verification commands are documented and pass.

### Phase 0 Local CI Verification Matrix (Draft)

| Surface | Local gate command | Trigger scope | Success criteria |
|---|---|---|---|
| Runtime (Elixir) | `cd apps/runtime && mix ci` | `apps/runtime/**`, runtime contracts, projections | format + compile + contract + test pass |
| Web (Laravel) | `cd apps/openagents.com && composer test` | `apps/openagents.com/**` | php feature/unit suites pass |
| Web Codex API proxy | `cd apps/openagents.com && php artisan test --filter=RuntimeCodexWorkersApiTest` | runtime Codex API integration changes | runtime worker proxy tests pass |
| Comms replay matrix | `./scripts/comms-security-replay-matrix.sh all` | comms auth/secret/replay changes across Laravel/runtime | both `laravel` and `runtime` lanes pass |
| Proto compatibility | `buf lint && buf breaking --against '.git#branch=main,subdir=proto' && ./scripts/verify-proto-generate.sh` | `proto/**`, `buf.yaml`, `buf.gen.yaml` | lint + breaking + generation checks pass |
| Desktop (Rust) | `cargo check -p autopilot-desktop && cargo check -p pylon` | `apps/autopilot-desktop/**`, `crates/pylon/**` | both crates compile with no errors |
| iOS | `xcodebuild -project apps/autopilot-ios/Autopilot/Autopilot.xcodeproj -scheme Autopilot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test -only-testing:AutopilotTests` | `apps/autopilot-ios/**` | iOS unit/integration suite passes |
| OpenClaw drift gate | `OPENCLAW_DRIFT_FAIL_ON_ACTIONABLE=1 ./scripts/openclaw-drift-report.sh` | openclaw intake/fixture drift changes | report generated, actionable drift rows = 0 |

Execution wiring:

- Install hooks once with `./scripts/install-git-hooks.sh`.
- Pre-commit runs `./scripts/local-ci.sh changed`.
- Pre-push is currently a no-op; run `./scripts/local-ci.sh all` manually when doing full-lane validation.

## Phase 1: Self-Hosted Khala in GCP Non-Prod (G1)

Objectives:

- stand up operational Khala backend + dashboard in OpenAgents GCP perimeter,
- validate lifecycle operations and backup posture.

Backlog:

1. Provision non-prod runtime-adjacent Khala environment.
2. Configure Khala backend + dashboard image pinning.
3. Configure SQL backend (`POSTGRES_URL`) in same GCP region.
4. Configure baseline env vars:
   - `KHALA_CLOUD_ORIGIN`
   - `KHALA_SITE_ORIGIN`
   - `NEXT_PUBLIC_DEPLOYMENT_URL`
   - `POSTGRES_URL`
5. Apply hardening env flags:
   - `REDACT_LOGS_TO_CLIENT=true`
   - `DISABLE_BEACON=true`
6. Validate admin key handling and secret distribution process.
7. Validate export/import backup posture in non-prod.
8. Write operational runbook for upgrades and rollback.

Implementation assets:

- `apps/runtime/deploy/khala/provision-nonprod-gcp.sh`
- `apps/runtime/deploy/khala/check-nonprod-health.sh`
- `apps/runtime/deploy/khala/README.md`
- `apps/runtime/deploy/khala/OPERATIONS_RUNBOOK.md`

Verification:

- `npx khala dev --once` with self-hosted deployment envs
- dashboard access and function/table visibility
- `npx khala export` and `npx khala import` dry runs

Exit criteria:

- non-prod Khala is healthy and observable,
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

- implement deterministic runtime -> Khala projection flow,
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

- runtime is single-writer for target Khala projections,
- deterministic replay/rebuild is proven by tests.

## Phase 4: Laravel Auth Bridge and API Surface (G3)

Objectives:

- establish production-safe client auth path into Khala,
- keep Laravel as session authority.

Backlog:

1. Implement Laravel endpoint to mint short-lived Khala JWTs.
2. Map OpenAgents user/session claims to Khala identity claims.
3. Add explicit token TTL, issuer, audience, and scope enforcement.
4. Add tests for token mint denial paths and refresh behavior.
5. Document auth flow for web/iOS/desktop clients.

Verification:

- Laravel feature tests for `/api/khala/token` success/failure
- token validation against Khala custom JWT/OIDC setup

Exit criteria:

- clients can subscribe to Khala with Laravel-issued JWTs only,
- admin keys remain operator-only.

## Phase 5: Web Codex + Runtime Sync Surfaces (G4)

Objectives:

- expose stable Codex admin and observability in web,
- combine runtime authoritative controls with Khala reactive summaries.

Backlog:

1. Add/verify Laravel Codex worker stream proxy endpoint.
2. Build web worker list/detail surfaces from runtime snapshots + stream.
3. Add Khala subscription integration for summary/state badges.
4. Ensure destructive/admin actions always route through runtime APIs.
5. Add web E2E flows for create/request/stream/stop/reconnect.

Implementation status (2026-02-19):

- Laravel now exposes worker list/create/show/request/stream/stop at
  `/api/runtime/codex/workers*`.
- Admin UI in `apps/openagents.com/resources/js/pages/admin/index.tsx` now
  supports worker list/detail/actions plus live stream log.
- Worker cards include runtime-projected Khala status badges (`in_sync` /
  `lagging`) sourced from runtime projection checkpoints.

Verification:

- Laravel API tests for worker endpoints and stream proxy
- web E2E smoke and regression checks

Exit criteria:

- web can fully administer workers and observe live status,
- no control action depends on Khala-authoritative state.

## Phase 6: Desktop Runtime Sync Completion (G5)

Objectives:

- make desktop-first Codex execution fully visible and governable across surfaces.

Backlog:

1. Ensure desktop creates/reattaches runtime worker on session start.
2. Add runtime event ingest for desktop async notifications.
3. Map desktop local bridge events to stable runtime taxonomy.
4. Ensure heartbeat + stop semantics are durable and replayable.
5. Mirror worker summaries into Khala via runtime projector.
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

- desktop activity appears in runtime and Khala summaries in near real time,
- cross-client admin actions reflect actual desktop worker state.

## Phase 7: iOS Read/Admin Parity (G6)

Objectives:

- bring iOS onto the same runtime/Khala contract model.

Backlog:

1. Ship read parity first: worker snapshot + status subscriptions.
2. Add scoped admin controls behind policy/role checks.
3. Reuse Laravel token mint and runtime proxy pathways.
4. Add iOS resilience paths for token refresh and reconnect.

Implementation status (2026-02-19):

- iOS now includes a runtime-backed Codex worker screen with:
  - worker list + snapshot reads via Laravel runtime proxy APIs,
  - long-poll stream parity using runtime SSE endpoint semantics (`cursor` + `tail_ms`),
  - khala projection status visibility from runtime worker summaries.
- Scoped admin actions (`request`, `stop`) use the same Laravel runtime endpoints and honor runtime policy responses (`403`/`409`) without client-side bypass.
- Khala auth for iOS uses Laravel token minting (`POST /api/sync/token`) with short-lived token caching and refresh.
- Stream reconnect, list polling refresh, and auth-error fallback paths are implemented for iOS reconnect resilience.

Verification:

- iOS integration tests against staging runtime + Khala
- manual E2E: web/desktop/iOS consistency checks

Exit criteria:

- iOS sees the same worker lifecycle truth as web/desktop,
- controls are policy-safe and auditable.

## Phase 8: Production Hardening and Cutover (G7)

Objectives:

- cut over safely with rollback confidence and operational readiness.

Backlog:

1. Run load/chaos tests across runtime + Khala projection path.
2. Set production alert thresholds for lag/drift/error budgets.
3. Execute backup/restore and replay drills.
4. Execute staged rollout:
   - internal users,
   - limited cohort,
   - full exposure.
5. Run rollback drill and measure recovery time.
6. Finalize runbooks and on-call ownership map.

Implementation status (2026-02-19):

- Runtime + projector load/chaos suite expanded with Khala-specific cases:
  - `apps/runtime/test/openagents_runtime/load/khala_projection_load_chaos_test.exs`
  - sustained run-event burst lag checks,
  - codex worker heartbeat burst checkpoint convergence,
  - sink failure chaos with replay recovery validation.
- Production threshold artifacts now include Khala lag/drift/error budgets:
  - `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`
  - `apps/runtime/deploy/monitoring/grafana/runtime-ops-dashboard.json`
  - `apps/runtime/docs/OPERATIONS_ALERTING.md`
- Load/chaos verification report is tracked at:
  - `apps/runtime/docs/reports/2026-02-19-khala-runtime-projector-load-chaos-report.md`
- Backup/restore/replay + rollback drill evidence is tracked at:
  - `apps/runtime/docs/reports/2026-02-19-khala-g7-backup-restore-replay-rollback-drill.md`
- Drill automation and staged rollout/on-call runbooks are now in place:
  - `apps/runtime/deploy/khala/run-backup-restore-drill.sh`
  - `apps/runtime/deploy/khala/run-rollback-drill.sh`
  - `apps/runtime/deploy/khala/run-runtime-replay-drill.sh`
  - `apps/runtime/deploy/khala/OPERATIONS_RUNBOOK.md`
- Remaining Phase 8 scope:
  - staged cohort rollout execution in production.

Verification:

- load test reports and alerting validation
- successful rollback and replay drill in production-like environment

Exit criteria:

- SLOs met during staged rollout,
- rollback is proven and documented.

## Cross-Workstream Operational Track

## Security

1. Keep Khala admin keys in secret manager only.
2. Enforce least privilege service identity between Laravel/runtime/Khala infra.
3. Keep MCP production access disabled by default.
4. Require explicit change control for production MCP enablement.
5. Validate no runtime receipt/log path leaks secrets.

Implementation status (2026-02-19):

- Security checklist automation:
  - `apps/runtime/deploy/khala/run-security-review-checklist.sh`
- MCP production default-deny gate + temporary enablement controls:
  - `apps/runtime/deploy/khala/mcp-production-access-gate.sh`
- Secret-handling hardening now redacts `admin_key` and `*_admin_key` payload fields
  in runtime sanitization paths.
- Review evidence and sign-off:
  - `apps/runtime/docs/reports/2026-02-19-khala-security-review-checklist.md`

## Observability

1. Dashboard panels for runtime event throughput and projection lag.
2. Alerts for projection drift, write failures, and auth token issuance failures.
3. Correlate request IDs across Laravel -> runtime -> projector writes.
4. Add replay/rebuild duration and success metrics.

Implementation status (2026-02-19):

- Correlation headers are covered Laravel -> runtime and runtime -> projector telemetry:
  - `apps/openagents.com/tests/Feature/Api/RuntimeCodexWorkersApiTest.php`
  - `apps/runtime/test/openagents_runtime_web/controllers/codex_worker_controller_test.exs`
- Monitoring assets now include Khala token mint failure ratio dashboard + alert:
  - `apps/runtime/deploy/monitoring/grafana/runtime-ops-dashboard.json`
  - `apps/runtime/deploy/monitoring/prometheus/runtime-alert-rules.yaml`
- Operator runbooks now include correlation walkthrough + token mint failure response:
  - `apps/runtime/docs/OPERATIONS_ALERTING.md`

## Data Governance

1. Maintain data class split:
   - kernel truth in runtime/Postgres,
   - projections in Khala.
2. Ensure additive projection schema evolution.
3. Retain replayability evidence for projection rebuilds.

## Risks and Mitigations

1. Risk: multi-writer drift in Khala projections.  
   Mitigation: runtime-only writer policy and explicit guardrails in code review.
2. Risk: token-mint path outage blocks subscriptions.  
   Mitigation: short-lived token refresh with graceful retry/backoff and alerts.
3. Risk: Khala upgrade regressions.  
   Mitigation: pinned versions + export-before-upgrade + non-prod canary.
4. Risk: desktop/runtime reconnect inconsistencies.  
   Mitigation: idempotent worker reattach and heartbeat/lease checks.
5. Risk: schema drift across languages.  
   Mitigation: proto-first workflow with `buf breaking` in CI.

## Definition of Done

The roadmap is complete only when:

1. Gates G0 through G7 are each closed with verification artifacts.
2. Runtime/Postgres authority remains intact with no Khala authority creep.
3. Web/iOS/desktop observe consistent Codex worker state from shared contracts.
4. Replay-based Khala projection rebuild is documented, tested, and exercised.
5. Operational runbooks and alerting are in place for production support.

## Execution Checklist (Agent-Friendly)

Use this checklist in order; do not claim completion without evidence.

1. Close G0 baseline and attach command outputs.
2. Stand up non-prod Khala and close G1.
3. Land proto contracts and close Phase 2 contract tasks.
4. Implement projector MVP and close G2.
5. Land Laravel Khala JWT endpoint and close G3.
6. Ship web Codex admin surfaces and close G4.
7. Finish desktop runtime sync and close G5.
8. Ship iOS parity and close G6.
9. Run production hardening/cutover and close G7.

## Decision Log

- 2026-02-19: Roadmap created as execution companion to ADR-0029 and Khala sync plan.
- 2026-02-19: Kept architecture authority in existing docs; this file is delivery and sequencing only.
- 2026-02-19: Master tracker execution issues `#1750` through `#1768` were completed and synchronized in issue `#1769`.
