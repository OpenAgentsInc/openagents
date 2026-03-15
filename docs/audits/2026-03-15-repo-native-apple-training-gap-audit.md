# 2026-03-15 Repo-Native Apple Training Gap Audit

> Historical note: this is a point-in-time architecture audit written on
> 2026-03-15. Product authority still lives in `docs/MVP.md`,
> `docs/OWNERSHIP.md`, and the retained MVP implementation. This document is an
> honest gap report for later-family Apple training work, not a claim that the
> whole program is in current MVP scope.

## Intent

This audit answers the narrower follow-up question after the real local Apple
adapter run succeeded:

> what is still missing for OpenAgents to honestly claim full repo-native Apple
> adapter training end to end?

For this audit, "repo-native training" means all of the following are owned by
code in this repo:

- dataset import, validation, augmentation, and packing
- training execution and checkpoint or adapter production
- `.fmadapter` export
- held-out eval and Apple runtime-smoke validation
- kernel and Nexus authority publication
- desktop, CLI, provider, and market truth

It does not mean "the Mac can train an adapter somehow." We already proved that
with the external Apple toolkit. It means OpenAgents itself owns the path.

## Executive Summary

OpenAgents does not have repo-native Apple training yet.

What is already real:

- Apple adapter dataset, package, and lineage specs are frozen in repo docs and
  fixtures.
- `psionic-adapters` owns first-party Apple `.fmadapter` parsing, writing, and
  inventory validation.
- `psionic-apple-fm` plus the Swift foundation bridge own adapter inventory,
  load, unload, attach, detach, and request-level adapter override at runtime.
- a real local Apple toolkit run on this Mac produced a final adapter,
  exported a `.fmadapter`, and that artifact successfully attached and ran
  through the live bridge.
- generic Psionic train, eval, environment, kernel-authority, and
  desktop-control substrate already exists.

What is not real yet:

- Apple dataset semantics do not live inside `psionic-data`
- Apple train or eval environment packages do not exist in
  `psionic-environments`
- Apple eval harnesses do not exist in `psionic-eval`
- kernel training and benchmark validation does not type-check Apple lineage
- `psionic-train` does not own an Apple adapter SFT lane
- Nexus does not publish Apple training truth with Apple-specific admissibility
  discipline
- the provider substrate does not advertise Apple adapter-hosting or Apple
  training families
- desktop and `autopilotctl` do not provide a full Apple adapter operator flow

The most important technical nuance is this:

> the current `psionic-train` core is a typed fixed-budget optimizer-step
> substrate, not a finished Apple training runtime.

That is an inference from the current codebase, not speculation. The trainer
core takes explicit gradient batches and applies them. It does not yet own the
Apple-specific data-to-forward-to-backward-to-export path.

The current open issue program `#3620` through `#3630` broadly matches the
remaining work. No remaining open issue should be closed today. The one place
where the program is still overloaded is `#3625`, which currently carries both
"Apple training execution backend" and "Psionic trainer orchestration/export
integration" in one issue body.

## Scope

The code and docs checked for this audit were:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/kernel/compute-training-authority.md`
- `docs/kernel/markets/compute-market.md`
- `docs/kernel/markets/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/APPLE_ADAPTER_DATASET_SPEC.md`
- `crates/psionic/docs/APPLE_FMADAPTER_PACKAGE_SPEC.md`
- `crates/psionic/docs/APPLE_ADAPTER_LINEAGE_SPEC.md`
- `crates/psionic/psionic-adapters/src/lib.rs`
- `crates/psionic/psionic-apple-fm/src/contract.rs`
- `crates/psionic/psionic-apple-fm/src/client.rs`
- `crates/psionic/psionic-data/src/lib.rs`
- `crates/psionic/psionic-environments/src/lib.rs`
- `crates/psionic/psionic-eval/src/lib.rs`
- `crates/psionic/psionic-train/src/lib.rs`
- `crates/psionic/psionic-train/src/core_loop.rs`
- `crates/psionic/psionic-train/src/model_io.rs`
- `crates/openagents-kernel-core/src/compute.rs`
- `crates/openagents-kernel-core/src/compute_benchmarks.rs`
- `crates/openagents-provider-substrate/src/lib.rs`
- `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- `apps/autopilot-desktop/src/panes/apple_fm_workbench.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/research_control.rs`
- `swift/foundation-bridge/Sources/foundation-bridge/*`

## What Is Already Proven

## 1. Artifact format truth is no longer the blocker

The repo now owns the Apple adapter package contract well enough to stop
arguing about the file format.

Concrete evidence:

- `crates/psionic/psionic-adapters/src/lib.rs`
  - `.fmadapter` package identification
  - metadata validation
  - package inventory digesting
  - writer support
- `crates/psionic/fixtures/apple_adapter/`
  - positive and negative package fixtures
- `crates/psionic/docs/APPLE_FMADAPTER_PACKAGE_SPEC.md`
  - repo-owned package contract

This is enough to say:

- OpenAgents knows what a valid Apple adapter package looks like
- OpenAgents can parse and write that package shape

## 2. Runtime adapter serving is no longer hypothetical

The repo also owns the serving-side adapter lifecycle now.

Concrete evidence:

- `crates/psionic/psionic-apple-fm/src/contract.rs`
  - adapter inventory, load, unload, attach, detach, and session-level adapter
    contracts
- `crates/psionic/psionic-apple-fm/src/client.rs`
  - blocking and async bridge client support
- `swift/foundation-bridge/Sources/foundation-bridge/Server.swift`
  - HTTP endpoints for adapter inventory and session attachment
- `swift/foundation-bridge/Sources/foundation-bridge/ChatHandler.swift`
  - real load, unload, attach, detach, and request override handling

The local live run on this Mac proved:

- a real exported adapter can be loaded into bridge inventory
- a real session can attach that adapter
- attached generation actually works

That closes the old "maybe Apple adapters are too hypothetical to integrate"
argument. They are not.

## 3. Generic train and authority substrate already exists

There is already meaningful reusable substrate in repo:

- `psionic-train`
  - fixed-budget trainer-step loop
  - optimizer families
  - distributed optimizer contracts
  - checkpoint and recovery substrate
  - model IO and generic adapter delta derivation
- `psionic-environments`
  - environment package ABI
- `psionic-eval`
  - eval run and benchmark package ABI
- `openagents-kernel-core`
  - training policy, benchmark package, training run, and accepted outcome
    object model
- `apps/nexus-control`
  - generic persistence for training runs and accepted outcomes
- `apps/autopilot-desktop`
  - training read-model projection through `desktop_control`

So the remaining work is not "invent train or authority from zero." It is
"finish the Apple-specific layers honestly on top of the substrate that now
exists."

## Current Codebase Reality

## 1. Desktop and CLI are still Apple-runtime status surfaces, not Apple adapter operator surfaces

The desktop app already knows about Apple FM readiness, but it does not yet own
adapter lifecycle or training workflow truth.

Concrete evidence:

- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
  - `AppleFmCommand` currently exposes only `Refresh` and `SmokeTest`
- `apps/autopilot-desktop/src/desktop_control.rs`
  - `DesktopControlAppleFmStatus` only carries readiness-style fields:
    `reachable`, `ready`, `model_available`, `ready_model`, `bridge_status`,
    `last_action`, and `last_error`
- `apps/autopilot-desktop/src/desktop_control.rs`
  - `DesktopControlActionRequest` only exposes `RefreshAppleFm` and
    `RunAppleFmSmokeTest` for the Apple FM lane
- `apps/autopilot-desktop/src/desktop_control.rs`
  - `DesktopControlTrainingStatus` is a projected read model over generic
    authority truth, not an Apple training control plane

What that means:

- `#3620` is still open
- `#3629` is still open

The app can report Apple runtime readiness and generic training history, but it
cannot yet truthfully act as the Apple adapter operator console.

## 2. `psionic-data` still has no Apple dataset importer

`psionic-data` is real substrate, but it is still generic substrate.

Concrete evidence:

- `crates/psionic/psionic-data/src/lib.rs` owns:
  - `DatasetKey`
  - `TokenizerDigest`
  - dataset manifests
  - iteration contracts
  - deterministic packing policy
- the crate does not yet contain Apple-specific JSONL parsing, role-order
  validation, `response_format` handling, or tool-definition augmentation

The important difference is:

- the spec exists in `APPLE_ADAPTER_DATASET_SPEC.md`
- the runtime implementation inside `psionic-data` does not

What that means:

- `#3621` is still open

## 3. `psionic-environments` and `psionic-eval` are generic only

OpenAgents has the reusable ABI for environment and eval truth, but not the
Apple package family that repo-native training would need.

Concrete evidence:

- `crates/psionic/psionic-environments/src/lib.rs`
  - generic `EnvironmentPackageFamily`
  - generic dataset binding
  - generic tool contracts
  - generic rubric hooks
- `crates/psionic/psionic-eval/src/lib.rs`
  - generic eval run
  - benchmark packages
  - metrics and artifact contracts

What is missing:

- Apple train or eval environment package families
- train/eval parity receipts tied to the same Apple package
- Apple-specific held-out eval harnesses
- structured-output checks for Apple adapter outputs
- tool-call behavior checks for Apple adapter outputs
- machine-legible runtime-smoke receipts that prove an exported adapter both
  parses and actually runs against the Apple lane

What that means:

- `#3622` is still open
- `#3623` is still open

## 4. `psionic-train` is not yet an Apple training lane

This is the most important gap.

`psionic-train` has real training substrate, but the current reference loop is
still one level below full repo-native Apple training.

Concrete evidence:

- `crates/psionic/psionic-train/src/core_loop.rs`
  - `TrainingGradientBatch` carries explicit gradients
  - `TrainingStepExecutionMode` currently only exposes
    `ExplicitGradientBatch`
  - `FixedBudgetTrainingRun::apply_step` applies explicit gradients to owned
    parameter groups
- `crates/psionic/psionic-train/src/model_io.rs`
  - generic portable state-dict handling exists
  - generic adapter delta derivation and merge or unmerge exists
  - GGUF and safetensors import exists
- `crates/psionic/psionic-apple-fm/src/*`
  - adapter runtime and serving contracts exist
  - there is no training API surface there
- `swift/foundation-bridge/Sources/foundation-bridge/*`
  - serving and session adapter support exists
  - there is no training endpoint or trainer orchestration surface there

Inference from the code:

- OpenAgents can currently apply gradients when they already exist
- OpenAgents cannot yet start from Apple training records and produce those
  gradients through a repo-owned Apple adapter lane
- OpenAgents can derive a generic additive adapter delta, but it still does not
  bridge that into an end-to-end repo-owned Apple `.fmadapter` training export
  pipeline
- `psionic-models` is centered on GGUF and safetensors portability, not on a
  repo-owned Apple Foundation Models training representation

This is why the successful local Apple toolkit run does not close `#3625`.

The external toolkit run proved:

- the Apple hardware and runtime path works
- export and bridge attach work

It did not prove:

- that OpenAgents itself owns the training execution path

What that means:

- `#3625` is still open
- `#3626` is still open

## 5. Kernel and Nexus are still generic about Apple lineage

The authority surface is more mature than before, but Apple-specific lineage is
still mostly implicit rather than typed.

Concrete evidence:

- `crates/openagents-kernel-core/src/compute_benchmarks.rs`
  - `ComputeBenchmarkAdapterKind` currently contains only
    `MmluMultipleChoiceV1`
- `crates/openagents-kernel-core/src/compute.rs`
  - `ComputeBenchmarkPackage`, `ComputeTrainingPolicy`, and
    `ComputeTrainingRun` all still rely on generic `metadata: Value` for
    extension fields
  - `validate_compute_training_run` enforces generic required fields, but does
    not enforce Apple-specific base-model signature, tokenizer digest,
    `.fmadapter` format version, draft-model posture, or Apple runtime
    validation posture
- `apps/nexus-control/src/kernel.rs`
  - generic create, finalize, and accepted-outcome paths already exist
- `apps/autopilot-desktop/src/desktop_control.rs`
  - desktop already projects generic training runs and accepted outcomes

What that means:

- `#3624` is still open
- `#3627` is still open

The important distinction is:

- generic training authority exists
- Apple training authority discipline does not yet exist

## 6. Provider and market truth still stop at Apple inference

The product and market layer has not yet crossed into Apple training or Apple
adapter-hosting.

Concrete evidence:

- `crates/openagents-provider-substrate/src/lib.rs`
  - `ProviderComputeProduct` includes `AppleFoundationModelsInference`
  - there is no Apple adapter-hosting product family there
  - provider product derivation is built around current visible products only
- `apps/autopilot-desktop/src/state/operations.rs`
  - compute-family drafting includes generic `training` and `adapter_hosting`
    labels, but that is not the same as a truthful Apple product derivation
- `docs/kernel/markets/compute-market.md`
  - training and adapter-hosting are still described as later-family work

What that means:

- `#3628` is still open
- `#3630` is still open

## Remaining Work, In Order

The remaining work is best read as four layers.

## Layer 1: Data and parity truth

These issues define what an Apple training run actually means in repo-owned
terms:

- `#3621`
  - port Apple dataset semantics into `psionic-data`
- `#3622`
  - define the Apple environment package family and train/eval parity receipt
- `#3623`
  - add Rust-owned eval and runtime-smoke harnesses
- `#3624`
  - type and validate Apple lineage at the kernel boundary

Until this layer exists, later training output would still be under-specified
and hard to admit into authority truth honestly.

## Layer 2: Repo-native trainer execution

This is the hard technical center:

- `#3625`
  - repo-owned Apple adapter SFT lane
- `#3626`
  - optional draft-model distillation lane

To finish this layer, OpenAgents still needs all of the following:

- Apple dataset batches emitted from repo-owned dataset code
- adapter-only parameter selection and base-model freeze semantics
- a repo-owned path from batch data to gradient production, not just gradient
  application
- checkpoint and summary emission that later layers can consume
- `.fmadapter` export from repo-owned training outputs

Without this layer, OpenAgents still depends on an external trainer for the
actual learning step.

## Layer 3: Authority truth

Once the trainer exists, the outputs still need to become durable truth:

- `#3627`
  - persist and project Apple training runs and accepted outcomes

To close this honestly, the accepted path must be:

- train in repo-owned code
- evaluate in repo-owned code
- runtime-smoke on the Apple lane
- accept through kernel and Nexus

not:

- export succeeded locally, therefore treat it as accepted truth

## Layer 4: Operator and product truth

Only after the lower layers are real can desktop and market surfaces become
truthful:

- `#3620`
  - desktop and CLI adapter lifecycle controls
- `#3628`
  - provider substrate adapter-hosting derivation
- `#3629`
  - end-to-end desktop and CLI Apple training workflow
- `#3630`
  - market and docs may finally advertise the family

## One Missing Decomposition

The current issue program is mostly right, but one technical concern is still
bundled too broadly.

## `#3625` is doing two jobs

Today `#3625` effectively includes both:

- the Apple training execution backend or integration needed to turn training
  examples into gradients
- the trainer orchestration, checkpointing, summary, and export wiring inside
  `psionic-train`

That can stay as one issue if momentum is good, but it is the one issue most
likely to need a split if work stalls.

A cleaner decomposition would be:

- Apple training execution backend and gradient production
- Apple adapter SFT orchestration and `.fmadapter` export on top of that backend

I do not think a new issue must be created immediately, but this is the one
place where the current program hides the hardest remaining work behind a
single title.

## What Can Be Closed Today

Only the already-finished early issues were honest closes:

- `#3616`
- `#3617`
- `#3618`
- `#3619`

The remaining open issues `#3620` through `#3630` should stay open.

Reason:

- they still describe real missing code or truth boundaries
- the successful local Apple toolkit run reduces risk, but it does not satisfy
  repo-native acceptance for the remaining issue set

## Honest Definition Of Done

OpenAgents can say "repo-native Apple training exists" only when the following
statement is true without qualification:

> starting from a checked-in Apple dataset fixture, OpenAgents can train an
> Apple adapter without Python, export a valid `.fmadapter`, evaluate it through
> Rust-owned harnesses, runtime-smoke it on the Apple lane, publish it through
> kernel and Nexus accepted-outcome truth, and surface the same truth through
> desktop, CLI, provider inventory, and market docs.

That is not true today.

It is the correct bar.
