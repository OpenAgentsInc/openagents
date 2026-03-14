# 2026-03-14 AFMTrainer And Apple Adapter Toolkit Integration Audit

## Intent

This audit answers a specific integration question:

> after reading the current OpenAgents compute-market, economy-kernel, and
> Psionic training-system docs, plus the local `AFMTrainer` and
> `adapter_training_toolkit_v26_0_0` repos, how should we integrate Apple's
> adapter-training stack and the AFMTrainer GUI into OpenAgents without
> violating current ownership boundaries or lying about what the system does?

The short answer is:

> we should integrate the Apple adapter toolkit as a narrow, explicit,
> Apple-specific training compatibility lane and artifact source, while
> treating AFMTrainer as workflow reference material rather than code to import.

That means:

- do adapt the training/export workflow
- do adapt the artifact and validation contracts
- do extend the Apple FM bridge so exported adapters can actually be used
- do publish training runs and accepted outcomes through kernel authority
- do not make the Python toolkit the definition of `Psionic Train`
- do not embed the Tk GUI into the product
- do not market this as an adapter-hosting market before bridge, eval, and
  settlement truth exist

## Scope

OpenAgents sources reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/kernel/README.md`
- `docs/kernel/economy-kernel.md`
- `docs/kernel/markets/compute-market.md`
- `docs/kernel/compute-training-authority.md`
- `docs/kernel/compute-benchmark-adapters.md`
- `docs/adr/ADR-0003-compute-market-ownership-and-authority-split.md`
- `docs/plans/compute-market-full-implementation-plan.md`
- `docs/audits/2026-03-10-apple-fm-swift-bridge-audit.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/FM_BRIDGE_CONSIDERATIONS.md`
- `crates/openagents-kernel-core/src/compute.rs`
- `crates/openagents-kernel-core/src/compute_benchmarks.rs`
- `crates/openagents-provider-substrate/src/lib.rs`
- `crates/psionic/psionic-apple-fm/src/*`
- `crates/psionic/psionic-adapters/src/lib.rs`
- `crates/psionic/psionic-train/src/model_io.rs`
- `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- `apps/autopilot-desktop/src/local_inference_runtime.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/state/operations.rs`
- `swift/foundation-bridge/README.md`
- `swift/foundation-bridge/Sources/foundation-bridge/*`

External repos reviewed:

- `/Users/christopherdavid/code/AFMTrainer/README.md`
- `/Users/christopherdavid/code/AFMTrainer/pyproject.toml`
- `/Users/christopherdavid/code/AFMTrainer/afm_trainer/afm_trainer_gui.py`
- `/Users/christopherdavid/code/AFMTrainer/afm_trainer/training_controller.py`
- `/Users/christopherdavid/code/AFMTrainer/afm_trainer/export_handler.py`
- `/Users/christopherdavid/code/AFMTrainer/afm_trainer/file_manager.py`
- `/Users/christopherdavid/code/AFMTrainer/afm_trainer/config_manager.py`
- `/Users/christopherdavid/code/AFMTrainer/afm_trainer/wandb_integration.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/docs/schema.md`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/data.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/train_adapter.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/train_draft_model.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/generate.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/examples/utils.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/export/constants.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/export/export_fmadapter.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/export/export_utils.py`
- `/Users/christopherdavid/code/adapter_training_toolkit_v26_0_0/export/produce_asset_pack.py`

## Executive Summary

The Apple adapter toolkit is real and useful, but it is much narrower than a
general training substrate.

What it actually is:

- a Python/Torch/TAMM adapter trainer for Apple's fixed foundation-model asset
  pack
- a JSONL chat-data pipeline with optional guided-generation and tool-calling
  schema augmentation
- an optional draft-model distillation lane for speculative decoding
- an exporter that turns checkpoints into an Apple-specific `.fmadapter`
  package plus optional Apple asset-pack packaging

What AFMTrainer actually is:

- a standalone Tk GUI wrapper around that toolkit
- a subprocess orchestrator that shells out to `uv run python -m ...`
- a workflow reference for setup, validation, training, export, and monitoring
- not a reusable library or stable service boundary

How this fits OpenAgents:

- it fits best as an Apple-specific compatibility backend for one narrow
  training family, not as the definition of Psionic train
- it fits cleanly with existing OpenAgents architecture only if we split:
  - training orchestration and UX in `apps/autopilot-desktop`
  - artifact identity and package truth in `psionic-adapters`
  - adapter-serving bridge extensions in `psionic-apple-fm` plus
    `swift/foundation-bridge`
  - canonical training-policy, training-run, benchmark, and accepted-outcome
    truth in kernel plus Nexus
- it does not fit if we try to import the Tk GUI, hide the Python stack inside
  `psionic-train`, or promise an adapter market before the bridge can actually
  load an exported adapter

The most important integration gap is not training.

The most important integration gap is serving:

> OpenAgents can already supervise and use the Apple FM bridge for text,
> sessions, streaming, structured generation, and tools, but the bridge and its
> Rust client currently expose no adapter-loading contract at all.

Until that changes, Apple adapter training can produce artifacts, but OpenAgents
cannot honestly use those artifacts as first-class local-runtime or provider
outputs.

## What The Apple Repos Actually Give Us

## 1. The toolkit is a narrow adapter-training stack, not a general train system

The Apple toolkit is specialized around one bounded training product:

- chat-style supervised fine-tuning data in JSONL
- fixed base-model assets shipped inside the toolkit
- frozen base model with trainable adapter parameters only
- optional draft-model distillation
- export into an Apple runtime-specific package format

This matters because it means the right OpenAgents translation is not:

- "we now have generic training solved"

It is:

- "we now have one concrete Apple adapter-training backend we can wrap,
  validate, and turn into typed artifacts and authority records"

The code makes that specialization explicit:

- `load_base_model(...)` loads toolkit-bundled base assets and freezes all
  non-adapter parameters
- `train_adapter(...)` is an SFT-style adapter trainer, not a general
  distributed train orchestrator
- `SchemaAugmenter` injects Apple-guided-generation schemas directly into the
  message format
- tool-calling is represented inside the dataset, not as a generic runtime ABI

## 2. The toolkit's data model lines up surprisingly well with our Apple FM bridge

This is the best part of the fit.

The toolkit can train for:

- plain chat responses
- schema-guided structured outputs
- tool-calling patterns

Our retained Apple FM lane already has:

- sessions
- streaming
- structured generation
- tool-callback plumbing

That means there is a real product path where Apple-trained adapters improve
exactly the behaviors the current local bridge already exposes. The missing
piece is adapter attachment, not conceptual alignment.

## 3. The toolkit's output format is Apple-specific and should stay explicit

`export_fmadapter.py` writes:

- `.fmadapter/metadata.json`
- `.fmadapter/adapter_weights.bin`
- optional `draft_weights.bin`
- optional `draft.mil`

The metadata includes vendor-specific facts like:

- adapter identifier
- base-model signature
- LoRA rank
- creator-defined metadata

This should not be silently flattened into generic `safetensors`.

It is a distinct packaging family with distinct serving implications. The
OpenAgents artifact layer should say that plainly.

## 4. AFMTrainer is useful as workflow reference, not as code to import

AFMTrainer contributes four useful ideas:

- operator-facing setup and validation flow
- configuration/profile UX
- progress and logging UX
- export and packaging workflow

But its implementation is the wrong substrate to import:

- Tk UI
- loose local file heuristics
- regex parsing of stdout for progress
- `uv` subprocess bootstrapping inside GUI logic

That is not how OpenAgents should represent replay-safe, control-plane-visible
training behavior.

The right move is:

- copy the workflow shape
- not the Python/Tk code

## Fit With OpenAgents Architecture

| OpenAgents layer | Correct role in this integration | What should not land there |
| --- | --- | --- |
| `apps/autopilot-desktop` | Detect toolkit, validate entitlement/toolchain presence, start and monitor runs, own WGPUI panes, own operator workflow, supervise Apple FM bridge, expose training status through desktop control | Reusable adapter package law, canonical settlement, generic train-system core |
| `crates/openagents-provider-substrate` | Later: narrow provider health/product descriptors for Apple adapter training or adapter hosting if those become reusable provider products | Python subprocess orchestration, bridge supervision, settlement logic |
| `crates/psionic/psionic-apple-fm` | Own typed Rust contract additions for adapter attach/list/detach and adapter-aware session/completion requests | Process discovery, app UX, user install/build messaging |
| `swift/foundation-bridge` | Implement actual adapter loading and execution against Apple runtime APIs | Market truth, desktop workflow, checkpoint or training-policy authority |
| `crates/psionic/psionic-adapters` | Own Apple adapter package identity, digests, manifests, hosted binding lineage | UI, subprocess management, Nexus authority |
| `crates/psionic/psionic-train` | Consume Apple-produced artifacts into typed model and lineage surfaces where honest; remain Rust-native core for real train substrate | Hidden Python toolkit wrapper masquerading as Psionic train itself |
| `crates/openagents-kernel-core` + `apps/nexus-control` | Publish training policies, training runs, benchmark packages, and accepted outcomes for Apple adapter jobs | Local subprocess logs as authority, app-local truth as market truth |

This split matches:

- `docs/OWNERSHIP.md`
- `ADR-0003`
- `ARCHITECTURE.md`
- `TRAIN_SYSTEM.md`
- `docs/kernel/compute-training-authority.md`

## The Right Integration Model

## Principle 1: treat the Apple toolkit as an external compatibility backend

The toolkit should be treated like:

- a versioned external execution environment
- an Apple-specific backend for a narrow training family
- a source of artifacts and metrics that we ingest into our own typed surfaces

It should not be treated like:

- the canonical implementation of `Psionic Train`
- a reason to pull Python orchestration into Psionic core
- a reason to weaken the current ownership split

The current Psionic docs are explicit that `Psionic Train` is not supposed to
be "a Python trainer hidden behind Rust wrappers." That rule should survive
this integration.

## Principle 2: training and serving must be decoupled and both must be explicit

The toolkit solves:

- training
- export

It does not solve our serving path inside OpenAgents.

Our serving path today is:

- `psionic-apple-fm` contract/client
- `swift/foundation-bridge`
- `apps/autopilot-desktop/src/apple_fm_bridge.rs`

Those surfaces must be extended before the training results become a truthful
product lane.

## Principle 3: artifact lineage is part of the market claim

The compute-market docs are already explicit that artifact lineage is market
truth. Apple adapter training makes that concrete.

Two Apple adapter-training jobs are not economically comparable unless we know:

- toolkit version
- base-model signature
- tokenizer lineage
- structured-generation or tool-calling schema policy
- benchmark package
- validator policy
- export format version
- whether a draft model was also trained

Those should not be hidden inside a local output folder.

They should become:

- environment bindings
- checkpoint bindings
- adapter package manifests
- benchmark package refs
- accepted-outcome artifacts

## Principle 4: AFMTrainer's UI should become an OpenAgents pane, not a bundled side app

The AFMTrainer tabs already suggest the right product shape:

- setup
- dataset selection
- training parameters
- export metadata
- monitoring

That should be re-expressed as:

- app-owned desktop pane(s)
- desktop-control status
- `autopilotctl` commands
- session logs and receipts

It should not stay as a Tk sidecar if the goal is integration into "our own
systems."

## Concrete Integration Plan

## Phase 0: add a local compatibility lane in `apps/autopilot-desktop`

This is the fastest honest integration.

Build an app-owned runner that:

- detects a user-supplied Apple toolkit root
- validates required toolkit structure and version
- validates dataset files against toolkit-compatible rules
- launches `examples.train_adapter`
- optionally launches `examples.train_draft_model`
- launches `export.export_fmadapter`
- captures stdout/stderr as structured job logs
- records final artifact paths and digests in app state

This should live in the app layer because it is:

- local operator workflow
- subprocess orchestration
- entitlement/toolchain messaging
- not reusable train-substrate truth yet

Recommended inputs:

- `OPENAGENTS_APPLE_ADAPTER_TOOLKIT_DIR`
- optional explicit Python/UV interpreter path
- output root under something like
  `~/.openagents/apple-adapters/<run-id>/`

Recommended immediate UI:

- a new WGPUI training pane or an Apple-adapter subsection of Mission Control
- explicit blockers such as:
  - toolkit missing
  - entitlements unavailable
  - Apple export toolchain unavailable
  - dataset invalid

Recommended immediate CLI:

- `autopilotctl apple-adapter validate`
- `autopilotctl apple-adapter train`
- `autopilotctl apple-adapter export`
- `autopilotctl apple-adapter status`

## Phase 1: make `.fmadapter` a typed OpenAgents artifact

This is the next required foundation.

Add explicit Apple adapter artifact support in `psionic-adapters`.

Minimum shape:

- package format for Apple FM adapter bundles
- stable digest of the package
- parsed metadata fields:
  - adapter identifier
  - base-model signature
  - LoRA rank
  - draft-model presence
  - creator metadata
- references to contained files
- hosted-binding lineage fields for later serving attachment

Important rule:

- do not pretend `.fmadapter` is `safetensors`
- do not force it into the existing `Safetensors` enum slot

Two acceptable shapes are:

- add an explicit Apple package format to `AdapterArtifactFormat`
- or add a sibling Apple-specific package-manifest type in
  `psionic-adapters`

Either is fine. The wrong move is flattening away the Apple-specific contract.

This phase should also capture the relationship between:

- training checkpoint `.pt`
- exported `.fmadapter`
- optional draft artifacts
- later served adapter binding

## Phase 2: extend the Apple FM bridge so OpenAgents can actually use trained adapters

This is the critical missing piece.

The current bridge stack already supports:

- health
- model discovery
- chat completions
- sessions
- structured generation
- streaming
- tools

It does not support:

- loading an exported Apple adapter package
- selecting an adapter per session/request
- listing loaded adapters
- resetting or unloading adapter state explicitly

Without this, the training lane ends in a dead artifact.

The bridge extension should add an explicit adapter contract, likely one of:

- adapter refs on session creation
- adapter refs on one-shot completion requests
- dedicated adapter management endpoints

Recommended ownership:

- `psionic-apple-fm`
  - typed request/response contracts
  - reusable Rust client behavior
- `swift/foundation-bridge`
  - real adapter-loading implementation
- `apps/autopilot-desktop`
  - UI, workflow, binary supervision, adapter-picker UX

Recommended first target:

- local workbench use of one selected adapter
- not provider-market adapter hosting yet

Once this exists, the existing Apple FM bridge workbench becomes a natural
test harness for trained adapters.

## Phase 3: publish Apple adapter jobs into kernel training authority

Once the app can run and export adapter jobs repeatably, it should stop living
only as local process output.

Map it into existing kernel training authority.

Recommended training-policy shape:

- `environment_ref`
  - something like `env://apple/fm/adapter_training/v26`
- `checkpoint_family`
  - Apple adapter training family, not generic model training
- `validator_policy_ref`
  - Apple-adapter eval and smoke-test policy
- `benchmark_package_refs`
  - task-specific benchmarks or held-out eval profiles
- `stage_policy_refs`
  - optional split between adapter SFT stage and draft-model distillation stage

Recommended training-run shape:

- `training_run_id`
- `training_policy_ref`
- resolved environment binding
- checkpoint binding
- benchmark package refs
- expected and completed step counts
- final `.pt` checkpoint refs
- exported `.fmadapter` recorded as artifact in the summary
- final metrics such as average loss and best eval score

Recommended accepted-outcome rule:

- do not accept a run into durable outcome truth only because export succeeded
- accept it only after benchmark/eval and Apple-runtime smoke validation pass

This matters because the economy kernel is about verified outcomes, not just
completed subprocesses.

## Phase 4: turn AFMTrainer's workflow into OpenAgents UI and control-plane truth

The current AFMTrainer app gives us a direct UI blueprint.

Translate its tabs into our own surfaces:

- `Setup`
  - toolkit path, dataset path, output root, environment blockers
- `Training`
  - epochs, learning rate, precision, packing, gradient accumulation
- `Export`
  - adapter name, author, description, optional draft model
- `Monitor`
  - streaming logs, progress, checkpoints, exported artifact refs

What should change in our version:

- use WGPUI, not Tk
- use desktop-control snapshots and session logs, not in-widget-only state
- use structured job state, not regex-only stdout parsing as source of truth
- use current OpenAgents training/history panes for later kernel-backed runs

This is where the existing `desktop_control.rs` training status becomes useful.

The repo already has:

- `DesktopControlTrainingStatus`
- `DesktopControlTrainingRunStatus`
- loading of training runs and accepted outcomes from kernel authority

So the UI does not need to invent a second training truth model.

## Phase 5: only then productize training and adapter-hosting compute families

The compute-market docs already reserve later-family space for:

- `training`
- `adapter_hosting`

This integration gives us a credible path to those families, but not an excuse
to claim them early.

The order should be:

1. local compatibility runner
2. typed Apple adapter artifact
3. bridge-side adapter loading
4. benchmark and accepted-outcome validation
5. training policy/run/outcome authority
6. only then market-facing training or adapter-hosting products

That sequencing matches the current compute-market law:

- proof before market claims
- authority before settlement
- artifact lineage before comparability claims

## Specific Component Translation

| External component | What it should become in OpenAgents | Notes |
| --- | --- | --- |
| `examples/train_adapter.py` | App-owned compatibility runner now; later environment-packaged backend with authority publication | Narrow Apple adapter SFT backend, not generic train core |
| `examples/train_draft_model.py` | Optional second-stage job behind explicit flag | Good later latency optimization, not phase-zero gate |
| `examples/generate.py` | Local validation harness and benchmark helper | Useful for smoke tests and post-export checks |
| `SchemaAugmenter` and tool-calling data schema | Environment-package or dataset-transform metadata | Strong fit with our existing Apple structured/tool surfaces |
| `export/export_fmadapter.py` | Export step plus artifact ingestion into `psionic-adapters` | Keep export Apple-owned; ingest results into our typed model |
| `export/produce_asset_pack.py` | Release/distribution pipeline tool, not core training truth | Keep operator-facing and optional |
| AFMTrainer `TrainingController` | Inspiration for app workflow only | Replace raw stdout regex as truth with structured local job state where possible |
| AFMTrainer `FileManager` | Better dataset and toolkit validation in desktop app | Use toolkit-compatible rules, not loose heuristics |
| AFMTrainer `ConfigManager` | App state plus desktop-control serialization | No reason to import Python config code |
| AFMTrainer `Monitor` tab | WGPUI pane plus session logs | Reuse existing app log and snapshot infrastructure |

## Important Design Constraints

## 1. Keep environment truth deterministic

AFMTrainer currently shells into `uv` and resolves dependencies at runtime.

That is fine for a standalone hobby GUI.

It is not enough for compute-market or accepted-outcome truth.

OpenAgents should capture at minimum:

- toolkit version
- Python version
- torch build
- TAMM version
- coremltools version
- platform and accelerator
- dependency-lock or environment digest

Those values should sit in:

- environment package metadata
- training-run metadata
- exported artifact lineage

## 2. A Linux training host does not remove the need for Apple validation

The toolkit code itself prefers:

- `mps`
- then `cuda`
- then `cpu`

So it is plausible to run training on Linux/NVIDIA.

But that only proves the training process ran.

If the claim is:

- "this exported artifact works on Apple Foundation Models"

then final acceptance should still include validation on:

- macOS
- Apple Silicon
- Apple runtime bridge with adapter loading

That can become a validator-policy requirement for accepted outcomes.

## 3. The exported Apple package is not the same as a generic Psionic adapter delta

`psionic-train/model_io.rs` and `psionic-adapters` already have useful adapter
and portable-model vocabulary.

That is a good place to attach Apple artifacts.

But the Apple package still remains vendor-specific.

The right model is:

- keep generic adapter lineage in Psionic
- add Apple package support explicitly
- do not erase Apple-specific semantics to make the type graph feel cleaner

## 4. The current bridge is richer than the old March 10 audit implies

The current retained bridge and Rust client already have:

- sessions
- streaming
- structured generation
- tools
- transcript export/restore

That makes Apple adapter integration more attractive than it looked in the
earlier audit.

The limiting factor is now adapter load/selection, not the rest of the bridge
surface.

## What We Should Not Do

- Do not vendor `AFMTrainer` into this repo.
- Do not ship the Tk GUI inside the desktop app as a parallel product surface.
- Do not move Python subprocess orchestration into `psionic-train` and call it
  "training substrate."
- Do not let local export success count as an accepted training outcome without
  benchmark and Apple-runtime validation.
- Do not pretend `.fmadapter` is a generic `safetensors` adapter.
- Do not add market-facing `adapter_hosting` claims before the Apple bridge can
  attach and serve exported adapters.
- Do not make the operator experience depend on ad hoc `uv` resolution with no
  captured environment digest.
- Do not rely on AFMTrainer's loose file validation or text-log parsing as the
  canonical truth path.

## Recommended Issue Sequence

1. `Autopilot: add Apple adapter toolkit detection and validation lane`
2. `Autopilot: add app-owned Apple adapter training/export runner with logs and status`
3. `Psionic Adapters: add explicit Apple FM adapter package manifest and digest support`
4. `Psionic Apple FM: add adapter attach/load/list contract to bridge client`
5. `foundation-bridge: implement adapter-aware session/completion support`
6. `Kernel: define Apple adapter-training environment, checkpoint family, and validator policy metadata`
7. `Autopilot + Nexus: publish Apple adapter jobs as compute training runs and accepted outcomes`
8. `WGPUI: add Apple adapter training pane and desktop-control integration`
9. `Eval: add Apple adapter benchmark packages and acceptance gates`
10. `Compute Market: productize training then adapter-hosting only after bridge and validation truth are live`

## Bottom Line

The Apple toolkit is worth integrating.

AFMTrainer is worth studying.

But the correct OpenAgents move is not "pull their GUI and Python stack into
our tree."

The correct move is:

- use the toolkit as a bounded external backend
- build our own app-owned workflow around it
- turn `.fmadapter` into a typed Psionic artifact
- extend the Apple bridge so the artifact can actually be used
- terminate training truth in kernel authority and accepted outcomes

If we do that, the Apple adapter stack becomes a credible bridge between:

- the current Apple FM local-runtime lane
- the later `training` compute family
- the later `adapter_hosting` compute family
- and the broader economy-kernel requirement that machine work be verified,
  attributable, and settleable rather than merely executable.
