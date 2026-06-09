# Apple FM Previous Implementations And First Probe Backend Audit

Date: 2026-06-07

Status: audit and implementation plan for making Apple Foundation Models the
first supported Probe backend in the new Bun/Effect runtime.

## Intended End State

Probe should support Apple Foundation Models as its first real backend by
attaching to a local Apple FM bridge, checking live availability, running
plain-text inference, emitting honest usage and availability receipts, and
only then adding session-backed tool callbacks.

This should be a first-party Probe backend implemented in the new Bun/Effect
runtime. Do not restore the old Rust tree as a compatibility layer. Harvest the
old contracts, tests, runbooks, and failure posture, then implement the final
product surface directly.

## Why Apple FM First

Apple FM is the cleanest first backend for the new Probe runtime because it
does not require cloud account linking, paid API keys, or ChatGPT grant
materialization. It proves the backend abstraction, local attach flow,
availability truth, receipts, and Pylon/SHC host capability path before the
runtime takes on harder hosted-account complexity.

The product thesis is also already clear in the OpenAgents transcripts:

- Episode 201 frames Apple Silicon and Apple Foundation Models as stranded
  local compute that can become routable market supply when paired with
  routing, receipts, settlement, and Pylon provider mode.
- Episode 218 frames Probe as the coding-agent runtime that should use local
  and remote model routes rather than treating Codex as the only path.
- Episode 219 shows the early Probe demo with three inference modes: Codex,
  remote Psionic Qwen over Tailnet, and local Apple FM. The explicit lesson was
  that Apple FM is not the strongest model, but it is useful for some local
  work and can offload part of a larger coding-agent workload.

That is the right first-backend posture: Apple FM is not the first complete
replacement for Codex. It is the first local backend that lets Probe prove
backend routing, local receipts, setup truth, and tool-call boundaries.

## Transcript Findings

### Episode 201: Fracking Apple Silicon

Episode 201 argues that Apple Foundation Models plus MLX could move a
meaningful fraction of inference from cloud to Apple Silicon. The relevant
Probe/Pylon points are:

- Apple Foundation Models were described as strong app-dev primitives with
  tool-call potential.
- A local M2 was already being used to run the model and stream control over
  WebSockets to a mobile device.
- The expected early use was not full coding replacement. It was local
  orchestration, titles, summaries, and other smaller work units that reduce
  cloud dependence.
- The Pylon compute-market thesis starts with Apple Silicon because the model
  is built into the computer, reducing provider setup friction.
- Compute-market language matters: routing, reputation, receipts, and
  streaming money turn idle local devices into usable supply.

For Probe, this means Apple FM should be modeled as a local capability surface
that can later be advertised by Pylons and SHC boxes. It should not be hidden
as an implementation detail behind a generic "local model" label.

### Episode 218: Probe

Episode 218 introduces Probe as a first-party coding-agent runtime. The old
direction was Rust-first, OpenCode-inspired, and intended to be embedded into
the Autopilot/OpenAgents product suite.

The relevant enduring idea is not the Rust implementation choice. The enduring
idea is that Probe should own the coding-agent loop and be able to run through
several inference routes. The new implementation has moved to Bun/Effect, but
the product contract still holds:

- `probe` is the product/runtime name.
- Probe owns sessions, tools, approvals, and artifacts.
- Backends provide inference, not runtime authority.
- The runtime should be embeddable in OpenAgents surfaces.

### Episode 219: Probe Inference Modes

Episode 219 shows the concrete early inference-mode split:

- Codex: best coding-agent lane, using ChatGPT accounts.
- Psionic Qwen: fast local/remote open model, shown over Tailnet to an NVIDIA
  desktop.
- Apple FM: local Apple Foundation Model.

The key observation was that Apple FM was weaker but useful. The plan was to
combine these routes so some of the many coding-agent model calls can be
offloaded to local models. That becomes the correct Probe policy:

- Apple FM should be first supported local backend.
- Apple FM should handle bounded local subwork first.
- Probe should avoid claiming Apple FM is a complete coding backend until the
  retained acceptance cases prove it.

## Prior Probe Apple FM Implementation

The archived Probe implementation at commit `2d82d44` had a complete Apple FM
lane. It should be treated as source material, not resurrected wholesale.

### Backend Profiles

Old Probe modeled Apple FM with its own backend kind:

- backend kind: `AppleFmBridge`
- profile: `psionic-apple-fm-bridge`
- oracle profile: `psionic-apple-fm-oracle`
- model id: `apple-foundation-model`
- default bridge URL: `http://127.0.0.1:11435`
- override order:
  - `PROBE_APPLE_FM_BASE_URL`
  - `OPENAGENTS_APPLE_FM_BASE_URL`

This part should mostly survive. In the new Probe runtime, the profile should
be represented with Effect schemas and a typed backend registry. The naming can
be simplified, but the externally useful facts should stay stable:

- `apple_fm_bridge` as the backend family
- `apple-foundation-model` as the default model id
- loopback bridge attach by default
- `PROBE_APPLE_FM_BASE_URL` before `OPENAGENTS_APPLE_FM_BASE_URL`

### Attach Boundary

Old Probe was explicit that Apple FM was attach-only. It did not try to
supervise managed launch in the same path as OpenAI-compatible backends.

The attach checks differed by backend:

- OpenAI-compatible Psionic/Qwen checked `/v1/models`
- Apple FM checked `/health`

If Apple FM was unavailable, Probe preserved typed availability truth instead
of flattening the condition into generic HTTP failure.

That is the correct first backend boundary for the new runtime:

- do not auto-launch the bridge in the first slice
- attach to `GET /health`
- surface typed unavailability reasons
- only run inference after readiness is true
- keep managed bridge launch as a later platform-specific packaging feature

### Plain-Text Provider Lane

Old Probe had a `probe-provider-apple-fm` crate with:

- provider config from backend profile
- message conversion from Probe plain-text roles to Apple FM roles
- blocking plain-text completion against `/v1/chat/completions`
- typed provider errors wrapping Foundation Models error payloads
- usage handling through Apple FM usage structures

That first lane supported:

- `probe exec`
- `probe chat`
- bounded `consult_oracle`
- bounded repository-analysis helper

For the new runtime, this maps cleanly to a first implementation slice:

- `packages/runtime/src/backends/apple-fm/contract.ts`
- `packages/runtime/src/backends/apple-fm/client.ts`
- `packages/runtime/src/backends/registry.ts`
- tests with a fake Apple FM bridge
- `probe apple-fm status`
- `probe apple-fm smoke`

### Tool Lane

Old Probe later added session-backed Apple FM coding turns without moving the
controller loop into Psionic or the bridge. Probe still owned:

- tool registry
- approvals
- transcript
- resume replay
- refusal and pause receipts
- bounded callback count

Apple FM received projected tool definitions for:

- `read_file`
- `list_files`
- `code_search`
- `shell`
- `apply_patch`
- `consult_oracle`
- `analyze_repository`

The bridge called back into a Probe-owned local callback URL for tool use.
Probe executed or denied tools under its normal policy, then returned
structured tool outputs to Apple FM.

Important compatibility details:

- root tool schemas were normalized before registration
- transcript restore retried once without transcript when Apple FM rejected a
  replay payload with the narrow invalid-JSON failure
- old backend session ids were not treated as Probe controller truth
- Probe rebuilt Apple session state from its own transcript before each turn
- `max_model_round_trips` bounded Apple FM callback count in a turn

The new runtime should not implement this first. It should be the second Apple
FM milestone after plain-text attach and receipts are green.

### Streaming Lane

Old Probe correctly refused to fake OpenAI-style token deltas for Apple FM.
Apple FM streamed full response snapshots through the session-first bridge
contract.

Old runtime events included:

- assistant stream started
- time to first token observed
- assistant snapshot
- assistant stream finished
- normal local tool lifecycle events
- final assistant commit

For new Probe, keep the same semantic distinction:

- OpenAI-compatible backends may stream deltas.
- Apple FM streams snapshots.
- The UI should replace the active Apple FM snapshot in place.
- The final Probe transcript row is authoritative only after terminal commit.

### Usage And Backend Receipts

Old Probe widened usage accounting because Apple FM usage could be estimated.
The retained lesson is important:

- token counts should not be assumed exact
- prompt/completion/total usage should carry `truth = exact | estimated`
- Apple FM typed availability and refusal facts should become backend receipts
- Apple-native transcript exports are adjunct evidence, not Probe transcript
  authority

This should be added early in the new backend contract. Do not let the first
Apple FM implementation emit fake exact token counts.

### TUI Setup Flow

Old Probe had an Apple FM setup screen that checked availability before any
inference calls. It surfaced:

- setup status
- backend facts
- availability detail
- timeline events
- typed unavailable states

The scope was deliberately narrow:

- plain-text Apple FM only
- no tool-backed coding in the setup screen
- no managed launch

The new Probe CLI should start with the same narrow shape:

- status command
- smoke command
- explicit availability truth
- no pretend success on non-admitted machines

If a TUI returns later, setup belongs in a backend overlay or setup tab, not as
global startup work unless Apple FM is the selected backend.

### Acceptance And Comparison

Old Probe had retained Apple FM versus Qwen comparison coverage for six
overlapping coding-bootstrap cases:

- `read_file_answer`
- `list_then_read`
- `search_then_read`
- `shell_then_summarize`
- `patch_then_verify`
- `approval_pause_or_refusal`

The comparison suite was explicit:

- admitted-Mac only
- not default CI
- unsupported is different from failure
- comparison receipts preserve backend-specific facts
- no global capability parity claim

New Probe should keep this posture. Live Apple FM tests should be local
admitted-hardware runbook tests, while CI uses fake bridge tests.

## Prior Psionic And Desktop Apple FM Work

The Apple FM work did not start in Probe. The earlier Psionic/OpenAgents
desktop lane matters because it explains the bridge shape and some mistakes to
avoid.

### Swift Bridge Audit

The Psionic audit from 2026-03-10 found a real Swift bridge in the historical
desktop app. It exposed:

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions`

Rust supervised the sidecar, polled health, started/stopped it, and issued
completions. Provider-mode plumbing already preferred Apple FM when ready, and
UI surfaces showed readiness/blockers.

The main limitations were:

- Apple FM was not the default local inference runtime.
- The bridge was provider-facing first, not user-facing first.
- The bridge shared one `LanguageModelSession` across requests.
- request generation parameters were mostly ignored.
- token counts were approximate.
- the HTTP server was intentionally naive.
- sidecar diagnostics were suppressed.
- local-inference naming drift made platform policy harder to reason about.

For Probe, these are migration constraints:

- define request/session semantics before relying on multi-turn behavior
- mark usage as estimated unless the bridge proves otherwise
- expose diagnostics through typed status, not only process logs
- keep platform policy explicit
- do not call Apple FM "local inference" in a way that hides what backend ran

### `psionic-apple-fm`

The current Psionic `psionic-apple-fm` crate is a reusable Apple FM contract
and client layer. It exposes typed constants and structures for:

- default model id
- health
- shutdown
- models
- sessions
- adapters
- chat completions
- streaming suffix
- transcript export suffix
- structured response suffix
- session adapter suffix
- system model use cases and guardrails
- typed unavailable reasons

This is the strongest source material for the new Probe backend contract.
Probe can either depend on the bridge HTTP contract semantically or, if we
later reintroduce Rust components, reuse the crate directly. For the current
Bun/Effect runtime, the right move is to port the contract into Effect schemas
and keep it small.

### Plugin Session Pilot

The Tassadar Apple FM plugin-session pilot proved a useful controller pattern:

- Apple FM receives projected tool definitions.
- A session-local callback runtime executes tools.
- success and refusal cases are both represented.
- transcript truth stays explicit.
- committed bundles avoid recording callback URL or raw session token.

This maps directly to Probe's later tool lane. The first tool-backed Apple FM
implementation should preserve session token binding without logging the raw
callback secret.

### Apple Adapter Training Is Separate

Psionic also has Apple adapter training/export work. That should not be
confused with Probe's first Apple FM backend.

Probe's first backend is runtime inference against the system Foundation Model
through a local bridge. It is not:

- Apple adapter training
- benchmark-useful adapter claims
- `.fmadapter` export authority
- MLX training

Adapter loading or attachment can become a later Apple FM backend capability
only after the plain system-model backend is stable.

## What To Keep

Keep these from the old implementations:

- dedicated Apple FM backend kind, not a generic local-model alias
- default model id `apple-foundation-model`
- default URL `http://127.0.0.1:11435`
- env override order: `PROBE_APPLE_FM_BASE_URL`, then
  `OPENAGENTS_APPLE_FM_BASE_URL`
- attach-only first milestone
- `GET /health` as the readiness gate
- typed availability reasons
- exact-versus-estimated usage truth
- backend receipts for typed failure, availability, and transcript evidence
- snapshot streaming semantics
- Probe-owned transcript authority
- Probe-owned tool execution and approval policy
- callback session tokens that are never logged as public artifacts
- admitted-Mac live test runbook separate from CI
- unsupported as a first-class comparison result

## What To Delete Or Avoid

Do not carry these forward:

- old Rust workspace layout
- old TUI as the immediate product surface
- automatic Apple FM setup work when Apple FM is not selected
- global backend hotkey cycling as primary UX
- pretending Apple FM exposes OpenAI-style token deltas
- treating backend session ids as Probe session truth
- local Apple FM bridge launch as a first milestone
- approximate token counts labeled as exact
- generic "local inference" labels that hide whether Apple FM, Qwen, MLX, or
  another backend actually ran
- claims that Apple FM can handle full coding-agent parity before retained
  cases prove it

## Proposed New Probe Backend Architecture

The new Probe runtime is already a Bun workspace with Effect v4. Apple FM
should fit that shape.

### Runtime Modules

Add:

- `packages/runtime/src/backends/backend-profile.ts`
- `packages/runtime/src/backends/registry.ts`
- `packages/runtime/src/backends/apple-fm/contract.ts`
- `packages/runtime/src/backends/apple-fm/client.ts`
- `packages/runtime/src/backends/apple-fm/receipts.ts`
- `packages/runtime/src/backends/apple-fm/fake-server.test.ts`

The contract module should define Effect schemas for:

- backend profile
- health response
- system model availability
- unavailable reasons
- chat messages
- chat completion request/response
- usage measurement with exact/estimated truth
- backend failure receipt
- backend availability receipt
- backend transcript receipt summary
- stream snapshot event

### Backend Registry

Start with one profile:

- profile id: `apple-fm-local`
- backend kind: `apple_fm_bridge`
- default base URL: `http://127.0.0.1:11435`
- model: `apple-foundation-model`
- attach mode: `attach_existing`
- auth: none
- readiness path: `/health`
- stream mode: `snapshot`

The registry should resolve base URL in this order:

1. explicit assignment/profile override
2. `PROBE_APPLE_FM_BASE_URL`
3. `OPENAGENTS_APPLE_FM_BASE_URL`
4. `http://127.0.0.1:11435`

### Client

The first client should implement:

- `health()`
- `requireReady()`
- `completePlainText(messages)`
- `smoke(prompt)`

It should not implement tool callbacks in the first slice.

### CLI

Add commands:

- `probe apple-fm status`
- `probe apple-fm smoke`

Status should print:

- base URL
- model id
- ready/unavailable
- typed unavailable reason
- availability message
- platform/version if the bridge returns it

Smoke should:

- call status first
- refuse to run if not ready
- run one short plain-text prompt
- print assistant text
- print usage truth as exact/estimated/unknown
- print redacted backend receipt summary

### Assignment Integration

Probe assignments should be able to request:

```json
{
  "backend": {
    "kind": "apple_fm_bridge",
    "profile": "apple-fm-local"
  }
}
```

Apple FM assignments do not need ChatGPT account refs or OpenAgents product surface provider auth
grants. They do need runner capability:

- runner kind: `local`, `shc`, `pylon`, or `sandbox`
- capability: `probe.backend.apple_fm_bridge`
- optional base URL override from trusted local config

For Pylons and SHC boxes, the Apple FM backend should only advertise as
available when live health says the model is ready.

## First Backend Milestones

### Milestone 1: Contract And Fake Bridge

- Add Effect schemas for Apple FM health, chat, usage, and receipts.
- Add fake Apple FM bridge tests.
- Add backend profile resolution tests for env override order.
- Keep this in CI.

### Milestone 2: Attach And Status

- Implement `health()` and `requireReady()`.
- Add `probe apple-fm status`.
- Preserve typed unavailable reasons.
- Refuse smoke/inference on non-ready machines.

### Milestone 3: Plain Text

- Implement `completePlainText`.
- Add `probe apple-fm smoke`.
- Emit estimated/unknown usage truth correctly.
- Add backend receipt summaries for typed failures.

### Milestone 4: Runtime Assignment

- Let a Probe assignment select `apple_fm_bridge`.
- Require runner capability before using the backend.
- Add no-auth backend materialization path: Apple FM needs attach config, not
  provider account auth.
- Emit backend start/finish/failure events.

### Milestone 5: Snapshot Streaming

- Add session-backed snapshot streaming.
- Emit `assistant_snapshot` events rather than fake token deltas.
- Ensure final transcript commit replaces transient snapshot state.

### Milestone 6: Tool Callback Lane

- Project Probe tools into Apple FM session tools.
- Run a loopback callback server with an unlogged session token.
- Enforce Probe approvals and max callback count.
- Persist refused and approval-pending tool results as Probe transcript truth.
- Rebuild Apple transcript snapshots from Probe transcript state on resume.

### Milestone 7: Admitted-Mac Acceptance

- Recreate the six retained overlapping cases from the old comparison suite.
- Keep fake bridge tests in CI.
- Keep live Apple FM acceptance as a local admitted-Mac runbook.
- Preserve explicit unsupported posture.

## Pylon And SHC Plan

Apple FM should become the easiest first Pylon provider backend.

The provider advertises:

- backend kind `apple_fm_bridge`
- base URL health status
- typed availability
- Apple Silicon / Apple Intelligence requirement facts when known
- current readiness
- snapshot stream support
- tool callback support when implemented

The provider does not advertise:

- raw local paths
- callback secrets
- Apple transcript payloads by default
- unsupported parity claims

OpenAgents product surface can assign small local tasks to this backend once Pylon/SHC reports live
health. Payment and receipts can later make this a compute-market route, but
the first Probe milestone only needs the runtime capability contract.

## Risks

- Availability depends on admitted Apple hardware and Apple Intelligence
  readiness.
- The bridge may return approximate usage only.
- Apple FM may refuse or fail tool calls in backend-specific ways.
- Snapshot streaming differs from OpenAI-style deltas and must be rendered
  honestly.
- A naive sidecar HTTP implementation can be acceptable for local MVP but
  should not be overclaimed as robust serving infrastructure.
- Apple FM is useful but weaker than hosted Codex for many coding tasks.

## Immediate Recommendation

Make Apple FM the first supported backend in the new Probe runtime, but scope
it narrowly:

1. backend registry plus Effect schemas
2. fake bridge tests
3. attach health/status
4. plain-text smoke
5. assignment-selected local backend

Only after that should Probe add Apple FM tool callbacks and snapshot
streaming. This preserves the best parts of the previous work while avoiding
the old repo's accidental complexity.

## Source Material Reviewed

Transcripts:

- `openagents/docs/transcripts/201.md`
- `openagents/docs/transcripts/218.md`
- `openagents/docs/transcripts/219.md`

Archived Probe at commit `2d82d44`:

- `docs/24-apple-fm-backend-lane.md`
- `docs/25-apple-fm-tool-lane.md`
- `docs/26-backend-receipts-and-usage-truth.md`
- `docs/27-apple-fm-qwen-comparison-suite.md`
- `docs/28-admitted-mac-comparison-runbook.md`
- `docs/32-apple-fm-setup-screen.md`
- `docs/48-apple-fm-streaming-and-snapshot-events.md`
- `crates/probe-core/src/backend_profiles.rs`
- `crates/probe-core/src/provider.rs`
- `crates/probe-provider-apple-fm/src/lib.rs`
- `crates/probe-provider-apple-fm/tests/provider_suite.rs`
- `README.md`

Psionic/OpenAgents Apple FM source material:

- `psionic/docs/audits/2026-03-10-apple-fm-swift-bridge-audit.md`
- `psionic/docs/audits/2026-03-16-psionic-apple-acceptance-harness-status.md`
- `psionic/docs/audits/2026-03-22-tassadar-post-article-apple-fm-plugin-session.md`
- `psionic/crates/psionic-apple-fm/Cargo.toml`
- `psionic/crates/psionic-apple-fm/src/contract.rs`
- `psionic/crates/psionic-apple-fm/src/client.rs`
- `psionic/crates/psionic-apple-fm/src/tassadar_post_article_starter_plugin_tools.rs`
