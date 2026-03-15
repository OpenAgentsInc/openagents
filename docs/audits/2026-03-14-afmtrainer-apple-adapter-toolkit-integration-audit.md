# 2026-03-14 AFMTrainer And Apple Adapter Toolkit Integration Audit

## Intent

This audit answers the stricter version of the integration question:

> after reading the current OpenAgents compute-market, economy-kernel, and
> Psionic training-system docs, plus the local `AFMTrainer` and
> `adapter_training_toolkit_v26_0_0` repos, how should we integrate the Apple
> adapter-training stack into OpenAgents if the requirement is zero Python?

The answer is now explicit:

> yes, the OpenAgents integration path should be Rust-owned and zero-Python.
> The Apple repos should be treated as reference implementations and format
> specifications only, not as runtime dependencies, subprocess backends, or
> embedded tooling.

There is one important non-Python exception already sanctioned by the repo:

- the Swift Apple Foundation Models bridge remains necessary because Apple's
  Foundation Models APIs are Swift-native and the repo already owns that bridge
  boundary

So the actual rule is:

- no Python in runtime
- no Python in operator flow
- no Python as a hidden Psionic backend
- no AFMTrainer subprocess wrapper inside the app
- Rust owns training orchestration, artifact handling, validation, and market
  integration
- Swift remains only at the Apple runtime bridge boundary where Apple forces it

## Decision

The earlier version of this audit recommended a short-term app-owned Python
compatibility lane.

That recommendation is now rejected.

The retained plan is:

- port the useful Apple adapter concepts into Rust-owned OpenAgents surfaces
- use the external Apple repos only as implementation references and fixture
  sources
- keep the current Swift bridge boundary for serving because Foundation Models
  requires it

The external repos are now reference material, not integration substrate.

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

The Apple adapter toolkit is worth studying, but not worth depending on.

What those repos give us:

- a concrete reference for Apple-style adapter SFT
- a concrete reference for optional draft-model distillation
- a concrete reference for Apple's `.fmadapter` output package
- a concrete reference for guided-generation and tool-calling training data
- a concrete reference for operator workflow, validation, and monitoring

What they do not give us under a zero-Python requirement:

- a runtime backend we can honestly integrate
- a training substrate that matches Psionic ownership rules
- a product workflow we should embed directly

The right OpenAgents reading is:

> port the semantics, not the implementation.

That means:

- reimplement the Apple-compatible dataset, training, artifact, and validation
  surfaces in Rust
- extend the Swift bridge only where Apple runtime attachment requires Swift
- publish training truth through kernel authority
- keep AFMTrainer and the Apple toolkit outside the runtime and operator path

This makes the program materially larger than the earlier compatibility-lane
plan, but it also makes it coherent with:

- `docs/OWNERSHIP.md`
- `ADR-0003`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`

## Hard Requirement

The integration requirement should be stated plainly.

## Zero Python means all of the following

- no `uv`
- no `python -m examples.train_adapter`
- no `python -m export.export_fmadapter`
- no Tk sidecar
- no Python subprocess launched by `apps/autopilot-desktop`
- no Python hidden inside `psionic-train`
- no Python in operator CLI flows
- no repo feature whose truthful use requires users to install Python

## What remains allowed

- Rust across the app, Psionic, kernel, and authority surfaces
- Swift where Apple platform APIs require Swift, which currently means the
  Foundation Models bridge and possibly Apple-specific packaging helpers if
  later proven unavoidable
- external Apple repos as documentation, conformance input, and manual
  reference while implementing the Rust port

## Why this matches current repo law

`TRAIN_SYSTEM.md` is already explicit that Psionic Train must not become a
Python trainer hidden behind Rust wrappers.

`ADR-0003` is already explicit that:

- desktop owns product and operator truth
- Psionic owns reusable execution substrate
- kernel plus Nexus own economic truth

A Python compatibility lane breaks that architecture immediately because it
creates a hidden second execution substrate that Psionic does not own and the
kernel cannot reason about cleanly.

## What The Apple Repos Actually Contribute

## 1. A reference data contract

The external toolkit gives us a concrete chat-data format:

- JSONL
- lists of message objects
- roles:
  - `system`
  - `user`
  - `assistant`
- optional guided-generation schema attachment via `response_format`
- optional tool definitions in the system message

This is useful because OpenAgents already has a real Apple FM serving lane with:

- sessions
- streaming
- structured generation
- tool callbacks

So the training data semantics fit the current Apple runtime direction. We
should port those semantics into Rust-owned dataset and transform contracts.

## 2. A reference training loop shape

The toolkit gives us a bounded training loop with:

- adapter-only parameter training
- optimizer and scheduler behavior
- mixed precision and activation checkpointing knobs
- sequence packing behavior
- checkpoint save cadence
- optional draft-model training

That is useful as algorithmic and workflow reference.

It is not acceptable as runtime implementation if the rule is zero Python.

## 3. A reference export format

The most concrete thing in the Apple repos is the `.fmadapter` package.

That matters because it gives us:

- a target package layout
- metadata conventions
- a base-model signature concept
- LoRA rank metadata
- optional draft-model side artifacts

This is the single most important part to port early, because serving and
artifact truth depend on it.

## 4. A reference operator workflow

AFMTrainer is useful mainly because it shows the operator flow we need:

- toolkit selection
- dataset validation
- parameter editing
- training progress
- export metadata
- artifact packaging

We should port that workflow into WGPUI and desktop-control.

We should not import the Tk code.

## What Must Be Ported To Rust

Under the zero-Python requirement, the integration program is a port, not a
wrapper.

## 1. Dataset and preprocessing contracts

We need Rust-owned equivalents for:

- JSONL parsing
- message validation
- role-order validation
- multi-turn sample validation
- sequence padding and packing rules
- guided-generation schema injection
- tool-calling schema augmentation
- tokenizer binding and digest capture

Likely owners:

- `psionic-data`
- `psionic-apple-fm`
- possibly `psionic-train` for train-only transforms

These should become typed, testable, replay-safe data contracts rather than
one-off UI validation helpers.

## 2. Apple-compatible adapter training loop

We need a Rust-owned implementation of the Apple adapter SFT loop.

That includes:

- loading the relevant base model artifact family
- freezing non-adapter parameters
- training adapter parameters only
- optimizer and scheduler behavior
- mixed precision policy
- activation checkpointing policy
- checkpoint export and restore
- step telemetry and summary metrics

Likely owner:

- `psionic-train`

If the current Psionic tensor or model stack is not yet sufficient to express
this faithfully, the right answer is to build that missing Rust substrate, not
to route around it with Python.

## 3. Draft-model distillation loop

The Apple toolkit's speculative-decoding path is a second distinct workload.

It requires a Rust implementation for:

- teacher-model inference
- draft-model training
- inverse-KL or equivalent distillation objective
- dual-model precision policy
- draft checkpoint output

This is optional in product sequencing, but if we claim support for it later it
must also be Rust-owned.

Likely owner:

- `psionic-train`

## 4. `.fmadapter` package reader and writer

We need a Rust-owned package implementation for the Apple artifact format.

That includes:

- metadata parsing and writing
- adapter binary payload writing
- package digesting
- package validation
- optional draft artifact presence
- package manifest identity
- linkage to training runs and hosted adapter bindings

Likely owner:

- `psionic-adapters`

This should be treated as an explicit Apple package family, not squeezed into
generic `safetensors` types.

## 5. Apple adapter serving attachment

The current bridge stack already supports:

- health
- models
- sessions
- structured generation
- streaming
- tools
- transcript restore/export

It does not support:

- loading a `.fmadapter`
- selecting an adapter for a session or request
- listing loaded adapters
- unloading or resetting adapter state

So we need a Rust-plus-Swift serving extension:

- `psionic-apple-fm`
  - typed contract and client additions
- `swift/foundation-bridge`
  - actual adapter load/attach behavior against Apple runtime APIs
- `apps/autopilot-desktop`
  - operator UX, picker, health, and workbench integration

This is not optional. If OpenAgents cannot attach the artifact, then OpenAgents
does not yet have a real adapter lane.

## 6. Benchmark and validation path

The economy kernel does not accept "export succeeded" as economic truth.

We need Rust-owned validation for Apple adapter jobs:

- held-out eval runs
- structured-generation conformance checks
- tool-calling behavior checks where relevant
- Apple bridge smoke tests on the exported package
- benchmark package linkage
- accepted-outcome gating

Likely owners:

- `psionic-eval`
- `openagents-kernel-core`
- `apps/nexus-control`

This should terminate in:

- training policies
- training runs
- accepted outcomes

## 7. Product workflow and control plane

We need to port AFMTrainer's UX into our own app surfaces.

Likely owner:

- `apps/autopilot-desktop`

Required surfaces:

- WGPUI panes for setup, training, export, and monitoring
- desktop-control status and logs
- `autopilotctl` commands
- session-log persistence

This keeps the operator world unified with the rest of the app instead of
forking it into a side GUI.

## What Should Remain Reference-Only

The following should not be integrated directly:

- AFMTrainer Tk UI
- AFMTrainer Python config management
- AFMTrainer subprocess orchestration
- AFMTrainer stdout regex parsing
- Apple's Python/TAMM training scripts
- Apple's Python export scripts
- runtime `uv` environment management
- Weights & Biases Python dependency

Those repos should instead function as:

- implementation references
- format references
- behavior references
- manual audit inputs

## Proposed Owner Split

| Surface | Correct owner in the zero-Python plan | Must not own |
| --- | --- | --- |
| Apple operator flow, panes, logs, job orchestration | `apps/autopilot-desktop` | Generic train substrate, canonical settlement |
| Apple adapter package identity, manifest, digest, hosted lineage | `psionic-adapters` | UI, Python bridge logic, Nexus authority |
| Apple FM request/session/adapter contract | `psionic-apple-fm` | Process supervision, pane UX |
| Apple runtime sidecar behavior | `swift/foundation-bridge` | Economic truth, product workflow |
| Adapter SFT and draft distillation runtime | `psionic-train` | Hidden Python compatibility shell |
| Dataset, packing, and tokenizer contracts | `psionic-data` plus `psionic-apple-fm` for Apple-specific schema/tool semantics | App-owned validation-only copies |
| Apple train/eval environment package shape and parity receipts | `psionic-environments` | Ad hoc prompt shaping or benchmark-only copies |
| Benchmark and eval execution | `psionic-eval` | Canonical accepted-outcome authority |
| Later Apple adapter-hosting capability derivation and provider publication | `openagents-provider-substrate` plus `psionic-provider` | Desktop-local product IDs or kernel authority |
| Policies, training runs, outcomes, receipts | `openagents-kernel-core` plus `apps/nexus-control` | Desktop-local truth as authority |

This is the current repo architecture applied consistently to the Apple lane.

## Codebase Reality That Changes The Issue Queue

The suggested issues need to reflect the repo we actually have now, not a blank
sheet.

- `crates/psionic/psionic-apple-fm` already owns a fairly complete Rust-side
  bridge contract for health, model listing, chat completions, persistent
  sessions, streaming, structured generation, tools, and transcript import or
  export. The Apple issue program is therefore an additive adapter-management
  extension, not a rewrite of the whole bridge lane.
- `swift/foundation-bridge` already mirrors that contract in
  `Types.swift`/`ChatHandler.swift`, but it currently has no adapter inventory,
  compatibility, attach, detach, or capability-reporting path.
- `apps/autopilot-desktop` already has `apple_fm_bridge.rs`, the Apple FM
  workbench pane, `desktop_control` Apple FM status, `autopilotctl apple-fm`
  status and smoke flows, and authority-projected training status. The operator
  work is therefore mostly about extending existing surfaces, not inventing a
  second Apple UI.
- `crates/psionic/psionic-adapters` already has generic
  `AdapterArtifactIdentity`, `AdapterPackageManifest`, and
  `AdapterServingBinding` over `DatastreamSubjectKind::AdapterPackage`, but it
  does not know about `.fmadapter`, Apple metadata, or Apple draft-model side
  artifacts.
- `crates/openagents-kernel-core/src/compute.rs` and
  `docs/kernel/compute-training-authority.md` already define generic training
  policy, training run, benchmark package, validator policy, and
  accepted-outcome authority. The Apple kernel work should extend validation and
  metadata discipline on top of those objects rather than inventing a parallel
  authority family.
- `crates/openagents-kernel-core/src/compute_benchmarks.rs` currently exposes
  only one benchmark adapter kind, `mmlu_multiple_choice_v1`. Any Apple lane
  that depends on benchmark import needs explicit new adapter kinds or an
  equally explicit decision to keep Apple validation entirely inside
  `psionic-eval`.
- `crates/openagents-provider-substrate/src/lib.rs` currently derives only
  `AppleFoundationModelsInference`. Later adapter-hosting claims cannot be
  honest until the provider substrate and capability publication layers learn an
  Apple adapter-hosting product family.

## Revised Integration Sequence

Because Python is off the table, the sequence changes materially.

## Phase 1: freeze the Apple spec in our own docs and tests

Before writing runtime code, capture the external behavior as a spec.

Deliverables:

- explicit doc for Apple adapter data format
- explicit doc for `.fmadapter` package layout
- explicit doc for Apple adapter training metadata and required lineage fields
- test fixtures that represent expected package and metadata shapes

Important rule:

- OpenAgents should not execute the Apple Python repos as part of product or
  operator flow
- if fixture generation is needed at all, it should be a one-time manual
  reference step outside the supported OpenAgents runtime path

The real goal is to stop treating the external repos as "tools we call" and
start treating them as "formats we implement."

## Phase 2: port the `.fmadapter` package into `psionic-adapters`

This is the first code target because it unblocks both serving and authority.

Deliverables:

- explicit Apple package format enum or sibling Apple package manifest type
- metadata parser/writer
- package digesting
- file inventory
- base-model signature capture
- optional draft-artifact linkage
- hosted binding lineage hooks

Why this goes first:

- the serving path cannot be truthful without an explicit artifact
- the authority path cannot record accepted outputs cleanly without an explicit
  artifact

## Phase 3: extend the Swift bridge and Rust client for adapter attachment

Before training is productized, serving must be made real.

Deliverables:

- adapter management contract in `psionic-apple-fm`
- adapter attach/list/detach behavior in `swift/foundation-bridge`
- adapter-aware session creation and one-shot completion requests
- bridge health and capability reporting that includes adapter state
- desktop workbench support for choosing an adapter

Why this should happen early:

- it allows OpenAgents to validate imported or prebuilt Apple adapter artifacts
  before the full Rust trainer lands
- it proves the final execution lane is actually able to consume the format we
  plan to train and export

## Phase 4: port Apple dataset and schema semantics into Rust

Deliverables:

- JSONL dataset reader and validator
- role-order and sample validation
- guided-generation schema augmentation
- tool schema augmentation
- tokenizer and prompt-shaping capture
- deterministic packing rules

Likely owners:

- `psionic-data`
- `psionic-apple-fm`
- `psionic-train`

This phase turns the Apple repos' loose Python input handling into typed Rust
contracts.

## Phase 5: implement the Rust adapter trainer

This is the largest engineering step.

Deliverables:

- Rust adapter-only training loop
- optimizer and scheduler support required by the Apple lane
- checkpointing
- train summary metrics
- reproducibility metadata
- exported Apple package generation

This phase should land only when it is honest to say:

- the training computation itself is Rust-owned
- the export is Rust-owned
- no Python is required anywhere in the supported flow

## Phase 6: implement Rust draft-model distillation if we still want it

This is optional and should remain clearly secondary to the base adapter lane.

Deliverables:

- teacher-draft distillation runtime
- draft checkpoint artifact
- optional draft payload in the final Apple package
- latency and acceptance-ratio metrics where relevant

This should not block the first honest adapter-training lane.

## Phase 7: bind the Apple lane into kernel authority

Once training and serving are both real, bind them into canonical market truth.

Deliverables:

- Apple-specific environment refs
- Apple adapter training policies
- benchmark package refs
- validator policy refs
- training run creation and finalize flows
- accepted outcome publication only after eval and Apple-runtime validation

The correct accepted-outcome rule is:

- not "training process completed"
- not "artifact exported"
- but "artifact exported and accepted by the relevant evaluation and Apple
  runtime validation posture"

## Phase 8: ship the product workflow in desktop and CLI

Deliverables:

- WGPUI training pane
- desktop-control status and history
- `autopilotctl` subcommands
- mission-control visibility where appropriate
- session-log persistence and replay-safe state reporting

This is where AFMTrainer's operator flow gets ported into our own system for
real.

## Market And Kernel Implications

## 1. This remains later-family work, not MVP scope

`docs/MVP.md` is still authoritative.

The visible MVP lane is still compute-provider-first.

So Apple adapter training is:

- strategically relevant
- architecturally compatible
- not part of the current product promise

That means the work should be framed as:

- substrate expansion
- future training-family preparation
- future adapter-hosting preparation

Not as "ship this as part of the current MVP loop."

## 2. The compute market should not claim `training` or `adapter_hosting` yet

The compute-market docs already reserve later-family space for:

- `training`
- `adapter_hosting`

That space is useful, but it should not be marketed as live until all of the
following are true:

- Rust-owned training runtime exists
- Apple package artifact truth exists
- Apple serving attachment exists
- benchmark and validation gates exist
- authority receipts and accepted outcomes exist

Before that, the honest statement is:

- OpenAgents is preparing the substrate for later Apple-compatible
  training-class and adapter-hosting products

## 3. Artifact lineage is part of the market claim

For Apple adapter jobs, the economic claim must include at least:

- base-model signature
- tokenizer lineage
- training environment ref and version
- benchmark package refs
- validator policy ref
- package format version
- draft-model presence
- final package digest

These values affect:

- comparability
- admissibility
- settlement
- future hosted serving claims

So they belong in:

- artifact manifests
- training-run metadata
- accepted outcomes

Not in local-only output folders.

## Risks And Friction Points

## 1. The Rust port may expose missing Psionic substrate

The external Apple trainer currently relies on:

- PyTorch
- TAMM
- Apple-provided model assets
- CoreML-related export logic

Recreating that in Rust may reveal missing capabilities in:

- model representation
- optimizer support
- mixed-precision handling
- activation checkpointing
- export serialization

That is not a reason to use Python. It is evidence of where Psionic still needs
to grow.

## 2. The Apple package format may hide undocumented constraints

The Python exporter gives us strong clues, not necessarily a full normative
spec.

So the Rust package port should be built with:

- parser and validator tests
- fixture-based conformance checks
- cautious versioning
- explicit rejection paths for unsupported package variants

This is another reason to port the package early rather than leaving it as an
opaque later task.

## 3. Serving support may still require limited Swift growth

The zero-Python requirement is fully compatible with the current Swift bridge
boundary.

It is not compatible with pretending Swift does not exist.

If adapter attach or packaging requires additional Swift work, that is still
acceptable because:

- the repo already uses Swift for the Apple runtime boundary
- Apple forces that boundary

The unacceptable move is bringing Python back in through a side door.

## 4. The port is larger, but it is architecturally clean

The compatibility-lane plan would have been faster.

The zero-Python plan is slower, but it avoids:

- hidden substrate drift
- inconsistent receipts
- operator dependency sprawl
- a fake Psionic ownership story

It is the cleaner long-term move.

## What We Should Not Do

- Do not shell out to the Apple Python toolkit from `apps/autopilot-desktop`.
- Do not add a `uv` or Python prerequisite to the supported OpenAgents flow.
- Do not vendor AFMTrainer into the repo as a side app.
- Do not hide Python calls behind Rust wrappers and call that Psionic.
- Do not treat `.fmadapter` as generic `safetensors`.
- Do not market training or adapter-hosting products before the Rust trainer,
  Apple package support, serving attachment, and authority gates exist.
- Do not let export success count as accepted economic truth without benchmark
  and Apple-runtime validation.

## Recommended Issue Sequence

The earlier 11-item queue was directionally right but too thin for the actual
repo. The concrete issue program should be:

### 1. `Apple Adapter Spec: freeze dataset, metadata, and .fmadapter contracts in repo docs and fixtures`

- Why this is a real repo gap:
  `psionic-apple-fm`, `psionic-adapters`, and `psionic-train` currently have no
  canonical in-repo `.fmadapter` spec or Apple adapter fixture corpus to code
  against.
- Primary owners:
  docs plus Psionic conformance tests.
- Current codebase anchors:
  `docs/audits/2026-03-14-afmtrainer-apple-adapter-toolkit-integration-audit.md`,
  `crates/psionic/docs/FM_BRIDGE_CONSIDERATIONS.md`.
- Deliverables:
  explicit Apple dataset schema doc, explicit `.fmadapter` package-layout doc,
  explicit Apple training/export metadata doc, and checked-in fixture bundles
  produced from one-time manual reference generation.
- Acceptance:
  later Rust implementations can validate against stable fixture and metadata
  inputs without executing any Python as part of the supported flow.
- Depends on:
  none.

### 2. `Psionic Adapters: add explicit Apple FM package family, parser, writer, and lineage support`

- Why this is a real repo gap:
  `crates/psionic/psionic-adapters/src/lib.rs` only knows generic adapter
  identity plus generic package manifests; it has no Apple-specific format
  family, no `.fmadapter` parser/writer, and no draft-model side-artifact
  handling.
- Primary owners:
  `psionic-adapters`.
- Deliverables:
  Apple package family enum or sibling manifest type, metadata parser/writer,
  digesting, compatibility validation, optional draft-artifact linkage, and
  datastream-backed lineage hooks that still terminate in the existing adapter
  package subject.
- Acceptance:
  a checked-in `.fmadapter` fixture roundtrips through Rust import/export and
  exposes base-model signature, tokenizer lineage, package version, and draft
  artifact presence as typed fields.
- Depends on:
  `1`.

### 3. `Psionic Apple FM: extend the bridge contract for adapter inventory, compatibility, and attach/detach`

- Why this is a real repo gap:
  `crates/psionic/psionic-apple-fm/src/contract.rs` currently exposes only
  health/models/completions/sessions/streaming/structured/tools/transcripts.
  There is no Rust contract for adapter load/list/attach/detach, compatibility
  checks, or adapter-aware session state.
- Primary owners:
  `psionic-apple-fm`.
- Deliverables:
  typed adapter-management request/response contracts, adapter inventory and
  compatibility models, adapter-aware health/capability reporting, and client
  support in both blocking and async bridge clients.
- Acceptance:
  the Rust lane can express adapter lifecycle and session binding without ad hoc
  JSON or desktop-local strings.
- Depends on:
  `1`, `2`.

### 4. `foundation-bridge: implement adapter-aware Apple FM sessions, requests, and capability reporting`

- Why this is a real repo gap:
  `swift/foundation-bridge/Sources/foundation-bridge/Types.swift` and
  `ChatHandler.swift` have no adapter-management or adapter-bound session path.
- Primary owners:
  `swift/foundation-bridge`.
- Deliverables:
  Swift-side adapter inventory/load/unload/attach behavior, session-level or
  request-level adapter binding, compatibility failures surfaced through typed
  errors, and health/model reporting that includes adapter state and any Apple
  entitlement/runtime gate facts.
- Acceptance:
  a Rust client can attach an imported `.fmadapter` to a real Apple FM request,
  detach it, and observe typed compatibility/refusal errors when the artifact is
  incompatible.
- Depends on:
  `3`.

### 5. `Autopilot Desktop: expose Apple adapter inventory and bridge controls through existing workbench, desktop_control, and autopilotctl`

- Why this is a missing issue:
  the repo already has `apple_fm_bridge.rs`, `panes/apple_fm_workbench.rs`,
  `desktop_control.rs`, and `autopilotctl apple-fm`, but none of them can list,
  load, attach, detach, or inspect adapters.
- Primary owners:
  `apps/autopilot-desktop`.
- Deliverables:
  Apple workbench adapter controls, bridge-side adapter inventory in
  desktop-control snapshots, `autopilotctl apple-fm` subcommands for adapter
  management, and replay-safe operator logs for adapter lifecycle actions.
- Acceptance:
  an operator can load a fixture Apple adapter, attach it to a session, inspect
  inventory/status from both UI and CLI, and detach it again without Python.
- Depends on:
  `3`, `4`.

### 6. `Psionic Data: port Apple adapter dataset parsing, tool/schema augmentation, tokenizer capture, and packing rules`

- Why this is a real repo gap:
  the Apple toolkit’s JSONL/messages/`response_format`/tool semantics do not yet
  exist as typed Rust dataset contracts, even though `psionic-data` already owns
  versioned dataset manifests and packing policy.
- Primary owners:
  `psionic-data`, with Apple-specific schema helpers in `psionic-apple-fm`
  where needed.
- Deliverables:
  JSONL parser/validator, role-order and multi-turn validation, Apple guided
  generation schema attachment, tool-contract augmentation, tokenizer digest
  capture, and deterministic packing rules for Apple adapter workloads.
- Acceptance:
  an Apple dataset fixture imports into typed Rust records with explicit refusal
  paths for malformed roles, missing schemas, or tokenizer drift.
- Depends on:
  `1`.

### 7. `Psionic Environments: define Apple adapter train/eval environment package family and parity receipts`

- Why this is a missing issue:
  `psionic-environments` already owns environment package ABI, tool contracts,
  rubric hooks, and train/eval parity receipts, but the audit’s earlier queue
  never assigned the Apple lane to that owner.
- Primary owners:
  `psionic-environments`.
- Deliverables:
  reusable Apple adapter train/eval/benchmark environment package shapes,
  package refs for Apple session/runtime requirements, tool/rubric bindings for
  guided generation and tool-calling tasks, and train/eval parity receipts that
  prove the same pinned environment is reused across both paths.
- Acceptance:
  Apple adapter training and held-out eval can share a versioned environment
  package instead of duplicating prompt/tool logic across app code and eval
  harnesses.
- Depends on:
  `6`.

### 8. `Psionic Eval: add Apple adapter held-out eval, structured-output, tool-call, and runtime-smoke harnesses`

- Why this is a missing issue:
  `psionic-eval` is real, but it currently has no Apple-specific harness for
  exported adapter artifacts, no bridge-smoke validation path, and no benchmark
  contract for structured generation or tool-calling behavior.
- Primary owners:
  `psionic-eval`.
- Deliverables:
  Apple adapter eval-run harnesses, structured-conformance checks, tool-call
  behavior checks, Apple FM runtime smoke receipts over exported packages,
  benchmark summaries, and reusable artifact/metric families suitable for later
  authority publication.
- Acceptance:
  a trained or imported Apple adapter can be evaluated through Rust-owned
  held-out and benchmark-style flows, and those flows emit machine-legible eval
  artifacts instead of “export succeeded” text.
- Depends on:
  `4`, `6`, `7`.

### 9. `Kernel Core: add Apple-specific benchmark adapter kinds and validation discipline for training policy/run metadata`

- Why this is a missing issue:
  `crates/openagents-kernel-core/src/compute.rs` already has generic training
  policy/run/outcome objects, but `compute_benchmarks.rs` only supports
  `mmlu_multiple_choice_v1`, and none of the current validation rules know what
  Apple adapter lineage must be present.
- Primary owners:
  `openagents-kernel-core`.
- Deliverables:
  Apple benchmark adapter kind(s) where benchmark import is required, validation
  helpers for Apple adapter benchmark package metadata, and stronger
  training-policy/training-run metadata rules for base-model signature,
  tokenizer digest, package-format version, draft-model presence, and Apple
  runtime validation posture.
- Acceptance:
  malformed or incomplete Apple adapter policy/run metadata is rejected by
  kernel validation instead of surviving only as nullable JSON blobs.
- Depends on:
  `1`, `8`.

### 10. `Psionic Train: implement Rust-native Apple adapter SFT lane on top of the existing fixed-budget core`

- Why this is a real repo gap:
  `psionic-train` now has the fixed-budget training core, optimizer families,
  distributed-optimizer contracts, and model-IO portability layer, but no
  Apple-specific adapter-only training path.
- Primary owners:
  `psionic-train`, with support from `psionic-ir`/`model_io` as needed.
- Deliverables:
  adapter-only parameter selection, base-model freeze semantics, Apple dataset
  batching, Apple-relevant mixed-precision and activation-checkpoint policy,
  checkpoint restore/export, train summary metrics, and `.fmadapter` export
  through the Rust-owned package layer.
- Acceptance:
  one small Apple-compatible fixture dataset can train adapter-only weights and
  emit a valid `.fmadapter` package plus typed training summary without Python.
- Depends on:
  `2`, `6`, `7`.

### 11. `Psionic Train: add optional Rust-native Apple draft-model distillation lane`

- Why this is still a separate issue:
  draft-model distillation is materially different from plain adapter SFT and
  should not block the first honest Apple adapter lane.
- Primary owners:
  `psionic-train`.
- Deliverables:
  teacher/draft runtime pairing, distillation objective, dual-model precision
  policy, draft checkpoint artifact, optional draft payload in the exported
  Apple package, and latency/acceptance-ratio metrics where relevant.
- Acceptance:
  the draft-model path is either real and Rust-owned or still absent; it is not
  hand-waved as “later maybe” inside the base SFT issue.
- Depends on:
  `10`.

### 12. `Nexus: persist, project, and receipt Apple adapter training runs and accepted outcomes through the existing training authority`

- Why this is more than “add an endpoint”:
  the generic training authority already exists, and `apps/autopilot-desktop`
  already projects it through `desktop_control` training status. The missing
  work is the Apple-specific create/finalize/publication discipline on top of
  those existing surfaces.
- Primary owners:
  `apps/nexus-control`, plus the typed authority client path.
- Deliverables:
  Apple training-run create/finalize flows over the existing registry objects,
  preservation of Apple metadata/benchmark refs/validator refs, accepted-outcome
  publication only after eval plus Apple runtime validation, and read-model
  projection that desktop-control can already consume.
- Acceptance:
  a successful Apple adapter run appears as a canonical `ComputeTrainingRun`
  and, only after acceptance gates pass, as a `ComputeAcceptedOutcome` that the
  current desktop-control training snapshot can display.
- Depends on:
  `8`, `9`, `10`.

### 13. `Provider Substrate: add Apple adapter-hosting product derivation, capability publication, and settlement linkage`

- Why this is a missing issue:
  `crates/openagents-provider-substrate/src/lib.rs` currently derives only
  `AppleFoundationModelsInference`; there is no Apple adapter-hosting product
  family even though `psionic-provider` already has generic `adapter_serving`
  receipt linkage.
- Primary owners:
  `openagents-provider-substrate` plus `psionic-provider`.
- Deliverables:
  explicit Apple adapter-hosting product(s), capability summaries that include
  adapter/runtime compatibility truth, inventory derivation from adapter-aware
  Apple FM readiness, and provider receipts that carry the served adapter digest
  through execution and settlement linkage.
- Acceptance:
  the provider can only advertise Apple adapter-hosting supply when compatible
  adapter artifacts and Apple runtime state are both real, and the resulting
  execution receipts preserve adapter identity.
- Depends on:
  `2`, `4`, `5`, `12`.

### 14. `Autopilot Desktop: ship the Apple adapter training operator workflow on top of existing Apple FM and training status surfaces`

- Why this is broader than a single pane:
  the repo already has an Apple workbench, training status read models, and
  local research state in `research_control.rs`, but no integrated Apple
  adapter operator flow.
- Primary owners:
  `apps/autopilot-desktop`.
- Deliverables:
  WGPUI setup/training/export/monitoring panes, desktop-control mutations beyond
  plain `GetTrainingStatus`, `autopilotctl` subcommands for train/export/accept
  operations, persisted session logs, and, where it makes sense, reuse of the
  existing research-control frontier/promotion surfaces rather than inventing a
  second experiment registry in app code.
- Acceptance:
  an operator can move from imported dataset to launched Apple adapter training
  run to exported artifact to accepted outcome using only desktop/CLI surfaces
  that replay their state after restart.
- Depends on:
  `5`, `10`, `12`.

### 15. `Compute Market: only after the above, productize Apple training and adapter-hosting claims`

- Why this must stay last:
  the market docs already reserve later `training` and `adapter_hosting`
  families, but the repo cannot honestly advertise them until training runtime,
  Apple package truth, serving attachment, eval gates, authority receipts, and
  provider product derivation are all real.
- Primary owners:
  kernel-market docs plus the provider/product layers.
- Deliverables:
  compute-market doc updates, provider inventory/product exposure, truthful
  capability envelopes, and any later buyer/provider UX only after the runtime
  and authority stack is complete enough to defend the claim.
- Acceptance:
  OpenAgents can state that Apple-compatible training or Apple adapter-hosting
  is a real compute-market family without depending on Python, desktop-local
  truth, or unvalidated export artifacts.
- Depends on:
  `12`, `13`, `14`.

## Bottom Line

The integration answer is now unambiguous:

- yes to Rust-owned integration
- yes to zero Python
- yes to using the Apple repos as reference material
- yes to keeping the existing Swift bridge where Apple runtime APIs require it
- no to any Python compatibility lane

So the work is not:

- "wrap AFMTrainer"
- "call Apple's toolkit from the desktop app"

The work is:

- port the Apple semantics into Rust
- port the Apple package into Psionic artifacts
- extend the Swift bridge for adapter attachment
- publish training truth through kernel authority

That is the only version of this integration that is consistent with the
current Psionic docs, the compute-market owner split, and the stated
zero-Python requirement.
