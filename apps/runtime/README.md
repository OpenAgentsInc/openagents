# OpenAgentsRuntime

Internal runtime service for OpenAgents long-running agent execution.

## Rust service foundation (OA-RUST-033)

`apps/runtime` now includes a Rust service foundation used for runtime authority migration.

Commands:

1. `cargo run -p openagents-runtime-service`
2. `cargo test -p openagents-runtime-service`

Environment:

- `RUNTIME_BIND_ADDR` (default `127.0.0.1:4100`)
- `RUNTIME_SERVICE_NAME` (default `runtime`)
- `RUNTIME_BUILD_SHA` (default `dev`)
- `RUNTIME_EVENT_LOG_PATH` (default `.runtime-data/runtime-events.jsonl`)
- `RUNTIME_CHECKPOINT_PATH` (default `.runtime-data/projection-state.json`)
- `RUNTIME_AUTHORITY_WRITE_MODE` (`rust_active|shadow_only|read_only`, default `rust_active`)
- `LEGACY_RUNTIME_WRITE_FREEZE` (`true|false`) for legacy Elixir write-path freeze
- `RUNTIME_FANOUT_DRIVER` (currently `memory`)
- `RUNTIME_FANOUT_QUEUE_CAPACITY` (default `1024`)
- `RUNTIME_SYNC_TOKEN_SIGNING_KEY` (HS256 key for Khala topic auth checks)
- `RUNTIME_SYNC_TOKEN_ISSUER` (default `https://openagents.com`)
- `RUNTIME_SYNC_TOKEN_AUDIENCE` (default `openagents-sync`)
- `RUNTIME_SYNC_REVOKED_JTIS` (comma-separated revoked token IDs)

Baseline endpoints:

- `GET /healthz`
- `GET /readyz`
- `POST /internal/v1/runs`
- `POST /internal/v1/runs/:run_id/events`
- `GET /internal/v1/runs/:run_id`
- `GET /internal/v1/runs/:run_id/receipt`
- `GET /internal/v1/runs/:run_id/replay`
- `GET /internal/v1/khala/topics/:topic/messages?after_seq=<n>&limit=<n>` (requires `Authorization: Bearer <sync-token>`)
- `GET /internal/v1/khala/fanout/hooks`
- `GET /internal/v1/projectors/checkpoints/:run_id`
- `GET /internal/v1/projectors/run-summary/:run_id`
- `GET /internal/v1/projectors/drift?topic=<topic>`
- `POST /internal/v1/workers`
- `GET /internal/v1/workers/:worker_id?owner_user_id=<id>`
- `POST /internal/v1/workers/:worker_id/heartbeat`
- `POST /internal/v1/workers/:worker_id/status`
- `GET /internal/v1/workers/:worker_id/checkpoint`
- `POST /internal/v1/sync/sessions/revoke` (internal control-plane revocation signal for live WS eviction)

The Rust boundary modules live under `apps/runtime/src/`:

- `authority.rs`: authority write interface + in-memory implementation
- `orchestration.rs`: run/event command orchestration and validation
- `projectors.rs`: projection checkpoint pipeline foundation
- `workers.rs`: worker registry, ownership checks, heartbeat/liveness, status transitions
- `server.rs`: HTTP routes, health/readiness, API handlers

Run state machine semantics:

- Supported statuses: `created`, `running`, `canceling`, `canceled`, `succeeded`, `failed`.
- Runtime validates event-driven transitions (for example `run.started`, `run.cancel_requested`, `run.finished`) and rejects illegal transitions with `400 invalid_request`.

Durable event log semantics:

- Run events are appended to a durable JSONL event log before in-memory projection updates.
- `POST /internal/v1/runs/:run_id/events` supports `idempotency_key` and `expected_previous_seq`.
- Duplicate `idempotency_key` returns idempotent replay without appending duplicate events.
- `expected_previous_seq` mismatches return `409 conflict`.

Projection checkpoint semantics:

- Projection checkpoints/read-model summaries are persisted to `RUNTIME_CHECKPOINT_PATH`.
- Recovered projectors resume from persisted checkpoints and apply only new events.
- Duplicate reprocessing is idempotent (`seq <= last_seq` is ignored).
- Sequence gaps register drift reports accessible through `/internal/v1/projectors/drift`.

Shadow parity harness:

- Run: `cargo run -p openagents-runtime-service --bin runtime-shadow-harness -- --legacy-manifest <path> --rust-manifest <path> --output <path>`
- Runbook: `apps/runtime/docs/SHADOW_MODE_PARITY.md`
- Harness exits non-zero when gate policy blocks cutover.

Authority cutover controls:

- Rust authority writes are enabled only when `RUNTIME_AUTHORITY_WRITE_MODE=rust_active`.
- `shadow_only` or `read_only` modes return `503 write_path_frozen` on Rust write endpoints.
- Legacy Elixir runtime can be frozen with `LEGACY_RUNTIME_WRITE_FREEZE=true` (returns `410 write_path_frozen` on mutation routes).

Khala fanout seam:

- Runtime writes publish through `FanoutDriver` abstraction (`memory` adapter implemented).
- Fanout queue is bounded per topic; oldest messages are evicted when capacity is exceeded.
- Khala topic polling enforces token auth + ACL checks:
  - topic scope matrix (`runtime.run_events`, `runtime.codex_worker_events`, `runtime.worker_lifecycle_events`)
  - worker topic ownership checks against worker owner
  - deterministic denied-path reason codes (`missing_authorization`, `invalid_authorization_scheme`, `invalid_token`, `token_expired`, `token_revoked`, `missing_scope`, `forbidden_topic`, `owner_mismatch`)
- `GET /internal/v1/khala/topics/:topic/messages` enforces deterministic stale-cursor handling:
  - `410 stale_cursor` when `after_seq` is below retained replay floor.
  - response details include `requested_cursor`, `oldest_available_cursor`, `head_cursor`, and recovery hint `reset_local_watermark_and_replay_bootstrap`.
  - successful responses include replay bootstrap metadata: `oldest_available_cursor`, `head_cursor`, `next_cursor`, `replay_complete`.
- `RUNTIME_FANOUT_DRIVER` keeps protocol stable while reserving hooks for `nats`, `redis`, and `postgres_notify` adapters.

Legacy Elixir/Phoenix runtime is still present for staged migration issues and should be treated as transitional.

Khala websocket revocation semantics:

- Sync sockets are bound to `oa_session_id` + `oa_device_id` claims.
- Runtime tracks revoked sessions/devices and denies reconnect with `reauth_required`.
- Revocation signals from control-plane disconnect matching live sockets immediately.

## Local development

1. `mix setup`
2. `mix phx.server`

Health endpoint:

- `GET http://localhost:4000/internal/v1/health`

Useful commands:

- `mix test`
- `mix format`
- `mix format --check-formatted`
- `mix runtime.contract.check` (validate `docs/` artifacts against implemented `/internal/v1` routes)
- `mix ci` (format check + compile warnings-as-errors + contract check + test warnings-as-errors)
- `DB_URL=<postgres-url> apps/runtime/deploy/cloudrun/verify-db-role-isolation.sh` (authority-plane DB role drift check)

## Local Postgres baseline (Gate G0)

Use these commands to verify runtime local DB prerequisites before feature work.

```bash
pg_isready -h localhost -p 5432
PGPASSWORD=postgres psql -h localhost -U postgres -d postgres -Atc "select datname from pg_database where datname in ('openagents_runtime_dev','openagents_runtime_test') order by datname;"
```

Expected:

- `localhost:5432 - accepting connections`
- `openagents_runtime_dev`
- `openagents_runtime_test`

Then validate runtime schema + test + contract checks:

```bash
mix ecto.create
mix ecto.migrate
mix test
mix runtime.contract.check
```

Expected:

- DB create/migrate are idempotent and complete without errors.
- Test suite passes.
- Contract check prints `runtime contract check passed`.

Container build:

- `docker build -t runtime:dev .`
- Cloud Build config: `deploy/cloudbuild.yaml`
- Cloud Run chained deploy + migration command:
  - `GCP_PROJECT=openagentsgemini GCP_REGION=us-central1 RUNTIME_SERVICE=runtime MIGRATE_JOB=runtime-migrate IMAGE=us-central1-docker.pkg.dev/<project>/runtime/runtime:<tag> apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh`

Kubernetes manifests:

- Base manifests: `deploy/k8s/base`
- Environment overlays: `deploy/k8s/overlays/{dev,staging,prod}`
- Render manifests:
  - `kubectl kustomize deploy/k8s/overlays/dev`
  - `kubectl kustomize deploy/k8s/overlays/staging`
  - `kubectl kustomize deploy/k8s/overlays/prod`
- Apply manifests:
  - `kubectl apply -k deploy/k8s/overlays/dev`
  - `kubectl apply -k deploy/k8s/overlays/staging`
  - `kubectl apply -k deploy/k8s/overlays/prod`
- Deploy-time migration/smoke jobs: `deploy/jobs/README.md`

Internal API docs:

- Contract: `docs/RUNTIME_CONTRACT.md`
- OpenAPI spec: `docs/openapi-internal-v1.yaml`
- Laravel SSE mapping: `docs/LARAVEL_SSE_MAPPING.md`
- DS-Elixir runtime contract: `docs/DS_ELIXIR_RUNTIME_CONTRACT.md`
- DS-Elixir operations runbook: `docs/DS_ELIXIR_OPERATIONS.md`
- Khala sync-layer integration boundary: `docs/KHALA_SYNC.md`
- Observability + telemetry guardrails: `docs/OBSERVABILITY.md`
- Dashboards + alerts runbook: `docs/OPERATIONS_ALERTING.md`
- Load/chaos validation suite: `docs/LOAD_TESTING.md`
- Restart/reconnect chaos drill runbook: `docs/RESTART_RECONNECT_CHAOS.md`
- WS/auth/stale-cursor incident runbook: `docs/INCIDENT_WS_AUTH_RECONNECT_STALE_CURSOR.md`
- Sanitization/redaction policy: `docs/SANITIZATION_POLICY.md`
- Kubernetes network policy hardening: `docs/NETWORK_POLICY.md`
- Deploy runbook (GCP/GKE): `docs/DEPLOY_GCP.md`
- Deploy runbook (Cloud Run): `docs/DEPLOY_CLOUD_RUN.md`
- Runtime operations runbook: `docs/OPERATIONS.md`
- Reprojection and drift-repair runbook: `docs/REPROJECTION.md`
- Shadow parity harness runbook: `docs/SHADOW_MODE_PARITY.md`
- Rust authority cutover runbook: `docs/RUST_AUTHORITY_CUTOVER.md`
- DB role isolation policy + tooling: `docs/DB_ROLE_ISOLATION.md`
