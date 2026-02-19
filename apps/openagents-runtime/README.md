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

Internal API docs:

- Contract: `docs/RUNTIME_CONTRACT.md`
- OpenAPI spec: `docs/openapi-internal-v1.yaml`
- Laravel SSE mapping: `docs/LARAVEL_SSE_MAPPING.md`
- Observability + telemetry guardrails: `docs/OBSERVABILITY.md`
- Dashboards + alerts runbook: `docs/OPERATIONS_ALERTING.md`
