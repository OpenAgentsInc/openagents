# Rustygrad Backends

Rustygrad backends are explicit runtime implementations, not hidden environment
switches.

## Backend Inventory

- `rustygrad-backend-cpu`
  Reference backend for correctness, deterministic tests, and early smoke flows.
- `rustygrad-backend-metal`
  Apple GPU backend placeholder for later acceleration work.
- `rustygrad-backend-amd-kfd`
  AMD backend aligned with the standard amdgpu/KFD posture.
- `rustygrad-backend-amd-userspace`
  AMD userspace backend placeholder for the higher-risk sovereign driver path.

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
