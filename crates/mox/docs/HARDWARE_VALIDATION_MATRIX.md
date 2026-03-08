# Mox Hardware Validation Matrix

This document is the human-readable source for the minimum shipped hardware
validation profile:

- `matrix_id = mox.minimum_hardware_validation.v1`
- machine-readable reference type:
  `mox_runtime::ValidationMatrixReference`

`mox-provider` capability envelopes and receipts now carry a `validation`
object that points back to one row in this matrix. If a backend/product pairing
falls outside the rows below, the surface must serialize
`coverage = not_yet_validated` instead of silently claiming shipped support.

## Coverage Rules

- CPU reference claims are bounded to ordinary host CPU families:
  `x86_64` and `aarch64`.
- Apple Silicon positive claims are bounded to Metal devices that report
  `MTLGPUFamily::Apple1` through `Apple9` or the equivalent `Common3` /
  `Metal3` / `Metal4` capability class in `mox-backend-metal`.
- Legacy-only Metal devices and non-macOS hosts are refusal-only coverage, not
  positive Apple Silicon support claims.
- NVIDIA positive claims are currently limited to the shipped CUDA embeddings
  path on Linux with a usable NVIDIA driver/runtime.
- AMD KFD is currently a discovery/readiness claim only. AMD served-product
  execution remains future work under `MOX-150` through `MOX-154`.
- AMD userspace remains an explicitly gated refusal/degraded lane unless and
  until a later issue expands the shipped execution matrix.

## Minimum Matrix

| Claim ID | Backend | Product | Hardware lane | Coverage | Green tests | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| `cpu.embeddings.reference` | `cpu` | `mox.embeddings` | `x86_64` or `aarch64` host CPU | `positive_execution` | `mox-serve/tests/model_backed_embeddings.rs::model_backed_embeddings_flow_returns_response_capability_and_receipt` | Reference embeddings lane used for parity and fallback truth. |
| `cpu.text_generation.reference` | `cpu` | `mox.text_generation` | `x86_64` or `aarch64` host CPU | `positive_execution` | `mox-serve/tests/model_backed_text_generation.rs::model_backed_text_generation_flow_returns_response_capability_and_receipt` | Reference text-generation lane used for parity and fallback truth. |
| `metal.embeddings.apple_silicon` | `metal` | `mox.embeddings` | Apple Silicon devices exposing `Apple1`-`Apple9` or `Common3`+ | `positive_execution` | `mox-serve/tests/metal_model_backed_embeddings.rs::metal_model_backed_embeddings_flow_returns_response_capability_and_receipt_or_explicit_unavailability`; `mox-serve/tests/metal_embeddings_parity.rs::metal_model_backed_embeddings_match_cpu_baseline_within_tolerance_on_ready_hardware` | First shipped Apple GPU embeddings claim. |
| `metal.text_generation.apple_silicon` | `metal` | `mox.text_generation` | Apple Silicon devices exposing `Apple1`-`Apple9` or `Common3`+ | `positive_execution` | `mox-serve/tests/metal_model_backed_text_generation.rs::metal_model_backed_text_generation_returns_response_capability_and_receipt_or_explicit_unavailability`; `mox-serve/tests/metal_text_generation_parity.rs::metal_text_generation_matches_cpu_baseline_within_budget_and_seeded_sampling` | First shipped Apple GPU text-generation claim. |
| `metal.refusal.off_platform` | `metal` | n/a | non-macOS hosts or legacy-only Metal devices | `explicit_refusal` | `mox-serve/tests/metal_embeddings_parity.rs::metal_model_backed_embeddings_parity_reports_explicit_offline_state`; `mox-serve/tests/metal_text_generation_parity.rs::metal_text_generation_parity_reports_explicit_offline_state` | Metal support must fall back or refuse explicitly instead of overclaiming readiness. |
| `cuda.embeddings.nvidia` | `cuda` | `mox.embeddings` | Linux host with a usable NVIDIA CUDA device | `positive_execution` | `mox-serve/tests/cuda_model_backed_embeddings.rs::cuda_model_backed_embeddings_flow_returns_response_capability_and_receipt_or_explicit_unavailability`; `mox-serve/tests/cuda_embeddings_parity.rs::cuda_model_backed_embeddings_match_cpu_baseline_within_tolerance_or_report_explicit_fallback` | First shipped NVIDIA served-product claim. |
| `cuda.refusal.unavailable` | `cuda` | n/a | host without usable NVIDIA driver/runtime | `explicit_refusal` | `mox-serve/tests/cuda_embeddings_parity.rs::cuda_model_backed_embeddings_match_cpu_baseline_within_tolerance_or_report_explicit_fallback` | CUDA support must degrade/fallback explicitly when NVIDIA execution is unavailable. |
| `amd_kfd.discovery` | `amd_kfd` | n/a | Linux AMD host on the standard KFD/amdgpu lane | `discovery_readiness` | `mox-backend-amd-kfd/src/lib.rs::amd_kfd_report_is_self_consistent_on_linux` | Discovery/readiness only until AMD execution lands. |
| `amd_userspace.refusal` | `amd_userspace` | n/a | Linux AMD userspace lane with opt-in disabled or kernel-driver handoff incomplete | `explicit_refusal` | `mox-backend-amd-userspace/src/lib.rs::userspace_health_is_offline_when_disabled`; `mox-provider/src/lib.rs::amd_userspace_capability_reports_disabled_risk_posture` | Elevated-risk lane stays explicitly gated. |

## Lab Runbook

Run the matrix by host class instead of pretending one machine can prove every
claim.

### Any host

- `cargo test -p mox-runtime -p mox-provider`
- `cargo test -p mox-backend-amd-kfd -p mox-backend-amd-userspace`
- `cargo test -p mox-serve --test model_backed_embeddings --test model_backed_text_generation`

### Apple Silicon macOS host

- `cargo test -p mox-backend-metal`
- `cargo test -p mox-serve --test metal_embeddings_parity --test metal_model_backed_embeddings --test metal_text_generation_parity --test metal_model_backed_text_generation`

### Linux NVIDIA host

- `cargo test -p mox-backend-cuda`
- `cargo test -p mox-serve --test cuda_embeddings_parity --test cuda_model_backed_embeddings`

### Common hygiene

- `scripts/lint/ownership-boundary-check.sh`
- `git diff --check`

## Update Rule

Whenever a backend/product claim changes:

1. update the machine-readable claim mapping in `mox-runtime`
2. update this document in the same issue
3. update or add the tests named in the affected row
4. ensure provider capability/receipt surfaces point at the new claim

Do not advertise a new backend/product lane as shipped until it has a row here
and the referenced tests are green.
