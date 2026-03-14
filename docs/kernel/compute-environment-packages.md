# Compute Environment Packages

This document defines the first Rust-native environment package and registry
contract landed for the compute expansion.

## Purpose

Environment packages make evaluation and later training/sandbox environments
first-class kernel objects instead of loose refs buried in metadata.

The retained owner split is:

- `openagents-kernel-core`: canonical object model and wire contracts
- `apps/nexus-control`: authority-managed registry mutations and read models
- future environment/eval services: package production, artifact preparation,
  and run orchestration

## Identity And Versioning

- Stable identity is `environment_ref`
- Immutable package version is `version`
- Canonical storage key is `environment_ref@version`
- Read paths may request a specific version or the latest known version for an
  `environment_ref`

`ComputeEnvironmentBinding` in compute products and delivery proofs continues to
use `environment_ref` plus optional `environment_version`, which lets later
issues bind directly into this registry without another schema change.

## Object Model

`ComputeEnvironmentPackage` currently contains:

- package identity: `environment_ref`, `version`, `family`, `display_name`,
  `owner_id`
- lifecycle: `created_at_ms`, `updated_at_ms`, `status`
- description and integrity: `description`, `package_digest`
- datasets: `dataset_bindings[]`
- execution harness: `harness`
- rubrics: `rubric_bindings[]`
- required outputs: `expected_artifacts[]`
- policy attachment: `policy_refs[]`
- extension surface: `metadata`

Sub-objects:

- `ComputeEnvironmentDatasetBinding`
  - dataset ref, split, mount path, integrity ref, access policy ref,
    required flag, metadata
- `ComputeEnvironmentHarness`
  - harness ref, runtime family, entrypoint, args, sandbox profile ref,
    evaluator policy ref, time budget, metadata
- `ComputeEnvironmentRubricBinding`
  - rubric ref, score type, pass threshold, metadata
- `ComputeEnvironmentArtifactExpectation`
  - artifact kind, optional artifact ref, required flag, verification policy
    ref, metadata

## Validation Rules

The kernel currently enforces:

- non-empty `environment_ref`, `version`, `family`, `display_name`, `owner_id`
- `updated_at_ms >= created_at_ms`
- non-empty optional digest/entrypoint refs when present
- non-empty dataset refs
- non-empty rubric refs
- rubric thresholds bounded to `0..=10000`
- non-empty artifact kinds
- non-empty policy refs when present
- positive harness time budget when present

## Authority Surface

Nexus now exposes:

- `POST /v1/kernel/compute/environments`
- `GET /v1/kernel/compute/environments?family=&status=`
- `GET /v1/kernel/compute/environments/{environment_ref}?version=`

Register operations emit canonical receipts under
`kernel.compute.environment.register`.

The registry persists in kernel state, survives restart, and is available
through the generated proto/contracts path and `HttpKernelAuthorityClient`.

## Reference Package

The retained reference package used in tests is:

- `environment_ref`: `env.openagents.math.basic`
- `version`: `2026.03.13`
- `family`: `evaluation`

It exercises dataset binding, harness config, rubric binding, artifact
expectation, policy refs, digest, and metadata.

## Next Integration Path

- `#3517`: bind `environment_ref` and `environment_version` truth into compute
  products and delivery proofs against this registry
- `#3518`: make eval runs terminate in environment-backed objects
  - landed in `docs/kernel/compute-evaluation-runs.md`
- later synthetic-data and training issues should reuse this package identity
  instead of inventing parallel environment descriptors
