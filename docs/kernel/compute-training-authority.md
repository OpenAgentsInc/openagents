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

- `OpenAgentsInc/psionic`
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
    binding, validator posture, work class, replica type, benchmark package
    refs, source linkage, step expectations, terminal summaries, and checkpoint
    promotion refs
- `ComputeAdapterTrainingWindow`
  - canonical training-window projection linked to one `ComputeTrainingRun`,
    including lifecycle state, validator score summary, promotion readiness,
    promotion lineage, declared work class, declared replica type, round index,
    base-checkpoint lineage, planned local-work semantics, aggregation rule and
    weighting basis, optional accepted aggregate identity, optional promoted
    checkpoint identity, and optional accepted-outcome linkage
- `ComputeAdapterContributionOutcome`
  - canonical contribution-level projection linked to one
    `ComputeAdapterTrainingWindow`, including manifest/object digests, validator
    disposition, aggregation eligibility, contribution work class, contribution
    replica type, base-checkpoint lineage, local-step or token or example
    accounting, aggregation weight basis and value, and preserved
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

The current type names still carry the historical adapter-first naming, but the
authority semantics no longer do. Windows and contributions now carry explicit
round, checkpoint, local-work, and aggregation fields directly so the same
sealed closeout path can describe adapter windows, grouped stages, or island
local-update rounds without inferring those semantics from opaque metadata.

## Operator Summary And Public Stats Projection

Nexus now projects training state into two explicit read-model buckets instead
of one blended counter set:

- participation
  - admitted nodes
  - online nodes
  - active runs
  - active windows
  - pending-validation windows
  - open or queued validator challenges
- progress
  - runs with accepted progress
  - accepted closeouts
  - nodes that contributed to accepted progress
  - windows that advanced checkpoint lineage
  - payout-eligible closeouts
  - checkpoint-age and artifact-failure signals

This split is deliberate.

- participation answers who is present and what coordination load is live
- progress answers what actually produced accepted state

The public `/stats`, `/api/stats`, `/api/training/summary`, and homepage
snapshots should expose both categories without collapsing them into one
"activity" number. A rewarded or accepted closeout can exist without advancing
checkpoint lineage, and a busy validator queue can exist without any accepted
progress.

## Visualization Snapshot

Nexus now also exposes one visualization-oriented training read model at:

- `/api/training/visualization`

The homepage payload mirrors the same object at:

- `/api/homepage.training_visualization`

This snapshot is the public/operator surface intended for WGPUI and future
homepage or stats-page visualizations. It keeps the stable counters from
`/api/training/summary`, but widens the shape so consumers do not need to
reconstruct training state from scheduler internals.

The visualization snapshot now projects:

- capability-tier buckets
  - node totals, online totals, eligible totals
  - role mix, networks, backend families, throughput bands, replay capability,
    and upload-latency classes per tier
- run state
  - work class, replica type, progress class
  - required worker, validator, and recovery-source tiers
  - active window ids, latest aggregate ref, and latest promoted checkpoint ref
- window state
  - round index, base checkpoint, planned local-step count
  - aggregation rule and weighting basis
  - validator pressure per window
  - aggregate digest or aggregate id
  - output or promoted checkpoint refs
  - accepted closeout linkage, payout eligibility, and contributor tier mix
- validator state
  - open, queued, leased, retrying, verified, rejected, and timed-out counts
  - per-window challenge grouping when the challenge id binds to one training
    window
- aggregate, checkpoint, and closeout projections
  - aggregate refs with closeout status, payout eligibility, and
    weak-device-bearing flags
  - checkpoint refs by role such as `base`, `accepted_closeout`, `promoted`,
    and `run_latest`
  - closeouts with work class, progress class, payout basis, payout projection,
    contributor tiers, weak-device-bearing state, and accepted checkpoint refs

This is the authority-facing answer to the UI request for:

- tier visualization
- active windows and validation pressure
- accepted aggregates and promoted checkpoints
- participation versus progress
- payout references that stay legible after closeout

## Named Run Detail Snapshot

Nexus now also exposes one public named-run detail read model at:

- `/api/training/runs/{training_run_id}`

This endpoint is the website-oriented companion to `/api/training/summary` and
`/api/training/visualization`.

It exists so public consumers can render one proof-grade run page without
needing authenticated kernel access or private reconstruction logic.

The named run detail snapshot projects:

- one normalized run record
  - run status, work class, progress class, replica type, and current window
  - assigned, accepted, and model-progress contributor counts
  - latest checkpoint, aggregate, and promoted-checkpoint refs
- one featured window
  - current window when present, otherwise the latest run window
  - full public window state including acceptance, payout-eligibility, and
    checkpoint-lineage fields
- full public window history for the named run
- contribution rows
  - contribution, assignment, worker, and contributor identity
  - validator disposition and aggregation eligibility
  - manifest, object, provenance, validator, replay, and promotion digests
- participating node rows
  - admitted node identity, role claims, release/build metadata, eligibility,
    and latest run/window linkage
- queue pressure, launch-health, and treasury caveats that materially affect
  interpretation of the run

This endpoint is intended for proof pages such as public homework-run
visualization on `openagents.com`, where consumers need one stable record that
answers:

- what run is this
- which window is the current or proving window
- which hosts contributed
- what artifacts and digests identify the accepted work
- what public caveats still apply to settlement or validation

## Work-Class Settlement Projection

The accepted-outcome closeout record now also carries explicit settlement
projection metadata so operators do not need private runbook knowledge to infer
why a rewarded window paid anyone.

Each training closeout should now publish:

- `work_class`
  - for example `validation_replay`, `grouped_replica_stage_execution`, or
    `full_island_local_update_training`
- `replica_type`
  - `single_node`, `island`, or `grouped_replica`
- `progress_class`
  - `participation_only`, `model_update`, or `checkpoint_advance`
- `payout_projection`
  - machine-legible payout basis such as:
    - `validator_verdict`
    - `accepted_contribution`
    - `aggregation_weight`
    - `grouped_stage_share`
    - `aggregate_acceptance`
    - `checkpoint_authority`
  - optional weighting basis and total weighted value
  - whether one accepted result is shared across multiple contributors
  - one projected participant list with contribution identity and share basis
- `contributor_tiers`
  - minimum and maximum admitted contributor capability tiers counted for
    settlement
  - per-tier participant counts for the accepted outcome
  - one `weak_device_bearing` flag when the accepted settlement includes any
    contributor below `tier3_island`

This is the public authority answer to two different questions that were
previously conflated:

- did this work advance model state?
- did this work earn payout?

For example:

- validation replay can now be payout-eligible while remaining
  `participation_only`
- grouped replica stage execution can now surface one shared accepted result
  with split attribution across multiple nodes and expose whether the accepted
  lane actually included weaker consumer-device tiers
- aggregation or checkpoint-promotion lanes can be payout-eligible and still be
  classified as checkpoint-advance work rather than raw local training

The top-level training summary should therefore expose:

- progress-only counts
  - accepted progress closeouts
  - runs with accepted progress
  - nodes contributing to accepted progress
- settlement counts
  - accepted closeouts regardless of progress class
  - payout-eligible closeouts regardless of progress class
  - work-class breakdowns so participation-only payout lanes stay legible

## Launch default weak-device lane

For the current Transcript 222 launch-hardening window, the default weak-device
lane is `validation_replay`.

That launch freeze means the authority, stats, and payout surfaces should treat
`validation_replay` as the first weak-device work class that is expected to:

- admit lower-tier nodes than the dense training lane
- produce real retained validator artifacts and receipts
- count toward assigned and accepted weak-device work
- remain `participation_only` unless a later closeout contract explicitly says
  otherwise

The launch freeze also means grouped-replica stage execution is not a hidden
launch dependency. The authority schema should keep supporting it, but launch
truth, product language, and public stats must not imply that grouped replicas
are already required before the weak-device claim is honest.

## Trust And Quorum Rules

The current control plane now treats overlap and promotion authority as
first-class invariants instead of operator convention.

- one node cannot hold conflicting active roles on the same run
  - worker plus validator is forbidden
  - worker plus recovery source is forbidden
  - validator plus recovery source is forbidden
- validators cannot lease challenges against assignments they contributed
- promotion-bearing closeouts must satisfy validator quorum explicitly
- promotion-bearing closeouts must wait out the validator challenge window
  before Nexus treats the checkpoint as canonization-ready

In practice that means:

- non-promotion closeouts can still finalize accepted work without pretending a
  new canonical checkpoint exists
- promotion requests stay held until the validator policy's minimum distinct
  validator count and challenge-window requirements are both satisfied
- the defensibility audit records the promotion posture, counted validators,
  contributor identities, and any refused overlap

## Current Apple Operator Path

The retained Apple adapter path now has a concrete operator-to-authority flow
in `apps/autopilot-desktop`:

1. desktop control or `autopilotctl` launches an Apple adapter run under
   repo-owned operator orchestration
2. the run stages one `.fmadapter` package locally
3. held-out eval plus runtime-smoke checks execute before authority acceptance
4. export is optional operator materialization, not canonical market truth
5. acceptance registers the canonical environment, benchmark, validator,
   checkpoint-family, training-policy, eval-run, training-run, and
   accepted-outcome records

For the current live Apple-valid path, the operator lane now delegates the
training/export step to the local Apple adapter toolkit wrapper in
`psionic-train`. That is distinct from the older repo-native reference backend
and is the truthful boundary for Apple runtime compatibility today.

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
- the declared work class and replica type must be coherent for the run:
  - grouped replica stage execution requires `grouped_replica`
  - full-island local-update training requires `island`
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

### Training window record

- window records require one explicit `base_checkpoint_ref`
- the explicit `base_checkpoint_ref` must match the source checkpoint pointer
- optional `round_index` and `planned_local_step_count` become part of the
  canonical authority object rather than metadata-only hints
- optional `aggregation_rule` and `aggregation_weight_basis` must either both
  be present or both be absent
- adapter-target and adapter-format fields remain required for
  `adapter_training`, but non-adapter work classes can persist empty adapter
  naming fields while still carrying the same window lifecycle
- `window_summary_digest` now covers the window's work class, replica type,
  round, base-checkpoint, aggregation semantics, accepted aggregate linkage,
  promoted checkpoint linkage, accepted outcome linkage, and contribution-level
  local-work accounting fields

### Contribution outcome record

- contribution records require one explicit `base_checkpoint_ref`
- the explicit `base_checkpoint_ref` must match the source checkpoint pointer
- contribution work class and replica type must remain coherent with the parent
  run or window topology rules
- local-step count, token count, example count, and aggregation weight value
  must be positive when present
- aggregation weight basis and value must either both be present or both be
  absent
- adapter dataset slices remain required for `adapter_training`, but non-adapter
  contributions can persist default slices while preserving the same receipt and
  lineage surface

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
- the window work class and replica type are durable control-plane fields, not
  inferred metadata
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

### Admitted training nodes and scheduler projection

Outside the kernel registry objects, Nexus also persists one admitted-node
read model for the training scheduler and public operator surfaces.

Each admitted training node now preserves:

- retained role claims and allowed networks
- contributor-availability contract fields
- one capability-tier profile with:
  - capability tier (`tier0_presence` through `tier4_authority`)
  - backend families and accelerator inventory
  - memory floor and available memory
  - throughput band
  - lease reliability class
  - replay capability
  - artifact upload latency class
- retained build and environment identity
- last observed runtime, lease, heartbeat, and settlement destination state

That projection is intentionally scheduler-readable. Nexus can match nodes
against run requirements without reverse-engineering raw host telemetry from
opaque metadata blobs or relay-only side channels.

Scheduler matching now consumes that admitted-node profile together with the
training run's declared `work_class` and `replica_type`. Lease assignment
therefore fails closed on explicit mismatches such as:

- backend-family mismatch
- environment mismatch
- work-class tier insufficiency
- replica-type tier insufficiency

This keeps scheduler refusal semantics legible instead of collapsing every
missed assignment into a generic "run not found" path.

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
That public lane now includes typed TRN `kind:39520` artifact locators for
accepted local updates, reconciled aggregates, and promoted checkpoints so the
relay-visible control plane carries round lineage and artifact roles without
embedding the underlying training bytes.
