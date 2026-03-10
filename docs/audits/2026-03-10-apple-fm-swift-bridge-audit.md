# Apple Foundation Models Swift Bridge Audit

Date: 2026-03-10
Branch audited: `main`
Scope: current retained-tree state for Apple Foundation Models via the Swift bridge in `apps/autopilot-desktop`, in light of the new MVP decision:

- do not ship GPT-OSS-on-Metal for Mac
- do keep shipping GPT-OSS via Psionic CUDA where that path is good
- on Mac, the local on-device story should be Apple Foundation Models via the existing Swift bridge

## Executive Summary

The Apple Foundation Models path is more real than a placeholder, but it is not yet integrated deeply enough to be the Mac MVP local-model lane.

What is real today:

- there is an active Swift package at `swift/foundation-bridge/`
- it builds on this machine and exposes a small localhost HTTP surface
- `apps/autopilot-desktop` has a real Rust worker that supervises that sidecar, polls health, starts/stops it, and issues completions
- provider-mode plumbing already knows how to prefer Apple FM for inference when it is ready
- Apple FM state is surfaced through provider runtime, kernel inventory, NIP-90 capability selection, delivery-proof metadata, and desktop status UI
- targeted bridge tests in `autopilot-desktop` pass

What is not true today:

- Apple FM is not the app’s first-class local inference runtime for the user-facing workbench/chat lane
- the default local inference seam still boots Psionic GPT-OSS, and on macOS its auto backend preference still points at Metal before CPU
- the Apple FM bridge contract is minimal and lossy: no streaming, no tool use, no session reset API, no structured multi-turn control from the desktop seam, and only approximate token accounting
- the current Swift handler keeps a single shared `LanguageModelSession` alive across requests, which is not a safe request-isolation contract for MVP local inference
- Apple FM request parameters from provider jobs are mostly ignored
- build/distribution integration is still sidecar/manual, not productized packaging

Bottom line:

- the repo already contains a usable starting point for `Mac = Apple FM`
- it is good enough to audit and build on
- it is not yet integrated enough to call the Mac local-model lane "done"
- the first real product move should be to make Apple FM a peer implementation of the app-owned `LocalInferenceRuntime` seam, and on macOS make that the default instead of Psionic GPT-OSS/Metal

## Audit Inputs

Primary files inspected:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- `apps/autopilot-desktop/src/local_inference_runtime.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/kernel_control.rs`
- `crates/openagents-provider-substrate/src/lib.rs`
- `swift/foundation-bridge/Package.swift`
- `swift/foundation-bridge/Sources/foundation-bridge/ChatHandler.swift`
- `swift/foundation-bridge/Sources/foundation-bridge/Server.swift`
- `swift/foundation-bridge/Sources/foundation-bridge/Types.swift`
- `swift/foundation-bridge/build.sh`
- `swift/foundation-bridge/README.md`

Verification run during this audit:

- `cargo test -p autopilot-desktop apple_fm_bridge::tests:: -- --nocapture`
- `./build.sh` in `swift/foundation-bridge/`

Observed result:

- the three Rust-side Apple FM bridge tests passed
- the Swift bridge built successfully and produced `bin/foundation-bridge`

## Current Architecture

There are currently two separate local-model lanes in the desktop app:

1. `Apple FM bridge` lane
   - implemented as a localhost Swift sidecar supervised by Rust
   - lives in `apps/autopilot-desktop/src/apple_fm_bridge.rs`
   - used today mainly by provider/runtime plumbing

2. `Local inference runtime` lane
   - implemented as an app-owned Rust trait in `apps/autopilot-desktop/src/local_inference_runtime.rs`
   - currently defaults to `PsionicGptOssRuntimeAdapter::new_auto()`
   - on macOS, that auto path still prefers `Metal` then `Cpu`

That split is the single most important integration fact in the tree right now.

The Apple FM bridge is real, but it is not the default local inference seam. The default local inference seam is still the Psionic GPT-OSS runtime.

## What Is Actually Landed

### 1. There is a real Swift bridge package in-tree

`swift/foundation-bridge/Package.swift` defines an executable package targeting `macOS(.v26)` with the `foundation-bridge` product.

The Swift side implements:

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions`

That is not a doc stub. It is a buildable executable with a retained API surface.

### 2. The bridge is supervised by the desktop app

`apps/autopilot-desktop/src/apple_fm_bridge.rs` implements:

- a worker thread
- command queue
- update queue
- local child-process supervision
- health polling
- explicit `Refresh`, `EnsureBridgeRunning`, `StopBridge`, and `Generate` commands

It finds the sidecar binary from:

- `OPENAGENTS_APPLE_FM_BRIDGE_BIN`
- `bin/foundation-bridge`
- `swift/foundation-bridge/.build/release/foundation-bridge`
- `swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge`

That means the app already knows how to supervise the Swift bridge as an app-owned localhost sidecar.

### 3. App startup already instantiates the Apple FM worker

`apps/autopilot-desktop/src/render.rs` creates `AppleFmBridgeWorker::spawn()` during app boot and queues an initial `Refresh`.

So the sidecar is not hidden behind a developer-only path. The app always has the worker present.

### 4. Provider-mode lifecycle already uses Apple FM

When the user goes online, `apps/autopilot-desktop/src/input.rs` does all of the following:

- refreshes Apple FM health
- asks the worker to ensure the bridge is running
- refreshes the local inference runtime
- stops the Apple FM bridge when provider mode goes offline

This is real product wiring, not just tests.

### 5. Provider execution can already run through Apple FM

`apps/autopilot-desktop/src/input/reducers/jobs.rs` has a full Apple FM provider execution path:

- request preflight and accept-block logic
- queueing generation through the bridge worker
- started/completed/failed handling
- active-job state updates
- execution provenance capture
- NIP-90 capability sync integration

If Apple FM is the active inference backend, text-generation provider jobs can already execute through it.

### 6. Provider/backend truth already prefers Apple FM when ready

`crates/openagents-provider-substrate/src/lib.rs` makes `ProviderAvailability::active_inference_backend()` prefer `AppleFoundationModels` over `Ollama`.

`apps/autopilot-desktop/src/kernel_control.rs` and `apps/autopilot-desktop/src/input/reducers/jobs.rs` then flow that preference through:

- inventory registration
- launch compute bindings
- capability envelopes
- metering rule IDs
- request routing
- provider execution backend selection

So the higher-level provider product model already conceptually understands `Apple FM on Mac`.

### 7. The desktop UI already shows Apple FM state

`apps/autopilot-desktop/src/pane_renderer.rs` exposes Apple FM readiness and blocker state in provider surfaces:

- Apple FM ready/degraded/offline status
- active backend label
- serving model
- dependency status

So the integration is visible in UI truth surfaces even though it is not yet first-class in the local workbench/chat lane.

## What The Swift Bridge Actually Does

The Swift bridge is intentionally small.

`swift/foundation-bridge/Sources/foundation-bridge/ChatHandler.swift`:

- checks `SystemLanguageModel.default.availability`
- maps Apple availability reasons into human-readable messages
- creates a `LanguageModelSession`
- answers requests with `session.respond(to:)`

`swift/foundation-bridge/Sources/foundation-bridge/Server.swift`:

- listens on localhost TCP with `NWListener`
- parses raw HTTP itself
- exposes health/models/chat-completions
- returns JSON responses only

`swift/foundation-bridge/Sources/foundation-bridge/Types.swift`:

- defines a small OpenAI-ish request/response schema
- includes `model`, `messages`, `temperature`, `max_tokens`, and `stream`
- but only the simplest response path is actually used

This is enough to be useful, but it is still a thin bridge, not a complete app-serving runtime.

## Strengths Of The Current Integration

### 1. The ownership shape is correct

The retained implementation lives in `apps/autopilot-desktop`, which matches `docs/OWNERSHIP.md`. This is app-specific runtime orchestration and should stay app-owned.

### 2. The sidecar supervision model is pragmatic

The app does not try to link Foundation Models directly into Rust. It supervises a localhost Swift executable and talks to it over HTTP. For MVP, that is a reasonable boundary.

### 3. Apple platform gating is explicit

The current code is honest about:

- macOS-only support
- Apple Silicon requirement
- Apple Intelligence availability
- model readiness

That truth shows up in:

- Swift `/health`
- Rust-side snapshot state
- provider blockers
- kernel capability envelopes

### 4. The provider lane is already meaningfully wired

The Apple FM path is not trapped in a demo. It already participates in:

- online/offline lifecycle
- inventory projection
- provider selection
- request execution
- result provenance
- provider-admin health events

That is substantial retained value.

### 5. There is existing test coverage

The Rust bridge worker has targeted tests for:

- healthy bridge refresh/generation
- unavailable bridge
- misconfigured base URL

That is a better starting point than a blank integration.

## The Main Gaps

### 1. Apple FM is not the default local inference runtime

This is the biggest gap relative to the MVP direction.

`apps/autopilot-desktop/src/local_inference_runtime.rs` still defines the app-owned local runtime seam, and `default_local_inference_runtime()` still returns `PsionicGptOssRuntimeAdapter::new_auto()`.

On macOS, `GptOssRuntimeBackend::Auto` still prefers:

- `Metal`
- then `Cpu`

That means the local user-facing model lane is still fundamentally shaped around Psionic GPT-OSS, not Apple FM.

If the new MVP policy is:

- Mac => Apple Foundation Models
- NVIDIA/Linux => Psionic GPT-OSS CUDA

then the current default runtime selection is no longer aligned with product policy.

### 2. The Apple FM bridge is provider-facing first, not user-facing first

Today Apple FM is wired into:

- provider runtime
- NIP-90 capability selection
- kernel inventory
- provider job execution

It is not wired into the same app-owned local inference seam that powers the general local model lane.

That means Apple FM is closer to "backend provider execution adapter" than "first-class local desktop model runtime".

For the MVP direction, that should be inverted on Mac.

### 3. There is no true Apple FM workbench/chat integration yet

I did not find Apple FM used as the default backend for:

- the local inference runtime seam
- the local GPT-OSS workbench path
- the general chat/local-assistant lane

The existing local runtime/workbench path is still Psionic GPT-OSS-backed.

So even though Apple FM is visible in provider status UI, it is not yet the user-facing local-model experience on Mac.

### 4. The current bridge shares one `LanguageModelSession` across requests

`ChatHandler` stores a single actor-owned `session: LanguageModelSession?` and creates it once.

That has two implications:

- request isolation is unclear
- state can bleed across what the desktop may believe are independent requests

For provider jobs and a local workbench, that is a risky default. The MVP local-model lane needs explicit semantics:

- per-request isolated session
- or explicitly managed conversational session with reset/continuation controls

Right now it is neither explicit nor surfaced through the Rust seam.

### 5. The bridge request contract is much thinner than the app will need

The Rust bridge caller in `apple_fm_bridge.rs` currently always sends:

- one `user` message
- `temperature: None`
- `max_tokens: Some(1024)`
- `stream: false`

And the provider Apple FM path only passes:

- `request_id`
- `prompt`
- `requested_model`

It does not pass execution params like:

- `temperature`
- `top_k`
- `top_p`
- penalties
- `stop`

So compared to the Psionic local runtime seam, Apple FM currently ignores most generation controls.

That is acceptable for a first audit, but not for a finished local-model MVP lane.

### 6. Token accounting is approximate, not truthful

`ChatHandler.swift` sets:

- `promptTokens = prompt.count / 4`
- `completionTokens = content.count / 4`

Those are heuristic character counts, not real token counts from the Foundation Models runtime.

That means:

- perf displays using token counts are only approximate
- provider provenance fields are not truthful token receipts
- capability-envelope throughput derived from those counts is directionally useful at best

For the MVP local user experience this may be tolerable short-term, but for any performance claims, receipts, or pricing logic it is not good enough.

### 7. The Swift HTTP server is intentionally naive

`Server.swift` does raw request parsing and only calls `connection.receive(...)` once per connection.

That is fine for a tiny localhost sidecar under light load, but it is not robust HTTP infrastructure. Risks include:

- partial request bodies
- larger prompt truncation edge cases
- no streaming path
- no chunked or SSE support

For MVP this may still be acceptable if prompts stay modest, but it is a real technical limit.

### 8. Bridge stdout/stderr are suppressed by Rust

When Rust launches the sidecar, it sets:

- `stdin` to null
- `stdout` to null
- `stderr` to null

That keeps the desktop quiet, but it also means bridge diagnostics are hidden unless the worker surfaces them through HTTP failures.

For a shipping Mac local-model lane, that is a supportability weakness.

### 9. There is still naming drift from older local-inference assumptions

A lot of desktop code still uses legacy labels like:

- `ollama_execution`
- `local inference`
- preferred backend string `"psionic" | "local_inference" | "ollama"`

while the retained local runtime is now Psionic GPT-OSS.

Apple FM then sits beside that under its own separate naming.

This is not just cosmetic. It makes policy harder to reason about:

- what is "local inference" on Mac now?
- what is the non-Apple Mac fallback?
- what does "ollama" mean in a world where the runtime is Psionic GPT-OSS?

Before Apple FM becomes the Mac MVP lane, this naming and backend model should be cleaned up.

### 10. Current product policy is preference, not enforcement

The provider substrate currently prefers Apple FM when it is ready, but it does not enforce:

- `Mac must use Apple FM only`

If Psionic local inference is also ready on macOS, the product model still has both lanes available.

That was reasonable before the current pivot. It is not aligned with the new direction.

## What This Means For The New MVP Direction

The new MVP direction is viable without resurrecting Metal GPT-OSS.

The repo already has the right foundation for:

- `macOS -> Apple FM via Swift bridge`
- `NVIDIA/Linux -> Psionic GPT-OSS CUDA`

But the missing work is integration work, not model math work.

The important conclusion is:

- we do not need to invent the Apple FM lane from zero
- we do need to move it from "provider-side sidecar integration" to "first-class local runtime/backend on Mac"

## Recommended Path Forward

### Phase 1. Make Apple FM a real implementation of the app-owned runtime seam

Add an Apple FM-backed implementation behind `LocalInferenceRuntime`, instead of keeping it as a separate provider-only worker.

Desired result:

- the app-owned local runtime seam can be backed by either:
  - Apple FM bridge
  - Psionic GPT-OSS

### Phase 2. Make backend selection explicit by platform/policy

The desktop should stop auto-choosing Psionic Metal GPT-OSS on macOS.

The desired MVP policy is:

- macOS default local runtime => Apple FM
- Linux/NVIDIA default local runtime => Psionic CUDA GPT-OSS
- macOS Psionic Metal GPT-OSS => non-shipping or hidden/developer-only

### Phase 3. Reuse the same backend in the user-facing local pane/workbench

The local model workbench/chat lane should consume the same backend selection that provider execution uses, rather than having Apple FM on one side and Psionic GPT-OSS on the other.

For Mac, that means the user-facing local-model surface should actually talk to Apple FM.

### Phase 4. Fix request/session truth

Before calling the Apple FM lane "done", the bridge should gain explicit behavior for:

- per-request session isolation or explicit reset/continuation
- request parameter mapping
- better metrics/token accounting
- surfaced diagnostics

### Phase 5. Productize build/distribution

The bridge currently builds and is discoverable, but packaging should be first-class:

- deterministic build inclusion in desktop distribution
- known binary location at runtime
- versioning/supervision expectations
- release validation on supported Apple hardware

## Specific Missing Work I Would Track Next

1. Make `AppleFmBridgeWorker` or a sibling adapter implement the same runtime contract as `LocalInferenceRuntime`.
2. Change `default_local_inference_runtime()` so macOS does not default to Psionic Metal GPT-OSS.
3. Add an explicit platform policy layer: `macOS => Apple FM`, `NVIDIA/Linux => Psionic CUDA`.
4. Route the local inference pane/workbench to Apple FM on Mac.
5. Decide and implement request/session semantics for Apple FM:
   - isolated request
   - conversational session
   - resettable session
6. Pass generation controls through when the Apple API and bridge contract can support them truthfully.
7. Replace approximate token metrics with a more defensible metric story, or mark them as estimated in receipts/UI.
8. Improve bridge diagnostics and error surfacing.
9. Clean up backend naming drift so "Ollama", "Psionic", and "Apple FM" are not conflated.

## Final Assessment

The Apple Foundation Models bridge is already integrated enough to matter, but not yet integrated enough to be the shipped Mac local-model lane.

The good news is that the hard part for this pivot is not building a new Apple bridge. That part already exists:

- Swift bridge exists
- Rust supervision exists
- provider/runtime wiring exists
- UI/provider truth exists
- tests exist

The real missing work is unification:

- unify Apple FM with the app-owned local runtime seam
- make platform policy explicit
- make Mac actually use Apple FM in the user-facing local model experience

So the state today is best described as:

- Apple FM integration: real, partial, worth building on
- Mac-local MVP readiness: not complete
- Metal GPT-OSS dependency for Mac MVP: no longer necessary
