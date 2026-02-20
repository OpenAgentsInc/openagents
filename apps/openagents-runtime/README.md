# OpenAgentsRuntime

Internal runtime service for OpenAgents long-running agent execution.

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

- `docker build -t openagents-runtime:dev .`
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
- Runtime operations runbook: `docs/OPERATIONS.md`
- Reprojection and drift-repair runbook: `docs/REPROJECTION.md`
