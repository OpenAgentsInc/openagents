# 2026-03-13 Apple Silicon vs NVIDIA GPT-OSS Parity Audit

## Scope

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/v01.md`
- `docs/plans/mission-control-pane.md`
- `README.md`
- `AGENTS.md`
- `docs/headless-compute.md`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/input/reducers/local_inference.rs`
- `apps/autopilot-desktop/src/local_inference_runtime.rs`
- `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `apps/autopilot-desktop/src/kernel_control.rs`
- `apps/autopilot-desktop/src/state/provider_runtime.rs`
- `crates/openagents-provider-substrate/src/lib.rs`
- `swift/foundation-bridge/README.md`
- `crates/psionic/docs/FM_BRIDGE_CONSIDERATIONS.md`
- `docs/audits/2026-03-10-apple-fm-swift-bridge-audit.md`
- `docs/audits/2026-03-09-gpt-oss-local-inference-pane-audit.md`
- `docs/audits/2026-03-08-psionic-vs-llama-cpp-gpt-oss-performance-audit.md`

## Executive Summary

The repo did not truly delete GPT-OSS. It removed GPT-OSS from the `v0.1` product promise and from the production Mission Control story, but the retained tree still contains a live Psionic GPT-OSS runtime seam, a dedicated `GPT-OSS Workbench` pane, provider/kernel inventory paths, and significant CUDA/Metal engine work.

The difference today is not "Apple exists, NVIDIA does not." The difference is:

- Apple FM is productized.
- NVIDIA GPT-OSS is engine-capable but product-demoted.

Apple Silicon support currently wins on product integration:

- Mission Control chooses Apple FM by policy on macOS.
- the desktop control plane and `autopilotctl` know how to refresh and wait for Apple FM
- packaged release verification bundles and checks `foundation-bridge`
- release docs, README instructions, and AGENTS guidance are Apple FM-first

NVIDIA GPT-OSS currently wins on raw local-model control:

- the app-owned `LocalInferenceRuntime` seam is already a Psionic GPT-OSS seam
- the runtime can choose CPU, CUDA, or Metal backends
- the workbench exposes warm, unload, prompt execution, GGUF path, backend label, and execution provenance
- the underlying Psionic CUDA path has strong recent performance evidence in this repo

The main parity blocker is architectural drift, not missing inference code. GPT-OSS is split across four incompatible identities:

- `MissionControlLocalRuntimeLane::NvidiaGptOss`
- `PsionicGptOssRuntimeAdapter`
- `ProviderBackendKind::Ollama`
- capability/provenance strings that sometimes say `psionic`

Until those identities collapse into one truthful runtime model, Mission Control, the provider substrate, the control plane, and release docs will keep favoring Apple FM as the only coherent story.

The right plan is not to resurrect archived code. The retained tree already has the right starting seams:

- `apps/autopilot-desktop/src/local_inference_runtime.rs`
- `apps/autopilot-desktop/src/panes/local_inference.rs`
- `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- `crates/openagents-provider-substrate/src/lib.rs`

Parity work should build on those retained seams and change product policy, naming, control-plane support, and Mission Control rendering around them.

## What "Apple Silicon Support" Means In This Repo

There are two separate Apple-related stories in the current tree, and they should not be conflated:

1. Shipped macOS local runtime
   - Apple Foundation Models via the Swift sidecar in `swift/foundation-bridge/`
   - this is the `v0.1` release story in `docs/v01.md`

2. Hidden Psionic Metal GPT-OSS candidate
   - `default_local_inference_runtime()` still boots `PsionicGptOssRuntimeAdapter::new_auto()`
   - on macOS, that adapter still prefers `metal` before `cpu`
   - this is not the product lane Mission Control uses on macOS today

That distinction matters because "Apple Silicon support" in the shipped UX currently means Apple FM, not Metal GPT-OSS.

## Current State By Layer

| Layer | Apple FM / Apple Silicon | NVIDIA GPT-OSS |
| --- | --- | --- |
| Product status | First-class release path | Dev-only/internal path |
| Mission Control policy | Hard-selected on macOS by `mission_control_uses_apple_fm()` | Selected only when not macOS, in dev mode, and runtime backend label is `cuda` |
| Runtime architecture | Rust worker supervising Swift localhost sidecar | In-process app-owned Psionic runtime |
| Pane visibility | Mission Control + Apple FM Workbench | Separate `GPT-OSS Workbench`, hidden on macOS |
| Control plane | `desktop_control` and `autopilotctl` expose Apple FM actions and readiness waits | No equivalent GPT-OSS-specific control-plane actions or wait conditions |
| Packaging / release verification | Bundled `foundation-bridge`, packaged roundtrip script checks it | No packaged GPT-OSS artifact/runbook parity |
| Provider substrate identity | Truthful `apple_foundation_models.*` naming | Still largely expressed as `ollama.*` and `ProviderBackendKind::Ollama` |
| Provider priority | Preferred when both backends are ready | Secondary backend when Apple FM is ready |
| Workbench features | Sessions, streaming, structured generation, tool profiles, transcript export/restore | Warm/unload, model path, backend label, prompt playground, metrics/provenance |
| Documentation quality | Explicit and current | Mostly historical/perf audits, not current product docs |

## Repo-Grounded Findings

### 1. GPT-OSS was removed from the MVP UX, not from the retained implementation

`docs/v01.md` explicitly narrowed `v0.1` to:

- Apple Silicon Mac
- Apple Foundation Models
- Mission Control as the only production shell
- no GPT-OSS copy in the production Mission Control surface

But the code still retains GPT-OSS in several active places:

- `default_local_inference_runtime()` returns `PsionicGptOssRuntimeAdapter::new_auto()`
- `RenderState` always stores `local_inference_runtime` plus `ollama_execution`
- `PaneKind::LocalInference` is still the `GPT-OSS Workbench`
- `MissionControlLocalRuntimeLane` still has `NvidiaGptOss`

So the correct statement is: GPT-OSS is hidden and policy-gated, not gone.

### 2. Apple FM is more product-complete than GPT-OSS

Apple FM has a full app-owned supervision and operational story:

- `apple_fm_bridge.rs` supports `Refresh`, `EnsureBridgeRunning`, `StopBridge`, provider `Generate`, rich workbench commands, and Mission Control summary streaming
- Mission Control inline actions know how to start or refresh Apple FM
- `desktop_control.rs` exposes `RefreshAppleFm`
- `autopilotctl` exposes `apple-fm-ready`
- `scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh` bundles and waits for `foundation-bridge`
- README and AGENTS instructions explain how to build, run, and verify the bridge

GPT-OSS does not have comparable end-to-end product wiring:

- no GPT-OSS-specific desktop-control action
- no `autopilotctl` wait condition for GPT-OSS ready
- no packaged release verification contract for GGUF artifact presence and CUDA readiness
- no current release docs describing a supported NVIDIA host flow

### 3. GPT-OSS is actually better integrated into the generic local runtime seam

Apple FM is better integrated into product surfaces, but GPT-OSS is better integrated into the generic local runtime abstraction.

`local_inference_runtime.rs` already defines the app-owned local-model seam, and its default implementation is GPT-OSS:

- `LocalInferenceRuntime`
- `LocalInferenceRuntimeCommand`
- `LocalInferenceRuntimeUpdate`
- `PsionicGptOssRuntimeAdapter`

That runtime already handles:

- backend selection: `cpu`, `cuda`, `metal`, `auto`
- GGUF artifact discovery
- warm/load
- unload
- prompt execution
- request metrics and provenance

Apple FM, by contrast, still lives beside that seam as a separate worker with a separate snapshot type and separate workbench state.

So the repo today has an inversion:

- the shipped product lane is Apple FM
- the app’s generic local-runtime abstraction is GPT-OSS

That split is the core architectural reason parity feels poor.

### 4. Mission Control is hardcoded to Apple policy, not driven by a generic runtime policy model

The critical Mission Control gating logic is:

- `mission_control_uses_apple_fm()` -> `cfg!(target_os = "macos")`
- `mission_control_supports_cuda_gpt_oss()` -> only when not macOS and backend label is `cuda`
- `mission_control_sell_compute_supported_for_platform()` -> Apple FM always wins on macOS; CUDA GPT-OSS is only a dev-mode exception

This means Mission Control is not currently expressing "active local runtime truth." It is expressing "release-cut platform policy."

That was correct for `v0.1`, but it is the first thing that must change if GPT-OSS is returning as a first-class supported lane.

### 5. Mission Control copy is still lane-specific in a way that blocks GPT-OSS parity

Current Mission Control behavior:

- on macOS: `START APPLE FM`, `REFRESH APPLE FM`, `OPEN APPLE FM`
- on NVIDIA dev path: `OPEN GPT-OSS WORKBENCH`
- Go Online hints and log lines on the Apple path speak only Apple FM truth
- Go Online hints on the NVIDIA path intentionally redirect the user to the separate GPT-OSS pane

This matches `docs/plans/mission-control-pane.md`, which explicitly says GPT-OSS-specific loading and troubleshooting belong in the separate workbench.

The user request is a different product decision: bring GPT-OSS back and have the Mission Control local-runtime area show GPT-OSS truth instead of Apple-Silicon-only truth where appropriate.

That means the current Mission Control doc and implementation are intentionally out of alignment with the requested future state.

### 6. Pane visibility still encodes the old MVP demotion

`pane_registry.rs` disables `PaneKind::LocalInference` on macOS:

- the GPT-OSS pane is hidden on macOS
- command-palette/runtime resolution also rejects GPT-OSS pane names on macOS

This is stronger than "not release-facing." It is "not even visible on the Apple build."

If GPT-OSS is returning as a supported product lane, this needs to become policy-driven instead of compile-target-driven.

### 7. Naming drift is the single biggest truth problem

The same NVIDIA GPT-OSS lane currently appears as:

- `GPT-OSS Workbench`
- `PsionicGptOssRuntimeAdapter`
- `state.ollama_execution`
- `provider_runtime.ollama`
- `ProviderBackendKind::Ollama`
- `ProviderBlocker::OllamaUnavailable`
- kernel product ids `ollama.text_generation` and `ollama.embeddings`
- capability metadata that sometimes uses backend string `psionic`

This is not cosmetic debt. It creates concrete parity problems:

- Mission Control cannot render truthful labels from one stable model
- provider inventory and kernel receipts mix old and new names
- automation and docs cannot describe one stable NVIDIA/GPT-OSS contract
- future parity work risks copying the same conceptual backend under yet another name

Until this is cleaned up, GPT-OSS support will keep looking half-restored even when the code works.

### 8. The provider/kernel layer overstates GPT-OSS capability today

The current local GPT-OSS runtime seam only exposes:

- `Refresh`
- `WarmConfiguredModel`
- `UnloadConfiguredModel`
- `Generate`

There is no embeddings command in the app-owned runtime seam.

But the provider substrate and kernel layer still advertise:

- `ProviderComputeProduct::OllamaEmbeddings`
- `ollama.embeddings`
- `meter.ollama.embeddings.v1`
- launch bindings and forward inventory for text embeddings

This is a truth gap independent of Apple FM parity. If GPT-OSS is brought back, the advertised product set must match what the runtime can actually do.

Short version:

- Apple FM is narrower, but more honest
- GPT-OSS is broader on paper, but less honest in product naming and capability projection

### 9. Apple FM and GPT-OSS each own the wrong half of the parity story

Apple FM currently has the stronger interaction/workbench model:

- sessions
- streaming
- structured generation
- tool callbacks
- transcript export/restore
- Mission Control-local smoke test

GPT-OSS currently has the stronger runtime operations model:

- model artifact path
- backend selection
- explicit warm/load/unload
- runtime residency view
- raw backend/provenance metrics

Parity does not mean "copy Apple onto GPT-OSS" or "copy GPT-OSS onto Apple" mechanically.

Parity means combining:

- Apple FM’s product and control-plane completeness
- GPT-OSS’s runtime and model-control completeness

### 10. Apple FM has current ops docs; GPT-OSS mainly has engineering history

Apple FM is documented in:

- `README.md`
- `AGENTS.md`
- `docs/v01.md`
- `docs/plans/mission-control-pane.md`
- `crates/psionic/docs/FM_BRIDGE_CONSIDERATIONS.md`
- packaged roundtrip scripts and Mission Control control-plane docs

GPT-OSS is mainly documented in:

- historical audits
- performance audits
- local integration notes

That is enough for engineering continuity, but not enough for a supported product lane.

### 11. The control plane is Apple-biased today

`desktop_control.rs` and `autopilotctl.rs` currently treat Apple FM as the named local-runtime domain:

- refresh Apple FM
- wait for Apple FM ready
- run Apple FM smoke test

There is no equivalent generic model such as:

- refresh local runtime
- wait for active local runtime ready
- wait for GPT-OSS ready
- warm/unload/test GPT-OSS

If Mission Control is going to speak GPT-OSS truth, the programmatic control plane has to speak it too. Otherwise the UI and automation will drift again.

## Bottom-Line Comparison

Apple FM is ahead in:

- Mission Control integration
- release-policy alignment
- control-plane support
- packaging and startup story
- current docs

NVIDIA GPT-OSS is ahead in:

- generic runtime abstraction
- explicit backend/model control
- local model provenance and observability
- recent engine performance evidence

The repo already contains enough retained GPT-OSS code to restore first-class support without reopening archived backroom code. The missing work is mostly productization, naming cleanup, and truth-model unification.

## Recommended Target State

The target should be:

- `macOS / Apple Silicon` -> Apple FM remains a first-class local-runtime lane
- `Linux + NVIDIA CUDA` -> GPT-OSS becomes a first-class local-runtime lane
- Mission Control renders the active lane truth, not hardcoded Apple-only copy
- the provider substrate and kernel inventory use truthful, stable backend/product names
- the desktop control plane and packaged verification support both lanes

This does not require making both lanes identical.

Parity should mean:

- equal first-class status in Mission Control
- equal truth in provider inventory and receipts
- equal automation surface
- equal documentation/runbook quality
- equal validation discipline

It does not require:

- Apple FM to gain GGUF artifact management
- GPT-OSS to gain Apple-style session semantics on day one

## Multi-Step Plan To Bring GPT-OSS To Parity

### Phase 1. Replace the hardcoded platform gate with an app-owned runtime policy layer

Add an explicit app-owned policy model for the Mission Control local-runtime lane, for example:

- `AppleFoundationModels`
- `GptOssCuda`
- `GptOssMetal`
- `None`

Drive Mission Control from that policy model instead of:

- `cfg!(target_os = "macos")`
- ad hoc `desktop_shell_mode.is_dev()`
- string checks on `backend_label == "cuda"`

Exit criteria:

- Mission Control decides which local-runtime story to render from one policy function
- the policy can be widened to support GPT-OSS without rewriting pane logic

### Phase 2. Rename the retained NVIDIA lane from `Ollama` to a truthful backend identity

Do this in app-owned code and the provider substrate:

- `ProviderBackendKind::Ollama` -> `ProviderBackendKind::GptOss` or `ProviderBackendKind::PsionicGptOss`
- `ProviderOllamaRuntimeState` -> `ProviderGptOssRuntimeState`
- `state.ollama_execution` -> `state.gpt_oss_execution` or `state.local_model_execution`
- product ids `ollama.*` -> `gpt_oss.*` once migration plan is set
- blocker names and UI labels should stop saying `Ollama`

Keep this rename small and mechanical. The goal is one stable identity, not a broad redesign.

Exit criteria:

- one backend has one name everywhere
- Mission Control, kernel inventory, receipts, logs, and tests all use the same terminology

### Phase 3. Make the provider/kernel contract honest before widening the UX

Fix capability drift first:

- stop advertising embeddings on the GPT-OSS lane until the app-owned runtime seam supports them
- update metering rule ids and compute-product registration to the renamed GPT-OSS family
- make `preferred_provider_compute_capability()` publish the same backend name that inventory and receipts use

Exit criteria:

- no `ollama`/`psionic`/`gptoss` split in protocol-facing metadata
- advertised products exactly match executable capabilities

### Phase 4. Introduce one Mission Control local-runtime view model

Mission Control should render one generic local-runtime card with lane-specific fields.

For Apple FM, render:

- bridge status
- model availability
- ready model
- Apple-specific recovery hints

For GPT-OSS, render:

- backend (`cuda`, `metal`, `cpu`)
- model artifact path / artifact presence
- loaded vs unloaded state
- ready model
- last runtime error

The important change is not visual. It is moving Mission Control from Apple-specific copy to lane-aware truth.

Exit criteria:

- on NVIDIA hosts, Mission Control shows GPT-OSS-specific readiness instead of Apple-Silicon-specific copy
- on Apple hosts, Mission Control still shows Apple FM truth

### Phase 5. Decide how much GPT-OSS control belongs inline in Mission Control

The user request implies more inline GPT-OSS presence than the current `OPEN GPT-OSS WORKBENCH` redirect.

Recommended staged approach:

1. First parity cut:
   - Mission Control shows GPT-OSS state truthfully
   - inline action can still open the workbench
2. Second parity cut:
   - add inline `REFRESH GPT-OSS`
   - add inline `LOAD/WARM GPT-OSS`
   - optionally add inline `UNLOAD GPT-OSS`

Do not start by duplicating the full workbench inside Mission Control.

Exit criteria:

- Mission Control no longer tells NVIDIA users to manage a hidden secondary story
- inline actions are enough to get to `Go Online` honestly

### Phase 6. Bring the control plane to generic local-runtime parity

Add control-plane concepts that work for both Apple FM and GPT-OSS:

- `refresh local runtime`
- `wait for local runtime ready`
- `wait for GPT-OSS ready`
- `warm/load GPT-OSS`
- optional `unload GPT-OSS`
- optional `run local runtime smoke test`

Then map `autopilotctl` and `desktop_control` to the same Mission Control truth model the UI uses.

Exit criteria:

- automation can validate Apple FM and GPT-OSS through the same control-plane style
- Apple-specific actions are no longer the only named local-runtime contract

### Phase 7. Unify the workbench story behind the same local-runtime seam

There are two options:

1. Keep two workbenches but make them share a common action/state contract
2. Create one `Local Runtime Workbench` with lane-specific sections

The second option is cleaner long-term, but the first option is lower-risk.

Near-term recommendation:

- keep both panes
- add a shared app-owned "local runtime capability surface" for:
  - refresh
  - readiness
  - run text
  - streaming support
  - structured support
  - model-management support

This lets Mission Control talk to one local-runtime service model without forcing Apple FM and GPT-OSS to expose identical features immediately.

Exit criteria:

- workbench capabilities are described in one shared model
- Mission Control does not need special-case knowledge of two unrelated runtime implementations

### Phase 8. Restore docs and release/runbook parity

Once the implementation is real, update the product and ops docs:

- `docs/v01.md` if the release promise widens beyond Apple FM only
- `docs/plans/mission-control-pane.md` because it currently says GPT-OSS detail belongs outside Mission Control
- `README.md` with supported NVIDIA/GPT-OSS host instructions
- `docs/headless-compute.md` with GPT-OSS control-plane flows
- packaged or headless verification docs for GGUF artifact and CUDA readiness

Do not update these docs before the product policy changes in code. Right now they correctly describe the narrower release cut.

Exit criteria:

- Apple FM and GPT-OSS both have current operator docs
- Mission Control docs match the actual local-runtime behavior

### Phase 9. Add validation gates for GPT-OSS parity

Minimum parity validation should include:

- unit tests for the runtime policy selector
- Mission Control rendering tests for Apple FM and GPT-OSS states
- provider inventory tests proving truthful product ids and backend names
- desktop-control snapshots that include GPT-OSS readiness
- one scripted NVIDIA host smoke test:
  - GGUF present
  - runtime warm/load succeeds
  - Mission Control shows ready
  - `Go Online` gate unlocks

If product scope includes packaged NVIDIA verification later, add a packaged roundtrip equivalent to the Apple FM bundle check.

Exit criteria:

- GPT-OSS parity is enforced by tests, not by memory

## Recommended Order Of Work

Do the bring-back in this order:

1. Runtime policy model
2. Naming cleanup
3. Honest provider/kernel contract
4. Mission Control local-runtime view model
5. Control-plane parity
6. Inline GPT-OSS Mission Control actions
7. Workbench convergence
8. Docs and release/runbook updates
9. Validation gates

This order keeps the work within current MVP ownership rules and avoids rebuilding surfaces on top of stale names.

## Risks If We Do This In The Wrong Order

- If we widen Mission Control before fixing naming, the UI will surface more `ollama`/`psionic`/`gptoss` inconsistency.
- If we restore GPT-OSS copy before fixing provider/kernel contracts, receipts and product ids will stay misleading.
- If we add NVIDIA product docs before the control plane exists, docs will outpace reality.
- If we try to revive archived code, we will likely reintroduce pre-prune complexity instead of using the retained MVP seams.

## Final Recommendation

Start pulling GPT-OSS back in, but do it as a first-class lane reactivation, not as a pane-only resurrection.

The immediate objective should be:

- make Mission Control lane-aware
- make the NVIDIA lane truthful
- make automation and docs understand GPT-OSS as a supported local-runtime family

Once that is done, Apple FM and NVIDIA GPT-OSS will finally be comparable on the dimensions that matter for product parity:

- what Mission Control says
- what `Go Online` gates on
- what the provider advertises
- what the control plane can verify
- what the docs promise

Today Apple FM leads because it owns those dimensions. GPT-OSS already has enough retained runtime substance to catch up once the product layer stops treating it as an old hidden exception.
