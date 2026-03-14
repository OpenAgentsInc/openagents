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
| Dataset and tokenizer contracts | `psionic-data` and related Psionic crates | App-owned validation-only copies |
| Benchmark and eval execution | `psionic-eval` | Canonical accepted-outcome authority |
| Policies, training runs, outcomes, receipts | `openagents-kernel-core` plus `apps/nexus-control` | Desktop-local truth as authority |

This is the current repo architecture applied consistently to the Apple lane.

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

1. `Apple Adapter Spec: document dataset, metadata, and .fmadapter package contracts`
2. `Psionic Adapters: add explicit Apple FM adapter package identity and manifest support`
3. `Psionic Apple FM: add adapter attach/list/detach contract`
4. `foundation-bridge: implement adapter-aware Apple FM sessions and completions`
5. `Psionic Data: port Apple adapter dataset and schema augmentation rules to Rust`
6. `Psionic Train: implement Rust-native Apple adapter SFT lane`
7. `Psionic Train: add optional Rust-native draft-model distillation lane`
8. `Kernel: define Apple adapter training policy, benchmark, and validator registry surfaces`
9. `Nexus: publish Apple adapter training runs and accepted outcomes`
10. `Autopilot Desktop: ship WGPUI Apple adapter training pane and autopilotctl controls`
11. `Compute Market: only then productize training and later adapter_hosting claims`

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
