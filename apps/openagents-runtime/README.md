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
- `mix ci` (format check + compile warnings-as-errors + test warnings-as-errors)

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
