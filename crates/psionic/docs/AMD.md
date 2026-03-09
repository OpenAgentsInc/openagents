# Psionic AMD Runbook

Psionic models AMD as two separate backend families:

- `amd_kfd`
- `amd_userspace`

This is intentional. They have different operator expectations, risk, and
recovery posture. Psionic does not collapse them into one generic `amd`
backend.

## Current Status

AMD support in this subtree is currently **execution-substrate groundwork**, not
served-product-complete support.

What exists today:

- explicit backend discovery/reporting in
  `psionic-backend-amd-kfd` and `psionic-backend-amd-userspace`
- backend-owned staging allocation and explicit fill/copy submission substrate
  in `psionic-backend-amd-kfd` and `psionic-backend-amd-userspace`
- explicit allocator/kernel-cache/device-budget truth for AMD execution
  substrate paths
- serializable AMD runtime metadata in `psionic-runtime`
- provider-facing AMD context derived from backend/runtime state in
  `psionic-provider`

What does not exist yet:

- AMD graph lowering or execution kernels for served products
- CPU-vs-AMD parity coverage for a shipped product path
- AMD-backed served product paths in `psionic-serve`
- any positive hardware-validation claim that Psionic can execute inference or
  embeddings on AMD today

## Canonical Types

The AMD runbook is anchored on concrete types, not prose alone:

- `psionic_runtime::AmdBackendReport`
- `psionic_runtime::AmdDeviceMetadata`
- `psionic_runtime::AmdTopologyInfo`
- `psionic_runtime::AmdRiskProfile`
- `psionic_runtime::AmdRecoveryProfile`
- `psionic_backend_amd_kfd::AmdKfdBackend`
- `psionic_backend_amd_userspace::AmdUserspaceBackend`
- `psionic_provider::AmdCapabilityContext`

Reviewers and operators should prefer those types over free-form interpretations
of backend strings.

## AMD KFD

`amd_kfd` is the lower-risk AMD posture and the first AMD execution substrate
that later served-product work should target.

Expected environment:

- Linux host
- standard `amdgpu` kernel-driver posture
- `/dev/kfd` present
- AMD DRM devices visible under `/sys/class/drm`

Expected readiness states:

- `ready`
  AMD DRM devices are present and `/dev/kfd` exists. In this state Psionic can own
  backend-local staging buffers and explicit fill/copy submissions.
- `degraded`
  AMD hardware is present but `/dev/kfd` is missing, so Psionic must not pretend
  the KFD execution substrate is available.
- `offline`
  no AMD KFD posture is detectable on the host.

Risk posture:

- `requires_explicit_opt_in = false`
- `may_unbind_kernel_driver = false`
- recovery is kernel-driver oriented (`kernel_driver_reset`, then `reboot_host`)

## AMD Userspace

`amd_userspace` is the higher-risk AMD posture.

Opt-in:

- Psionic never enables it silently.
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
  the expected userspace posture and Psionic must not pretend the userspace
  execution substrate is available
- `ready` + `opt_in = enabled`
  AMD devices were found and the kernel-driver handoff was detected. In this
  state Psionic can own backend-local staging buffers and explicit fill/copy
  submissions.

Risk posture:

- `requires_explicit_opt_in = true`
- `may_unbind_kernel_driver = true`
- recovery expects userspace-oriented intervention first
  (`process_restart`, `rebind_kernel_driver`, `reboot_host`)

## Validation

Repo-level validation coverage for the current AMD groundwork:

- `cargo test -p psionic-backend-amd-kfd`
- `cargo test -p psionic-backend-amd-userspace`
- `cargo test -p psionic-provider`

Practical host checks:

- KFD posture:
  verify `/dev/kfd` exists and AMD DRM devices are visible under
  `/sys/class/drm/card*/device`
- userspace posture:
  verify `RUSTYGRAD_AMD_USERSPACE_ENABLE=1` is set intentionally and confirm
  whether `/sys/module/amdgpu` is still loaded

## Review Boundary

If a future change claims AMD served-product execution support, it must add more
than discovery, staging allocation, and explicit copy/fill submissions. This
runbook only covers the current execution-substrate phase and should stay
explicit about that limit.
