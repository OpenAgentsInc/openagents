# Rustygrad AMD Runbook

Rustygrad models AMD as two separate backend families:

- `amd_kfd`
- `amd_userspace`

This is intentional. They have different operator expectations, risk, and
recovery posture. Rustygrad does not collapse them into one generic `amd`
backend.

## Current Status

AMD support in this subtree is currently **discovery/readiness groundwork**, not
execution-complete support.

What exists today:

- explicit backend discovery/reporting in
  `rustygrad-backend-amd-kfd` and `rustygrad-backend-amd-userspace`
- serializable AMD runtime metadata in `rustygrad-runtime`
- provider-facing AMD context derived from backend/runtime state in
  `rustygrad-provider`

What does not exist yet:

- AMD execution kernels
- AMD-backed served product paths
- any claim that Rustygrad can execute inference or embeddings on AMD today

## Canonical Types

The AMD runbook is anchored on concrete types, not prose alone:

- `rustygrad_runtime::AmdBackendReport`
- `rustygrad_runtime::AmdDeviceMetadata`
- `rustygrad_runtime::AmdTopologyInfo`
- `rustygrad_runtime::AmdRiskProfile`
- `rustygrad_runtime::AmdRecoveryProfile`
- `rustygrad_backend_amd_kfd::AmdKfdBackend`
- `rustygrad_backend_amd_userspace::AmdUserspaceBackend`
- `rustygrad_provider::AmdCapabilityContext`

Reviewers and operators should prefer those types over free-form interpretations
of backend strings.

## AMD KFD

`amd_kfd` is the lower-risk AMD posture.

Expected environment:

- Linux host
- standard `amdgpu` kernel-driver posture
- `/dev/kfd` present
- AMD DRM devices visible under `/sys/class/drm`

Expected readiness states:

- `ready`
  AMD DRM devices are present and `/dev/kfd` exists.
- `degraded`
  AMD hardware is present but `/dev/kfd` is missing.
- `offline`
  no AMD KFD posture is detectable on the host.

Risk posture:

- `requires_explicit_opt_in = false`
- `may_unbind_kernel_driver = false`
- recovery is kernel-driver oriented (`kernel_driver_reset`, then `reboot_host`)

## AMD Userspace

`amd_userspace` is the higher-risk AMD posture.

Opt-in:

- Rustygrad never enables it silently.
- The explicit gate is `RUSTYGRAD_AMD_USERSPACE_ENABLE=1`.

Expected environment:

- Linux host
- dedicated machine or clearly isolated operator environment
- AMD DRM device present
- kernel-driver handoff prepared for userspace mode

Expected readiness states:

- `offline` + `opt_in = disabled`
  the operator has not enabled userspace mode
- `offline` + `opt_in = enabled`
  userspace mode was enabled but no AMD devices were detected
- `degraded` + `opt_in = enabled`
  AMD devices were found but `amdgpu` is still loaded, so the machine is not in
  the expected userspace posture
- `ready` + `opt_in = enabled`
  AMD devices were found and the kernel-driver handoff was detected

Risk posture:

- `requires_explicit_opt_in = true`
- `may_unbind_kernel_driver = true`
- recovery expects userspace-oriented intervention first
  (`process_restart`, `rebind_kernel_driver`, `reboot_host`)

## Validation

Repo-level validation coverage for the current AMD groundwork:

- `cargo test -p rustygrad-backend-amd-kfd`
- `cargo test -p rustygrad-backend-amd-userspace`
- `cargo test -p rustygrad-provider`

Practical host checks:

- KFD posture:
  verify `/dev/kfd` exists and AMD DRM devices are visible under
  `/sys/class/drm/card*/device`
- userspace posture:
  verify `RUSTYGRAD_AMD_USERSPACE_ENABLE=1` is set intentionally and confirm
  whether `/sys/module/amdgpu` is still loaded

## Review Boundary

If a future change claims AMD execution support, it must add more than discovery
and readiness truth. This runbook only covers the current discovery/reporting
phase and should stay explicit about that limit.
