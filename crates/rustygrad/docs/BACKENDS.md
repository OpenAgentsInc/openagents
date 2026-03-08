# Rustygrad Backends

Rustygrad backends are explicit runtime implementations, not hidden environment
switches.

## Backend Inventory

- `rustygrad-backend-cpu`
  Reference backend for correctness, deterministic tests, and early smoke flows.
- `rustygrad-backend-metal`
  Apple GPU backend with a first model-backed embeddings product path.
- `rustygrad-backend-amd-kfd`
  AMD discovery/readiness backend aligned with the standard amdgpu/KFD posture.
- `rustygrad-backend-amd-userspace`
  AMD discovery/readiness backend for the higher-risk userspace driver posture.

## Backend Contract

Each backend is expected to report:

- backend identifier
- discovered devices
- supported dtypes
- runtime health/readiness
- supported execution modes and limitations

## AMD Policy

AMD KFD and AMD userspace remain separate backends in both code and provider
capability reporting. They should not be collapsed into a single generic `amd`
mode because they have different operational and trust assumptions.

See [AMD.md](./AMD.md) for the operator runbook, readiness states, and current
phase boundary.
