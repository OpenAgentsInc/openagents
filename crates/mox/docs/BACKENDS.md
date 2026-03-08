# Mox Backends

Mox backends are explicit runtime implementations, not hidden environment
switches.

## Backend Inventory

- `mox-backend-cpu`
  Reference backend for correctness, deterministic tests, and early smoke flows.
- `mox-backend-metal`
  Apple GPU backend with a first model-backed embeddings product path.
- `mox-backend-amd-kfd`
  AMD discovery/readiness backend aligned with the standard amdgpu/KFD posture.
- `mox-backend-amd-userspace`
  AMD discovery/readiness backend for the higher-risk userspace driver posture.

## Backend Contract

Each backend is expected to report:

- backend identifier
- discovered devices
- supported dtypes
- runtime health/readiness
- supported execution modes and limitations

Shipped backend claims are bounded by
[HARDWARE_VALIDATION_MATRIX.md](./HARDWARE_VALIDATION_MATRIX.md). Do not claim a
backend/product lane as shipped unless it has a row in that matrix and the
referenced tests are green.

## AMD Policy

AMD KFD and AMD userspace remain separate backends in both code and provider
capability reporting. They should not be collapsed into a single generic `amd`
mode because they have different operational and trust assumptions.

See [AMD.md](./AMD.md) for the operator runbook, readiness states, and current
phase boundary.
