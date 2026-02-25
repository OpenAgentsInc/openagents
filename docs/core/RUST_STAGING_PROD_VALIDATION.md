# Rust Staging/Prod Validation Matrix

Status: active
Owner: `owner:infra`, `owner:openagents.com`, `owner:runtime`
Issue: OA-RUST-110 (`#1935`)

## Purpose

Define one deterministic matrix that gates Rust cutover readiness for staging and production.

## Canonical Entrypoint

```bash
scripts/release/validate-rust-cutover.sh
```

Validation evidence should be archived in backroom (not committed into `docs/`).

## Matrix Lanes

1. Control service lane
- health/readiness checks (`/readyz` is canonical; `/healthz` is best-effort in some environments)
- static host policy checks
- auth/session/sync token checks (when auth token is provided)
- HTMX perf budget checks (`login/feed/settings/chat`) via:

```bash
apps/openagents.com/service/scripts/htmx_perf_check.sh
```
- HTMX browser smoke checks (`chat/feed/settings` on staging/prod token lanes) via:

```bash
apps/openagents.com/service/scripts/htmx_browser_smoke.sh
```
- HTMX route-group staged rollout canary (staging token lane) via:

```bash
apps/openagents.com/service/scripts/htmx-route-group-canary.sh
```

1. Gmail inbox lane (control-service)
- connect Google account via `/settings/integrations/google/redirect` and callback completion
- verify inbox thread list endpoint:
  - `GET /api/inbox/threads`
- verify inbox detail + action endpoints:
  - `GET /api/inbox/threads/{thread_id}`
  - `POST /api/inbox/threads/{thread_id}/draft/approve`
  - `POST /api/inbox/threads/{thread_id}/draft/reject`
  - `POST /api/inbox/threads/{thread_id}/reply/send`
- confirm runtime comms ingest route alignment:
  - control forwards to `/internal/v1/comms/delivery-events`
  - runtime accepts `POST /internal/v1/comms/delivery-events`
- capture release evidence:
  - request/response logs with request ids
  - connected Gmail test account identifier (redacted)
  - send confirmation message id

2. Runtime lane
- runtime health + authority API smoke
- migration drift check (runtime image matches migrate job image)
- CEP MVP-1 contract checks:
  - `GET /internal/v1/openapi.json` includes `/credit/*` operations
  - `GET /internal/v1/credit/health`
  - `GET /internal/v1/credit/agents/{agent_id}/exposure` (known test agent)
- Hydra MVP-2 contract + observability checks:
  - `GET /internal/v1/openapi.json` includes `/hydra/routing/score`, `/hydra/risk/health`, `/hydra/observability`
  - `GET /internal/v1/hydra/observability` returns routing counts, confidence distribution buckets, breaker transitions/recoveries, and withdrawal throttle affected-request counters
  - `./scripts/vignette-hydra-mvp2.sh`
- Hydra MVP-3 FX contract + determinism checks:
  - `GET /internal/v1/openapi.json` includes `/hydra/fx/rfq`, `/hydra/fx/quote`, `/hydra/fx/select`, `/hydra/fx/settle`
  - `GET /internal/v1/hydra/observability` returns `fx` metrics (`rfq_total`, `quote_total`, `settlement_total`, conversion, spread avg/median, withheld/failed, provider breadth)
  - `./scripts/vignette-hydra-mvp3.sh`

2.5. Public stats lane
- `GET /stats` renders minute-cache Hydra rows with stable columns for:
  - selected route counts (`route-direct`, `route-cep`, `other`)
  - confidence distribution buckets (`<0.40`, `0.40-0.70`, `0.70-0.90`, `>=0.90`)
  - breaker transitions + recoveries
  - throttle mode + affected/rejected/stressed request totals
  - FX rows (`rfq/quote/settlement`, quote->settle conversion, spread avg/median, withheld/failed, treasury provider breadth)

3. Spacetime sync contract lane

```bash
cargo test --manifest-path apps/runtime/Cargo.toml server::tests::spacetime_sync_metrics_expose_stream_delivery_totals -- --nocapture
cargo test --manifest-path apps/runtime/Cargo.toml server::tests::retired_spacetime_routes_return_not_found -- --nocapture
```

4. Surface parity lane (optional)

```bash
scripts/run-cross-surface-contract-harness.sh
```

5. Observability + rollback readiness
- logging probes
- canary/rollback status checks

## Required Environment

Defaults:

- `STAGING_CONTROL_BASE_URL=https://staging.openagents.com`
- `PROD_CONTROL_BASE_URL=https://openagents.com`
- `GCP_PROJECT=openagentsgemini`
- `GCP_REGION=us-central1`
- `CONTROL_SERVICE=openagents-control-service`
- `RUNTIME_SERVICE=runtime`
- `MIGRATE_JOB=runtime-migrate`

Optional authenticated smoke tokens:

- `STAGING_CONTROL_ACCESS_TOKEN`
- `PROD_CONTROL_ACCESS_TOKEN`

HTMX perf lane toggle:

- `RUN_HTMX_PERF_CHECKS=1` (default)
- `RUN_HTMX_BROWSER_SMOKE=1` (default)
- `RUN_HTMX_ROUTE_GROUP_CANARY=1` (default)

## Go/No-Go Policy

1. All required checks must pass.
2. Required checks cannot be skipped in release gating mode.
3. Any required failure is a no-go until fixed and rerun.
4. Legacy deletion phases require fresh Laravel DB backup evidence in addition to this matrix.

## Related Runbooks

- `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
- `apps/openagents.com/service/docs/HTMX_ROUTE_GROUP_ROLLOUT.md`
- `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- `docs/core/SCHEMA_EVOLUTION_PLAYBOOK.md`
