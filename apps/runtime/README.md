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

Baseline endpoints:

- `GET /healthz`
- `GET /readyz`
- `POST /internal/v1/runs`
- `POST /internal/v1/runs/:run_id/events`
- `GET /internal/v1/runs/:run_id`
- `GET /internal/v1/runs/:run_id/receipt`
- `GET /internal/v1/runs/:run_id/replay`
- `GET /internal/v1/projectors/checkpoints/:run_id`
- `GET /internal/v1/projectors/run-summary/:run_id`
- `GET /internal/v1/projectors/drift?topic=<topic>`
- `POST /internal/v1/workers`
- `GET /internal/v1/workers/:worker_id?owner_user_id=<id>`
- `POST /internal/v1/workers/:worker_id/heartbeat`
- `POST /internal/v1/workers/:worker_id/status`
- `GET /internal/v1/workers/:worker_id/checkpoint`

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

Legacy Elixir/Phoenix runtime is still present for staged migration issues and should be treated as transitional.

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
- Sanitization/redaction policy: `docs/SANITIZATION_POLICY.md`
- Kubernetes network policy hardening: `docs/NETWORK_POLICY.md`
- Deploy runbook (GCP/GKE): `docs/DEPLOY_GCP.md`
- Deploy runbook (Cloud Run): `docs/DEPLOY_CLOUD_RUN.md`
- Runtime operations runbook: `docs/OPERATIONS.md`
- Reprojection and drift-repair runbook: `docs/REPROJECTION.md`
- Shadow parity harness runbook: `docs/SHADOW_MODE_PARITY.md`
- Rust authority cutover runbook: `docs/RUST_AUTHORITY_CUTOVER.md`
