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

## Apple Adapter Metadata Discipline

The authority objects remain generic overall, but the kernel now reserves one
typed metadata extension for the Apple adapter lane:

- `metadata.apple_adapter`

The nested payload is parsed into typed Rust contracts before persistence for:

- `ComputeBenchmarkPackage`
  - ABI version `compute.apple_adapter_benchmark_package.v1`
- `ComputeTrainingPolicy`
  - ABI version `compute.apple_adapter_training_policy.v1`
- `ComputeTrainingRun`
  - ABI version `compute.apple_adapter_training_run.v1`

These typed payloads carry the Apple lineage and admissibility anchors that the
generic registry fields do not model directly:

- base-model signature
- tokenizer digest
- `.fmadapter` package-format version
- canonical environment ref
- canonical benchmark-package refs
- validator policy ref
- draft-model presence
- Apple runtime-validation posture

For Apple benchmark packages the typed payload also carries:

- benchmark environment group and core-environment refs
- admitted Apple sample kinds for the package

This keeps the top-level authority objects generic while preventing later Apple
training receipts from surviving as opaque or nullable JSON blobs.

## Run And Outcome Objects

The authority now also manages:

- `ComputeTrainingRun`
  - canonical train run id, policy refs, environment binding, checkpoint
    binding, validator posture, benchmark package refs, source linkage, step
    expectations, terminal summaries, and checkpoint promotion refs
- `ComputeAdapterTrainingWindow`
  - canonical decentralized-adapter window projection linked to one
    `ComputeTrainingRun`, including lifecycle state, validator score summary,
    promotion readiness, promotion lineage, and optional accepted-outcome
    linkage
- `ComputeAdapterContributionOutcome`
  - canonical contribution-level projection linked to one
    `ComputeAdapterTrainingWindow`, including manifest/object digests, validator
    disposition, aggregation eligibility, aggregation weight, and preserved
    submission/artifact/provenance/security/validator receipt digests
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

The decentralized adapter objects are the durable authority bridge from Psionic
execution receipts into operator and settlement-facing truth:

- Psionic still owns assignment, upload, security, validator replay, and
  aggregation execution receipts
- Kernel and Nexus now persist the window and contribution projections derived
  from those receipts
- accepted outcomes remain the only canonical promotion boundary for later
  market settlement or accepted operator claims

## Current Apple Operator Path

The retained Apple adapter path now has a concrete operator-to-authority flow
in `apps/autopilot-desktop`:

1. desktop control or `autopilotctl` launches a repo-native Apple adapter run
2. the run stages one `.fmadapter` package locally
3. held-out eval plus runtime-smoke checks execute before authority acceptance
4. export is optional operator materialization, not canonical market truth
5. acceptance registers the canonical environment, benchmark, validator,
   checkpoint-family, training-policy, eval-run, training-run, and
   accepted-outcome records

That means the repo now has a truthful boundary between:

- local artifact success
- and accepted authority truth

The important rule is simple:

- exported Apple adapters are not economic truth by themselves
- accepted training outcomes are

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
- Apple benchmark packages additionally require:
  - a known benchmark adapter kind
  - typed `metadata.apple_adapter`
  - the Apple environment ref to match the top-level environment binding
  - the required metric set for the declared Apple sample kinds
- Apple training policies additionally require typed `metadata.apple_adapter`
  with environment, benchmark-package, and validator refs that match the
  top-level policy object

### Training run creation

- training-run creation requires an existing training policy
- the checkpoint family must match the training policy
- the validator policy must match the training policy
- the environment must be permitted by the training policy
- benchmark packages must exist and match the resolved environment
- Apple benchmark packages may satisfy that environment check either by matching
  the run's top-level environment directly or, for benchmark-split packages, by
  carrying `metadata.apple_adapter.core_environment_ref` that matches the run's
  resolved core environment
- Nexus resolves and locks `environment_version`
- only non-terminal initial statuses are accepted on create
- Apple training runs additionally require typed `metadata.apple_adapter`
  whose environment ref, benchmark-package refs, and validator policy ref
  match the top-level run object before authority persistence

### Training run finalize

- finalize requires an existing non-terminal training run
- only terminal statuses are allowed on finalize
- accepted runs require `final_checkpoint_ref`
- terminal summary state is stored on the run and emitted in the finalize
  receipt
- when finalize metadata includes `apple_adapter`, Nexus promotes that payload
  back into the canonical run metadata so the final package digest and eval or
  runtime-validation refs live on the stored `ComputeTrainingRun`, not only in
  an opaque finalize sidecar
- accepted Apple runs require the typed package digest and held-out eval ref,
  plus the runtime-validation eval ref when the Apple runtime-validation
  posture requires runtime smoke

### Accepted outcomes

- evaluation outcomes require a finalized `ComputeEvaluationRun`
- training outcomes require an accepted `ComputeTrainingRun`
- Nexus hydrates the canonical environment binding from the source run
- Nexus hydrates training checkpoint or validator posture from the accepted
  training run
- if the caller omitted the final summary, Nexus copies it from the source run
- accepted outcomes are durable, typed read models rather than log-only events
- Apple training outcomes are published only after the held-out eval run meets
  the benchmark threshold and, when the Apple runtime-validation posture
  requires it, a separate runtime-smoke eval run is finalized with positive
  runtime-smoke evidence
- for accepted Apple training outcomes, Nexus also copies typed
  `metadata.apple_adapter` into the accepted outcome so the package digest,
  held-out eval ref, and runtime-validation eval ref remain available to later
  projections without inventing a second local truth source

### Decentralized adapter windows and contributions

- recording one adapter window requires an existing `ComputeTrainingRun`
- the window validator policy must match the source training run validator
  policy
- each contribution recorded with the window must bind to the same:
  - training run
  - stage id
  - window id
  - contributor-set revision
  - adapter target lineage
  - source policy revision
  - source checkpoint pointer
- each contribution projection preserves:
  - submission receipt digest
  - artifact receipt digest
  - provenance bundle digest
  - security receipt digest
  - optional replay receipt digest
  - validator receipt digest
- each window projection preserves:
  - lifecycle status (`planned` through `reconciled`)
  - contribution counts and validator score summary
  - promotion readiness and gate reasons
  - promotion disposition and hold reasons
  - optional promoted policy revision and checkpoint pointer
  - optional accepted-outcome id once canonical outcome acceptance has happened
- re-recording the same window id replaces the current contribution projection
  set for that window, so one authority record can move from provisional window
  truth to accepted-outcome-linked truth without inventing parallel local state

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
- `POST /v1/kernel/compute/training/adapter-windows`
- `GET /v1/kernel/compute/training/adapter-windows?training_run_id=&status=`
- `GET /v1/kernel/compute/training/adapter-windows/{window_id}`
- `GET /v1/kernel/compute/training/adapter-contributions?training_run_id=&window_id=&disposition=`
- `GET /v1/kernel/compute/training/adapter-contributions/{contribution_id}`
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
- `kernel.compute.adapter_window.record.v1`
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
- adapter window record plus window/contribution list/get
- accepted-outcome accept/list/get

This replaces ad hoc JSON request construction for the authority-facing
training and accepted-outcome bridge.

## Regression Coverage

The landed regression coverage now includes:

- kernel validation tests for registry, training-run, and accepted-outcome
  contracts in `openagents-kernel-core`
- Apple-specific kernel validation tests for malformed benchmark-package,
  training-policy, and training-run metadata in `openagents-kernel-core`
- Nexus state-machine tests covering eval and training accepted-outcome flows
- Nexus state-machine tests covering adapter-window and contribution persistence
- generated-contract HTTP roundtrip coverage through `HttpKernelAuthorityClient`
  for all new registries, training runs, adapter windows, contributions, and
  accepted outcomes

## Relation To Psionic

Psionic still owns the runtime truth for:

- checkpoints and recovery
- validator simulation and verdict generation
- benchmark execution
- train and eval artifact production

Kernel and Nexus now own the canonical authority publication layer that turns
those runtime results into durable policy, receipt, and accepted-outcome truth.
