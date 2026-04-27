# Compute Training Run Definition Contract

Status: active  
Date: 2026-04-11

This document defines the canonical run-definition object for the Transcript
222 launch-hardening program.

It exists to answer one narrow question: when Nexus says a node was admitted to
or assigned work for one training program, what stable object describes that
program?

The answer is `ComputeTrainingRunDefinition`.

This contract extends the existing Nexus control-plane records. It does not add
a second run-definition registry.
The matching artifact-identity layer that windows, manifests, and closeouts
should use lives in
[`compute-training-artifact-resolver-contract.md`](./compute-training-artifact-resolver-contract.md).

## Why This Exists

Transcript 222 needs one stable object that binds:

- training family
- objective
- sync profile
- Psionic lane id and release id, when the policy is backed by a named
  Psionic lane
- checkpoint family
- environment references
- validator policy
- dataset identity
- fixed tokenizer, tokenized dataset, and validation-set digests for lanes
  that freeze those inputs
- model, optimizer, scheduler, aggregation, and base-checkpoint refs for lanes
  that need exact replay or resume claims
- dataset slice or page-proof family
- benchmark package set
- version semantics

Without that object, run windows, manifests, closeouts, public stats, and
later TRN publication all have to infer "what run are we in?" from scattered
policy fields.

## Canonical Object

The machine-readable schema is
`openagents_kernel_core::compute::ComputeTrainingRunDefinition`.

The durable source record is:

- `ComputeTrainingPolicy.metadata["run_definition"]`

That metadata must decode as:

- `ComputeTrainingRunDefinitionMetadata`
- `abi_version = "compute.training_run_definition.v1"`

The resolved object is produced by Nexus from the existing registry state:

- `ComputeTrainingPolicy`
- `ComputeEnvironmentPackage`
- `ComputeValidatorPolicy`
- `ComputeBenchmarkPackage`

That keeps the run definition inside the control-plane that already exists for
training policy, environment, validator, and benchmark truth.

## Read Path

Nexus now exposes one authenticated read path:

- `GET /v1/kernel/compute/training/policies/{training_policy_ref}/run-definition`

Supported query:

- `version=<training_policy_version>`

The route returns JSON encoded `ComputeTrainingRunDefinition`.

The route is implemented in:

- `apps/nexus-control/src/lib.rs`
- `apps/nexus-control/src/kernel.rs`

## Field Mapping

The run definition is a projection over existing durable records.

### Training policy metadata

These fields come from `ComputeTrainingRunDefinitionMetadata` stored under
`ComputeTrainingPolicy.metadata["run_definition"]`:

- `run_definition_ref`
- `training_family`
- `objective`
- `sync_profile`
- `lane_id`
- `lane_release_id`
- `environment_ref`
- `dataset_identity`
- `tokenizer_digest`
- `tokenized_dataset_digest`
- `validation_set_digest`
- `dataset_slice_family`
- `page_proof_family`
- `benchmark_package_set_ref`
- `benchmark_package_refs`
- `model_config_digest`
- `optimizer_config_digest`
- `scheduler_config_digest`
- `aggregation_rule_ref`
- `aggregation_weight_basis_ref`
- `checkpoint_family_ref`
- `base_checkpoint_ref`
- `validator_policy_ref`
- `version_semantics`
- `window_ref_family`
- `manifest_ref_family`
- `trn_ref_family`
- `closeout_ref_family`

These are the fields that explain the intended training program rather than the
runtime instance of one window.

For `training_family = "a1_minimal_distributed_lm"`, Nexus now treats the
fixed identity fields as required. The policy is invalid unless it includes the
Psionic lane id, release id, environment ref, tokenizer digest, tokenized
dataset digest, validation-set digest, model config digest, optimizer config
digest, scheduler config digest, aggregation rule, aggregation weight basis,
checkpoint family, base checkpoint, validator policy, and benchmark package
refs. The checkpoint family, validator policy, environment ref, and benchmark
package refs must match the owning `ComputeTrainingPolicy`.

### Training policy record

These fields come from `ComputeTrainingPolicy` itself:

- `training_policy_ref`
- `training_policy_version`
- `checkpoint_family`
- `validator_policy_ref`
- `environment_refs`
- `benchmark_package_refs`

The training policy remains the durable record that owns the admission and
execution posture. The run definition gives that policy a stable run identity
and dataset/training vocabulary.

### Validator policy

Nexus resolves `validator_policy_ref` through the existing validator-policy
registry and returns:

- `validator_policy_ref`
- `validator_policy_version`

This keeps closeout and acceptance semantics anchored to the same validator
contract already used elsewhere in Nexus.

### Environment packages

For each `environment_ref` on the training policy, Nexus resolves the existing
`ComputeEnvironmentPackage` record and projects:

- `environment_ref`
- `version`
- `family`
- `display_name`
- `package_digest`
- `dataset_bindings`
- `policy_refs`

The run definition also flattens the environment-level dataset bindings into a
top-level `dataset_bindings` array so manifests and operator tooling can read
the bound dataset contract without re-walking every environment package.

Each flattened dataset binding carries:

- `environment_ref`
- `environment_version`
- `dataset_ref`
- `split_ref`
- `mount_path`
- `integrity_ref`
- `access_policy_ref`
- `required`
- `metadata`

### Dataset identity and proof families

The run definition intentionally does not create a second dataset-truth system.

Dataset truth stays aligned with the existing Psionic surfaces:

- `psionic/docs/PUBLIC_DATASET_AUTHORITY_REFERENCE.md`
- `psionic/crates/psionic-train/src/psion_actual_pretraining_data_bundle.rs`
- `psionic/crates/psionic-train/src/adapter_cluster.rs`

The mapping is:

- `dataset_identity` names the canonical dataset family for the run
- `dataset_slice_family` names the slice or shard lineage used by manifests and
  assignment bundles
- `page_proof_family` names the proof family used when the run depends on
  page-level replay or authority proofs
- `integrity_ref` on individual dataset bindings points at the existing
  dataset-authority or integrity contract for the mounted dataset material

That means Nexus owns the public run-definition object, while Psionic remains
the source of truth for the dataset-proof and training-bundle mechanics.

### Benchmark packages

For each `benchmark_package_ref` on the training policy, Nexus resolves the
existing benchmark registry and projects:

- `benchmark_package_ref`
- `version`
- `family`
- `environment_ref`
- `environment_version`
- `artifact_refs`

`benchmark_package_set_ref` remains the run-level handle for the intended eval
set family, while `benchmark_packages` exposes the resolved concrete package
records that validators and operator tooling can load.

### Reference families

`reference_families` groups the ref prefixes or families that later training
objects should use:

- `window_ref_family`
- `manifest_ref_family`
- `trn_ref_family`
- `closeout_ref_family`

These are naming and linkage contracts, not runtime state.

## Example Shape

```json
{
  "schema_version": "compute.training_run_definition.v1",
  "run_definition_ref": "run.psion.pretrain.v1",
  "training_policy_ref": "policy.training.psion.pretrain",
  "training_policy_version": "2026.03.14",
  "training_family": "psion_actual_pretraining",
  "objective": "next_token_prediction",
  "sync_profile": "validation_replay",
  "lane_id": "psion_actual_pretraining_v1",
  "lane_release_id": "psionic-train.psion_actual_pretraining.release.v1",
  "environment_ref": "psionic.environment.psion_actual_pretraining.cuda_h100.operator@v1",
  "checkpoint_family": "checkpoint.psion.actual",
  "checkpoint_family_ref": "checkpoint.psion.actual",
  "base_checkpoint_ref": "checkpoint://psion/base",
  "validator_policy_ref": "policy.validator.training",
  "validator_policy_version": "2026.03.14",
  "dataset_identity": "dataset.psion.public-corpus.v1",
  "tokenizer_digest": "sha256:...",
  "tokenized_dataset_digest": "sha256:...",
  "validation_set_digest": "sha256:...",
  "dataset_slice_family": "slice.psion.public-corpus",
  "page_proof_family": "proof.psion.public-pages",
  "benchmark_package_set_ref": "benchmark-set.psion.reference",
  "benchmark_package_refs": ["benchmark.psion.reference.validation_loss"],
  "model_config_digest": "sha256:...",
  "optimizer_config_digest": "sha256:...",
  "scheduler_config_digest": "sha256:...",
  "aggregation_rule_ref": "trusted_weighted_delta_average_v1",
  "aggregation_weight_basis_ref": "accepted_tokens_processed_v1",
  "version_semantics": "date_versioned_policy_projection",
  "reference_families": {
    "window_ref_family": "window.psion.pretrain",
    "manifest_ref_family": "manifest.psion.pretrain",
    "trn_ref_family": "trn.psion.pretrain",
    "closeout_ref_family": "closeout.psion.pretrain"
  }
}
```

## Storage Rules

The run definition is not its own mutable registry object.

The storage contract is:

1. operators register or update `ComputeTrainingPolicy`
2. the policy includes `metadata["run_definition"]`
3. linked environment, validator, and benchmark records stay in their existing
   registries
4. Nexus projects the resolved run definition on read

This is deliberate. The launch-hardening plan explicitly avoids creating a
parallel runtime, payout, stats, visualization, or run-definition path.

## How Later Objects Should Reference It

This issue does not retrofit every downstream training object yet, but the
contract is now explicit:

- windows should carry `run_definition_ref` or a stable link back to the owning
  `training_policy_ref`
- manifests should carry the same `run_definition_ref` and the relevant
  `dataset_slice_family`
- TRN publication should use the run-definition families rather than inventing
  a second training vocabulary
- closeouts should record the `run_definition_ref` used when the work was
  admitted and accepted

Training-window metadata can now carry a `run_definition` block alongside
`pylon_training_window`. That block snapshots the resolved
`run_definition_ref`, `training_policy_ref`, policy version, training family,
sync profile, lane id, checkpoint family, validator policy, tokenizer digest,
tokenized dataset digest, validation-set digest, model/optimizer/scheduler
config digests, aggregation rule, aggregation weight basis, and base checkpoint
used when the window was planned. TRN window publication copies that block into
event content and emits a `run_definition` tag when the ref is present.
Closeout metadata copies the same block and top-level `run_definition_ref`
from the window. Pylon run manifests can now carry an additive
`run_definition_ref` field so manifest artifacts can point back to the same
canonical policy projection.

## A1 Minimal Distributed LM

The first concrete consumer is `a1_minimal_distributed_lm_001`.

Canonical OpenAgents policy refs:

- training policy: `policy.training.a1_minimal_distributed_lm.001`
- run definition: `rundef.a1_minimal_distributed_lm.001.v1`
- validator policy: `policy.validator.a1_minimal_distributed_lm.v1`
- environment:
  `psionic.environment.a1_minimal_distributed_lm.tiny_lm.operator@v1`
- checkpoint family: `psion.a1_minimal_distributed_lm.checkpoints.v1`
- benchmark package:
  `benchmark://a1-minimal-distributed-lm/validation-loss-v1`

Frozen Psionic refs:

- lane id: `a1_minimal_distributed_lm_001`
- release id: `psionic-train.a1_minimal_distributed_lm.release.v1`
- tokenizer digest:
  `sha256:e32b619b67029aba5de26391f9a5f4a32801220ca690ae2c89d565e61069cf63`
- tokenized dataset digest:
  `sha256:f0b92dc6301fc72e05a4ead6d85a4b5706e51267c116ecd72025a90c43a37905`
- validation-set digest:
  `sha256:6c1c6ca83a2d8eca6cb133f3ec719e822f134452723e72e5201407b28cd3d228`
- model config digest:
  `sha256:2cbe248294de6fa34c32ce3eb0195a661004062e9d8f450317118e854f89db8a`
- optimizer config digest:
  `sha256:ced6a7d4aacda7721920575e37f7a235d6650c20cbfd26666c43018354b6cca7`
- scheduler config digest:
  `sha256:aac8693277a49447d56b889cebe327c175690abc2240aa09b8b379d87d6f28af`
- aggregation rule: `trusted_weighted_delta_average_v1`
- aggregation weight basis: `accepted_tokens_processed_v1`
- base checkpoint: `base://a1_minimal_distributed_lm/step-000000`

This is a tiny fixed LM run contract for local-update and verifier/support
work. It does not claim OpenWebText leaderboard parity, model-size leadership,
token-budget leadership, broad pretraining, or permissionless model-progress
training.

That gives Transcript 222 a single answer to "what run did this node actually
work on?" even before all downstream issue work lands.

## Implementation Notes

The current implementation adds:

- schema and validation helpers in
  `crates/openagents-kernel-core/src/compute.rs`
- run-definition resolution in `apps/nexus-control/src/kernel.rs`
- the authenticated read route in `apps/nexus-control/src/lib.rs`

The targeted route test is:

- `kernel_compute_training_run_definition_route_projects_registered_contracts`

## Non-Goals

This change does not:

- add a second registry table for runs
- change runtime assignment intake
- change payout logic
- publish TRN records yet
- replace Psionic dataset-authority or training-bundle contracts

Those are handled by later launch-hardening issues.
