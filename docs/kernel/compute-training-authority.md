# Compute Training And Accepted Outcome Authority

This document defines the kernel-owned authority surface for training-adjacent
compute objects and accepted train or eval outcomes.

It extends the earlier evaluation-run lifecycle in
`docs/kernel/compute-evaluation-runs.md` with the registry and read-model
objects needed once train and eval artifacts become authority-facing economic
objects.

## Purpose

Psionic owns train, eval, checkpoint, and validator execution truth.

Kernel and Nexus own the authority-facing record of:

- which checkpoint family policy is canonical
- which validator policy is canonical
- which benchmark package is canonical
- which training policy is canonical
- which training run receipts are accepted into durable authority truth
- which finalized eval or accepted training runs have become accepted outcomes

The owner split is:

- `crates/psionic/*`
  - runtime, sandbox, datastream, eval, and train execution truth
- `crates/openagents-kernel-core`
  - typed object model, validation rules, generated proto contracts, typed Rust
    authority client
- `apps/nexus-control`
  - durable mutation authority, persistence, receipt emission, and read models

## Registry Objects

The authority now manages these versioned registries:

- `ComputeCheckpointFamilyPolicy`
  - canonical checkpoint family id, recovery posture, allowed environments,
    validator-policy binding, and retention-policy linkage
- `ComputeValidatorPolicy`
  - canonical validator pool, minimum validator count, challenge window,
    required proof posture, and benchmark-package linkage
- `ComputeBenchmarkPackage`
  - canonical benchmark package family, environment binding, adapter kind,
    evaluator policy, pass threshold, required metrics, and artifact refs
- `ComputeTrainingPolicy`
  - canonical training-policy family, allowed environments, checkpoint family,
    validator policy, benchmark package set, and stage-policy refs

All four registries are:

- versioned
- status-bearing through `ComputeRegistryStatus`
- durable across restart
- readable through list/get surfaces
- emitted through typed proto contracts and the typed `HttpKernelAuthorityClient`

## Run And Outcome Objects

The authority now also manages:

- `ComputeTrainingRun`
  - canonical train run id, policy refs, environment binding, checkpoint
    binding, validator posture, benchmark package refs, source linkage, step
    expectations, terminal summaries, and checkpoint promotion refs
- `ComputeAcceptedOutcome`
  - canonical accepted-outcome id for either:
    - one finalized `ComputeEvaluationRun`
    - one accepted `ComputeTrainingRun`

Accepted outcomes are the bridge into higher-level market or operator truth.

They keep one machine-legible accepted record with:

- accepted outcome kind
- source run id
- resolved environment binding
- optional checkpoint binding
- optional validator policy ref
- benchmark package refs
- accepted-at timestamp
- canonical evaluation or training summary

## Lifecycle And Binding Rules

### Registry registration

- checkpoint family policy registration validates checkpoint-family identity,
  version, and environment allowances
- validator policy registration validates pool identity, validator-count rules,
  and benchmark refs
- benchmark package registration resolves and locks `environment_version`
  against the environment registry
- training policy registration requires the referenced checkpoint family,
  validator policy, benchmark packages, and environments to exist

### Training run creation

- training-run creation requires an existing training policy
- the checkpoint family must match the training policy
- the validator policy must match the training policy
- the environment must be permitted by the training policy
- benchmark packages must exist and match the resolved environment
- Nexus resolves and locks `environment_version`
- only non-terminal initial statuses are accepted on create

### Training run finalize

- finalize requires an existing non-terminal training run
- only terminal statuses are allowed on finalize
- accepted runs require `final_checkpoint_ref`
- terminal summary state is stored on the run and emitted in the finalize
  receipt

### Accepted outcomes

- evaluation outcomes require a finalized `ComputeEvaluationRun`
- training outcomes require an accepted `ComputeTrainingRun`
- Nexus hydrates the canonical environment binding from the source run
- Nexus hydrates training checkpoint or validator posture from the accepted
  training run
- if the caller omitted the final summary, Nexus copies it from the source run
- accepted outcomes are durable, typed read models rather than log-only events

## HTTP Authority Surface

Nexus now exposes:

- `POST /v1/kernel/compute/checkpoints/policies`
- `GET /v1/kernel/compute/checkpoints/policies?status=`
- `GET /v1/kernel/compute/checkpoints/policies/{checkpoint_family}?version=`
- `POST /v1/kernel/compute/validators/policies`
- `GET /v1/kernel/compute/validators/policies?validator_pool_ref=&status=`
- `GET /v1/kernel/compute/validators/policies/{policy_ref}?version=`
- `POST /v1/kernel/compute/benchmarks/packages`
- `GET /v1/kernel/compute/benchmarks/packages?family=&environment_ref=&status=`
- `GET /v1/kernel/compute/benchmarks/packages/{benchmark_package_ref}?version=`
- `POST /v1/kernel/compute/training/policies`
- `GET /v1/kernel/compute/training/policies?environment_ref=&status=`
- `GET /v1/kernel/compute/training/policies/{training_policy_ref}?version=`
- `POST /v1/kernel/compute/training/runs`
- `GET /v1/kernel/compute/training/runs?training_policy_ref=&environment_ref=&status=`
- `GET /v1/kernel/compute/training/runs/{training_run_id}`
- `POST /v1/kernel/compute/training/runs/{training_run_id}/finalize`
- `POST /v1/kernel/compute/outcomes`
- `GET /v1/kernel/compute/outcomes?outcome_kind=&environment_ref=`
- `GET /v1/kernel/compute/outcomes/{outcome_id}`

## Receipt Families

The canonical new receipt families are:

- `kernel.compute.checkpoint_policy.register.v1`
- `kernel.compute.validator_policy.register.v1`
- `kernel.compute.benchmark_package.register.v1`
- `kernel.compute.training_policy.register.v1`
- `kernel.compute.training_run.create.v1`
- `kernel.compute.training_run.finalize.v1`
- `kernel.compute.outcome.accept.v1`

These receipts are emitted through normal Nexus mutation handling and survive
replay-safe restart in the same state snapshot as their corresponding read
models.

## Typed Client Surface

The `KernelAuthority` trait and `HttpKernelAuthorityClient` now expose typed
Rust request builders and response parsing for:

- all four registry registration flows
- list/get reads for those registries
- training run create/finalize/list/get
- accepted-outcome accept/list/get

This replaces ad hoc JSON request construction for the authority-facing
training and accepted-outcome bridge.

## Regression Coverage

The landed regression coverage now includes:

- kernel validation tests for registry, training-run, and accepted-outcome
  contracts in `openagents-kernel-core`
- Nexus state-machine tests covering eval and training accepted-outcome flows
- generated-contract HTTP roundtrip coverage through `HttpKernelAuthorityClient`
  for all new registries, training runs, and accepted outcomes

## Relation To Psionic

Psionic still owns the runtime truth for:

- checkpoints and recovery
- validator simulation and verdict generation
- benchmark execution
- train and eval artifact production

Kernel and Nexus now own the canonical authority publication layer that turns
those runtime results into durable policy, receipt, and accepted-outcome truth.
