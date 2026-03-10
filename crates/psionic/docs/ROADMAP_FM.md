# Psionic Apple Foundation Models Roadmap

> Status: updated 2026-03-10 after re-reading `docs/MVP.md` and
> `docs/OWNERSHIP.md`, after re-reading the retained Apple FM audit in
> `docs/audits/2026-03-10-apple-fm-swift-bridge-audit.md`, and after
> inspecting `~/code/python-apple-fm-sdk` across its exported API, user docs,
> and test suite, and after re-reading the current Mission Control pane and its
> plan in `apps/autopilot-desktop/src/pane_renderer.rs`,
> `apps/autopilot-desktop/src/input/actions.rs`,
> `apps/autopilot-desktop/src/app_state.rs`, and
> `docs/plans/mission-control-pane.md`.
>
> This is the live roadmap for the Apple Foundation Models lane. For the MVP,
> `Mac = Apple Foundation Models via our Swift bridge`, `NVIDIA = Psionic
> GPT-OSS CUDA`, and native Metal GPT-OSS is not the Mac shipping path.

Agent execution instruction: implement this roadmap one item at a time in the
recommended dependency order below. After each completed FM roadmap item,
update this document so it records what landed on `main`, which API surface is
now covered, what remains open, and what the current execution queue is. Do
not treat "we can hit `/v1/chat/completions`" as closure for this lane. The
goal here is a real Rust/Psionic Apple FM SDK and runtime surface with full
semantic coverage of the Apple Foundation Models API as represented by the
Python SDK.

Reference-first instruction: Apple FM work must not be implemented from memory.
Choose the reference that owns the layer being changed:

- start with `~/code/python-apple-fm-sdk` for the public semantic surface,
  exported symbols, documented behavior, and conformance expectations
- start with `docs/audits/2026-03-10-apple-fm-swift-bridge-audit.md` for the
  current retained repo truth and the known integration gaps
- start with the current Mission Control implementation in
  `apps/autopilot-desktop/src/pane_renderer.rs`,
  `apps/autopilot-desktop/src/input/actions.rs`,
  `apps/autopilot-desktop/src/app_state.rs`, and
  `docs/plans/mission-control-pane.md` when the work touches the earn-first
  shell on macOS
- start with `swift/foundation-bridge/` and
  `apps/autopilot-desktop/src/apple_fm_bridge.rs` for the currently shipped
  bridge contract and desktop supervision model

Retention rule: do not pull archived Backroom code into the active repo by
default. If an archived implementation later looks useful, treat it as a
reference only unless there is an explicit instruction to restore code.

## Objective

Build a Psionic-owned Apple Foundation Models lane that is truthful enough to
ship as the macOS local-model story:

- `crates/psionic/*` provides the reusable Rust API and runtime truth for Apple
  FM
- our Swift bridge remains the direct execution substrate for Apple's
  Foundation Models framework
- `apps/autopilot-desktop` consumes that reusable Psionic truth for chat,
  workbench, and provider flows
- the Rust surface reaches full semantic coverage of the Apple FM API exposed
  by `python-apple-fm-sdk`
- on macOS, Mission Control and the user-facing local-model surfaces show Apple
  FM truth instead of GPT-OSS-specific loading language
- the desktop stops treating Apple FM as a provider-only sidecar and instead
  uses it as the default Mac local inference lane when it is available

Coverage goal: "100% coverage" is semantic coverage, not Python syntax
imitation. The Rust API does not need to look like Python, but every public
capability in the Python SDK should be either:

- implemented in the Psionic Apple FM surface
- explicitly mapped to a Rust-native equivalent with the same behavior
- or documented as truly not applicable in Rust, with that exception kept rare

## Non-Goals

This roadmap is not:

- a plan to ship native Metal GPT-OSS as the Mac MVP local-model path
- a plan to keep Apple FM as only a thin OpenAI-compatible
  `/v1/chat/completions` sidecar
- a plan to move app-specific process supervision or pane behavior into
  `crates/psionic/*`
- a promise that MVP requires direct pure-Rust invocation of the Apple
  framework without Swift; the near-term execution substrate can and should
  stay our Swift bridge

## Ownership Rules

This roadmap must continue to respect `docs/OWNERSHIP.md`:

- `crates/psionic/*` owns reusable Apple FM API contracts, typed client
  surface, model/session semantics, transcript/schema/tool types, streaming
  semantics, option validation, error taxonomy, and conformance tests
- `swift/foundation-bridge/` owns the direct calls into Apple's Foundation
  Models framework and the typed bridge transport surface needed to expose that
  functionality
- `apps/autopilot-desktop` owns bridge supervision, bundling, default-runtime
  choice on macOS, provider UX, workbench/chat integration, and other
  app-specific orchestration
- `crates/openagents-provider-substrate` owns reusable provider product and
  backend identity truth, not Apple FM session/runtime semantics

## Why This Roadmap Exists

The retained repo already has a real Apple FM starting point, but it is much
smaller than the actual API surface Apple now exposes through the Python SDK.

What the retained tree has today:

- an in-tree Swift bridge in `swift/foundation-bridge/`
- app-owned Rust supervision in
  `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- provider/backend readiness truth and Apple FM inventory plumbing in the
  desktop
- a useful audit of the current state in
  `docs/audits/2026-03-10-apple-fm-swift-bridge-audit.md`

What the retained tree does not have today:

- a reusable Psionic Apple FM crate or Rust SDK surface
- session handles or transcript restore semantics
- streaming
- structured generation
- tools
- typed error mapping
- truthful generation options coverage
- Apple FM as the default Mac local inference lane
- a Mission Control pane that speaks Apple FM truth on macOS instead of
  hard-coded GPT-OSS 20B load semantics

The Python SDK makes the gap unmistakable: Apple FM is not just "one endpoint
that returns a string." It is a full sessioned API with streaming, transcript
export/import, schema-guided generation, tool calling, typed availability and
guardrail configuration, and a real error taxonomy.

## Current Retained Baseline

`main` already includes the following retained Apple FM baseline:

- `swift/foundation-bridge/` exposes:
  - `GET /health`
  - `GET /v1/models`
  - `POST /v1/chat/completions`
- `apps/autopilot-desktop/src/apple_fm_bridge.rs` supervises that bridge as a
  localhost sidecar
- desktop provider/runtime plumbing can already route eligible text-generation
  work through Apple FM when the bridge is healthy
- the Apple FM status is already surfaced in provider state and desktop UI
- the audit at `docs/audits/2026-03-10-apple-fm-swift-bridge-audit.md`
  documents the real shipped gaps

That is a legitimate base to build on, but it is still only an initial bridge,
not a Psionic-owned SDK or a complete Apple FM runtime lane.

## Shipped On Main

`FM-1`, `FM-2`, `FM-3`, `FM-4`, and `FM-5` are now landed on `main`.

What shipped:

- `crates/psionic/psionic-apple-fm` exists as the reusable Psionic crate for
  the currently retained Apple FM bridge contract and client
- the current bridge request/response models moved out of
  `apps/autopilot-desktop` and into reusable Psionic-owned contract types
- `psionic-apple-fm::AppleFmBridgeClient` now owns the reusable blocking
  bridge client for `/health`, `/v1/models`, and `/v1/chat/completions`
- `apps/autopilot-desktop/src/apple_fm_bridge.rs` now consumes the shared
  Psionic Apple FM client instead of owning duplicate ad hoc JSON transport
  shapes
- `crates/psionic/docs/FM_API_COVERAGE_MATRIX.md` exists as the living
  conformance matrix mapping the exported Python SDK surface and behavior
  families to the Rust/Psionic roadmap
- `psionic-apple-fm` now exposes typed `SystemLanguageModel`-equivalent Rust
  model/configuration state via `AppleFmSystemLanguageModel`,
  `AppleFmSystemLanguageModelUseCase`,
  `AppleFmSystemLanguageModelGuardrails`, and
  `AppleFmSystemLanguageModelUnavailableReason`
- bridge `/health` and `/v1/models` responses now carry typed Apple FM
  availability/use-case/guardrail truth, and the reusable client reconstructs
  that truth as `AppleFmSystemLanguageModelAvailability`
- `apps/autopilot-desktop` Apple FM runtime state now carries typed system-model
  readiness/configuration fields instead of collapsing everything into a single
  availability string
- the bridge now owns explicit Apple FM session IDs, per-session model/instruction
  binding, reset/get/delete/respond endpoints, and transcript-backed restore from
  raw transcript JSON
- the reusable Rust client now owns session lifecycle APIs instead of relying on
  one hidden shared Swift `LanguageModelSession`
- the old one-shot `/v1/chat/completions` path no longer depends on a hidden
  long-lived shared session; it now uses an ephemeral session per request
- `psionic-apple-fm` now exposes first-class Rust `AppleFmGenerationOptions`,
  `AppleFmSamplingMode`, and `AppleFmSamplingModeType` with Python-SDK-matching
  validation for temperature, maximum response tokens, and `top` versus
  `probability_threshold`
- the reusable Rust lane now has first-class plain-text generation request and
  response types instead of only the OpenAI-compatible chat envelope
- session response requests and one-shot chat requests now carry typed Apple FM
  generation options through the bridge
- the Swift bridge now applies those generation options to real
  `LanguageModelSession.respond(..., options: ...)` calls for both one-shot and
  sessioned text generation
- the bridge no longer reports derived token counts as authoritative raw usage;
  usage detail now marks the current counts as `estimated`
- the compatibility chat endpoint now rejects `stream: true` instead of
  silently ignoring it before `FM-5`
- the bridge now exposes a true session streaming endpoint at
  `/v1/sessions/{id}/responses/stream` using SSE snapshot events
- `psionic-apple-fm` now exposes a dedicated async Apple FM bridge client and
  reusable async stream surface for session streaming
- session streaming now yields full response snapshots, not token deltas, and
  terminal completion events include final session state plus usage detail
- the bridge now keeps transcript snapshots stable during in-flight streaming
  and only updates the visible session transcript after successful completion
- early client-side stream cancellation now restores the session promptly so a
  same-session follow-up request can succeed without manual reset

What `FM-1` through `FM-5` did not close:

- structured generation, tools, and typed error taxonomy
- desktop Mission Control cutover on macOS
- typed transcript import/export as a first-class Rust surface remains `FM-6`;
  `FM-3` only landed raw transcript JSON restore as the bridge/session substrate
- raw token counts are now truthfully marked as estimated, but the broader
  typed metrics/error taxonomy work still remains `FM-9`
- the OpenAI-compatible `/v1/chat/completions` path remains a one-shot
  compatibility wrapper; the shipped streaming surface is the session-first
  Apple FM lane, which is the roadmap-authoritative interface

## Mission Control Reality On Main

Mission Control is now an explicit part of this roadmap, because the current
Mac earn-first shell is still coded around GPT-OSS model loading rather than
Apple FM readiness.

Current Mission Control truth on `main`:

- the main pane copy, CTA label, and hints still say `GPT-OSS 20B`
- the `LOAD` action records and renders `Queued GPT-OSS 20B load`
- the log stream emits lines such as `Local GPT-OSS 20B is loading`
- the pane fallback model label is still `GPT-OSS 20B`
- the current Mission Control plan document explicitly requires GPT-OSS 20B to
  be loaded before `GO ONLINE`

This matters because the product rule is now different:

- on macOS, Mission Control should present Apple FM as the local model truth
- on macOS, the earn gate should reflect Apple FM readiness and its blockers,
  not GGUF artifact presence
- the GPT-OSS-specific Mission Control contract should remain only for the
  non-Mac paths that still actually depend on it

So Mission Control is not a side note under desktop polish. It is part of the
Mac Apple FM cutover definition of done.

## Coverage Target From `python-apple-fm-sdk`

The Python SDK defines the reference coverage target for this roadmap.

### Public API surface to cover

The exported Python surface in `src/apple_fm_sdk/__init__.py` includes:

- `SystemLanguageModel`
- `SystemLanguageModelUseCase`
- `SystemLanguageModelGuardrails`
- `SystemLanguageModelUnavailableReason`
- `LanguageModelSession`
- `Transcript`
- `GenerationOptions`
- `SamplingMode`
- `SamplingModeType`
- `GenerationSchema`
- `GeneratedContent`
- `GenerationID`
- `Generable`
- `GenerationGuide` and `guide`
- `generable`
- `Tool`
- the typed Foundation Models error family

The Rust/Psionic Apple FM lane should therefore cover:

- system-model availability and configuration
- explicit use-case and guardrail enums
- session creation with instructions, model selection, and tools
- plain text response generation
- streaming response generation
- guided generation by typed schema
- guided generation by raw JSON schema
- transcript export
- transcript import and session restore
- tool definitions and tool execution flow
- generation-option validation and transport
- typed error mapping

### Behavioral contract to preserve

The Python docs and tests also define non-obvious behavior that matters:

- a session serializes requests and is not a concurrent free-for-all
- cancellation or failure resets session task state without erasing transcript
  history
- `stream_response()` yields full response snapshots, not deltas
- transcripts update after successful completion, not during an in-flight stream
- `from_transcript()` restores history, but tools must still be supplied again
  for new tool calls
- `respond()` supports four real modes:
  - plain text
  - `Generable`
  - explicit `GenerationSchema`
  - raw JSON schema
- `SamplingMode.random()` allows either `top` or
  `probability_threshold`, but not both
- the error taxonomy is part of the contract, not an implementation detail

Those semantics need matching tests in the Psionic Apple FM lane. Do not reduce
this roadmap to endpoint-shape parity while losing session, transcript, or
streaming truth.

## Gap Versus Target

Relative to the Python SDK target, the retained repo is currently missing at
least the following:

- no reusable `crates/psionic/*` Apple FM SDK surface
- no typed bridge protocol beyond minimal model listing and one-shot chat
- no session handles or explicit request-isolation contract
- the current Swift bridge keeps a shared `LanguageModelSession`, which is not
  the right base for a full SDK
- no transcript export/import endpoints
- no structured generation endpoints
- no tool registration or tool-call/result protocol
- no streaming contract
- generation parameters are only partially surfaced and mostly ignored
- usage metrics are approximate and not clearly marked as such
- no typed Rust error taxonomy aligned to the Apple FM model contract
- the desktop still defaults local inference to Psionic GPT-OSS rather than
  Apple FM on macOS
- Mission Control still gates `GO ONLINE`, button copy, log copy, and model
  status around GPT-OSS-specific local-runtime fields instead of Apple FM
  readiness on macOS

## Marching Orders

The Apple FM lane should now be implemented in the following dependency order.
The queue below is the source of truth for "what to do next," not raw issue
number ordering and not whichever missing endpoint looks easiest.

### FM-1: Capture the contract and create the reusable Rust surface

Required outcome:

- a dedicated Psionic Apple FM crate exists under `crates/psionic/*`
- the crate exposes a stable Rust-first API shape for the Apple FM lane
- every exported Python SDK symbol is mapped into a Rust conformance matrix

Deliverables:

- introduce the Psionic Apple FM crate and its public modules
- write a living coverage matrix that maps Python SDK symbols to Rust
  equivalents
- define typed bridge request/response models in reusable Psionic code instead
  of duplicating ad hoc JSON shapes in the desktop app
- keep transport-neutral types separate from app-specific bridge supervision

Acceptance:

- the coverage matrix makes it obvious which Python SDK surfaces are covered,
  pending, or intentionally absent
- the desktop can consume the same typed Apple FM contracts that later tests
  and server/client code use

### FM-2: System model truth and availability/configuration coverage

Required outcome:

- Psionic exposes first-class Apple FM model availability/configuration truth

Deliverables:

- model wrapper equivalent to `SystemLanguageModel`
- Rust enums for:
  - unavailable reason
  - use case
  - guardrails
- bridge support to query model availability and available models truthfully
- tests that cover the enum mapping and availability/error behavior

Acceptance:

- the Rust Apple FM surface can express the same model-level configuration and
  availability reasons the Python SDK exposes
- desktop and provider code stop depending on ad hoc strings for Apple FM
  readiness truth

### FM-3: Session lifecycle, isolation, and reset semantics

Required outcome:

- Apple FM sessions become first-class reusable objects with explicit lifecycle
  behavior

Deliverables:

- session create/destroy support in the bridge
- per-session instructions, model binding, and tool registration
- explicit session IDs or handles
- request serialization semantics for each session
- reset behavior after cancellation/failure
- restore-from-transcript constructor or equivalent

Important rule:

- do not keep the current "one shared Swift `LanguageModelSession` for all
  requests" design and call the lane complete

Acceptance:

- the Rust/Psionic layer can create multiple independent Apple FM sessions
- concurrent work uses multiple sessions, not hidden shared mutable session
  state
- session reset semantics are tested and explicit

### FM-4: Plain text generation and full generation-options coverage

Required outcome:

- the Rust Apple FM lane can perform truthful plain text generation with the
  documented generation options

Deliverables:

- Rust `GenerationOptions`
- Rust `SamplingMode`
- bridge transport for temperature, sampling, and maximum response tokens
- validation that mirrors the Python SDK semantics
- response metrics/usage structure with explicit truth about what is exact
  versus estimated

Acceptance:

- the Rust API can express the documented generation-option space
- invalid option combinations fail locally with typed errors
- request parameters are no longer silently ignored

### FM-5: Streaming with snapshot semantics

Required outcome:

- the Rust Apple FM lane supports streaming response generation with the same
  contract the Python SDK documents

Deliverables:

- bridge streaming transport, likely SSE or another explicitly documented
  stream protocol
- Rust async stream surface
- snapshot semantics: each yielded item is the full response-so-far, not a
  token delta
- cancellation and cleanup behavior

Acceptance:

- streaming is not emulated by polling a completed result
- tests prove snapshot semantics, cancellation, and transcript update timing

### FM-6: Transcript export, import, and session restore

Required outcome:

- transcripts become first-class state, not just a prompt string assembled on
  the fly

Deliverables:

- transcript type in Psionic Apple FM
- transcript export from live sessions
- transcript import from dictionary/JSON form
- session restore from transcript
- explicit rule that historical tool mentions do not enable new tool calls
  unless tools are supplied again

Acceptance:

- the Rust lane can round-trip transcript state
- transcript-backed resume behavior matches the documented Python semantics

### FM-7: Structured generation and schema coverage

Required outcome:

- the Rust Apple FM lane supports structured generation without prompt-hack
  fallbacks

Deliverables:

- `GenerationSchema` equivalent
- `GeneratedContent` equivalent
- typed conversion helpers for Rust-native structs
- guided generation from:
  - typed Rust schema
  - explicit schema object
  - raw JSON schema
- coverage for guide constraints such as enum values, ranges, counts, and
  simple regex guidance where the underlying Apple contract supports them

Acceptance:

- structured generation uses the Apple FM structured-generation path, not "ask
  for JSON in the prompt"
- tests cover nested objects, lists, and validation/error cases similar to the
  Python SDK suite

### FM-8: Tool calling

Required outcome:

- the Rust Apple FM lane can register tools and service tool calls without
  degrading them into prompt tricks

Deliverables:

- reusable Rust tool trait or equivalent
- typed argument-schema registration
- bridge protocol for:
  - tool definitions
  - tool-call requests from the Swift side
  - tool results or tool errors back into the active session
- support for transcripted tool history
- typed `ToolCallError` mapping

Important rule:

- tool support requires a real session-aware callback contract; do not fake it
  by flattening tools into the prompt

Acceptance:

- the Apple FM lane can run real tool-enabled sessions
- tool errors are surfaced explicitly and do not corrupt unrelated session
  state

### FM-9: Error taxonomy, metrics, and evidence truth

Required outcome:

- the Apple FM lane becomes explicit and typed about failure and metrics truth

Deliverables:

- Rust error hierarchy aligned to the Python SDK error family:
  - context window
  - assets unavailable
  - guardrail violation
  - unsupported guide
  - unsupported locale/language
  - decoding failure
  - rate limited
  - concurrent requests
  - refusal
  - invalid schema
  - tool call failure
- bridge error mapping from Swift/Foundation Models into typed Rust errors
- explicit distinction between exact counts and estimated counts in usage and
  receipts
- conformance tests for common failure cases

Acceptance:

- the Rust Apple FM lane stops collapsing everything into generic request
  failure strings
- desktop/provider surfaces can present truthful actionable errors

### FM-10: Desktop cutover, Mission Control, workbench integration, and packaging

Required outcome:

- on macOS, Apple FM becomes the real local-model lane the desktop uses when it
  is available

Deliverables:

- make Apple FM a first-class implementation of the app-owned
  `LocalInferenceRuntime` seam
- make macOS runtime selection prefer Apple FM over Metal/CPU GPT-OSS when
  Apple FM is available
- cut Mission Control over so macOS no longer presents `LOAD GPT-OSS 20B` as
  the primary local-model action
- make Mission Control source its `Model`, `Backend`, `Load`, CTA, and log
  lines from the active backend truth rather than from GPT-OSS-only local
  artifact assumptions
- on macOS, gate Mission Control `GO ONLINE` on Apple FM readiness and Apple FM
  blockers, not on GGUF artifact presence
- add chat/workbench usage through the same reusable Psionic Apple FM surface
- keep provider execution and user-facing local inference on the same runtime
  truth
- preserve packaging rules already called out in the retained audit:
  wrapper-style bridge packaging when entitlements require it, explicit bridge
  discovery, and clear unsupported-platform messaging
- update `docs/plans/mission-control-pane.md` so the product plan matches the
  new Mac rule instead of codifying GPT-OSS 20B as the universal Mission
  Control gate

Acceptance:

- the desktop no longer has a split-brain `provider Apple FM` lane versus
  `user local inference GPT-OSS` lane on macOS
- Apple FM is visible, selectable, and truthful in the same app-owned runtime
  seam as other local runtimes
- on macOS, Mission Control shows Apple FM readiness, Apple FM blocker text,
  and Apple FM log lines instead of GPT-OSS-specific copy
- the Mission Control action button and `GO ONLINE` gate are backend-aware:
  Apple FM on macOS, GPT-OSS only where that is still the actual runtime truth

## Recommended Execution Queue

After `FM-5`, the next-item order is:

1. `FM-6` transcripts and session restore
2. `FM-7` structured generation and schema support
3. `FM-8` tools
4. `FM-9` typed errors and metrics truth
5. `FM-10` desktop cutover, Mission Control cutover, and packaging cleanup

That order is intentional. Tools and structured generation should not be bolted
onto the current minimal one-shot bridge. First build the reusable substrate and
session semantics, then extend the bridge contract, then cut the desktop over.

## Definition Of Done

The Apple FM roadmap is not done when a simple prompt returns text. It is done
when all of the following are true:

- the Psionic Apple FM crate exposes a reusable Rust API that covers the full
  intended Apple FM surface
- each exported Python SDK capability has a covered or explicitly documented
  mapping in the conformance matrix
- bridge transport and Swift execution cover sessions, streaming, transcripts,
  structured generation, and tools
- typed error mapping is in place and desktop surfaces consume it truthfully
- generation options are validated and transported correctly
- transcript and session restore semantics are real, not reconstructed prompt
  hacks
- tool calling is real, session-aware, and transcripted
- on macOS, Apple FM is the default app local-model lane when available
- on macOS, Mission Control no longer tells the user to load GPT-OSS 20B and
  instead reflects Apple FM truth throughout its sell-compute lane and log
  stream
- the provider lane and user-facing local-inference lane use the same Apple FM
  runtime truth
- usage and token counts are explicit about whether they are exact, derived, or
  estimated
- conformance tests exist for every major Python SDK feature family:
  - model availability
  - session lifecycle
  - plain text generation
  - generation options
  - streaming
  - transcripts
  - structured generation
  - tools
  - error handling

## Product Rule For MVP

Until this roadmap is complete enough to support honest desktop cutover, the
product rule remains:

- macOS local on-device story should converge on Apple Foundation Models
- NVIDIA local high-performance story remains Psionic GPT-OSS CUDA
- Metal GPT-OSS work is not the MVP Mac local-model lane

This roadmap exists to make that product rule real in code instead of true only
in planning documents.
