# Apple FM Integration And Pylon Planning Audit

Date: 2026-06-15

Status: comprehensive archaeology pass over current `openagents`, deleted
history in this repo, sibling Psionic/Probe material, Pylon planning docs, and
the requested transcripts `docs/transcripts/194.md` and
`docs/transcripts/201.md`.

## Executive Summary

Apple Foundation Models has been used in three distinct OpenAgents lanes:

1. **Native Swift app/orchestration lane.** Earlier Swift code used
   `FoundationModels` directly for conversation-title summaries, workspace
   exploration, code/session search tools, streaming ACP chat output, and a
   `codex.run` delegation tool.
2. **Swift Foundation bridge lane.** Later repo versions introduced
   `swift/foundation-bridge`, a localhost Swift HTTP sidecar on
   `http://127.0.0.1:11435`. It exposed health, model listing,
   OpenAI-compatible chat, explicit sessions, transcript export/restore,
   structured generation, snapshot streaming, and real tool callbacks into
   Rust/TypeScript controller code.
3. **Probe/Pylon runtime lane.** The current Bun/Effect runtime preserves the
   bridge contract as `apple_fm_bridge`: attach-only local health checks,
   plain-text smoke, snapshot streams, callback-tool sessions, Blueprint tool
   projection, Program Run evidence, capability reports, and redacted receipts.

The original product thesis from Episodes 194 and 201 was not "Apple FM
replaces Codex." It was: use on-device Apple Silicon for bounded local
orchestration, title/summary work, codebase search, and small tool-backed
subtasks, then route that capability through Pylon as one sellable local
compute envelope once market plumbing, receipts, settlement, and live provider
evidence are green.

The current truth is therefore yellow, not green:

- the backend/runtime material is real;
- the current monorepo now retains a buildable Swift Foundation Models bridge
  helper under `apps/pylon/swift/foundation-bridge`;
- the current monorepo has Probe/Pylon Apple FM client, capability, control,
  helper-discovery, fake-bridge tests, Autopilot Desktop loopback coverage,
  and admitted-Mac local Autopilot smoke evidence;
- live Pylon market claims still require current receipts, wallet readiness,
  assignment or NIP-90 evidence, and settlement proof.

## Source Set Read

Requested transcript sources:

- `docs/transcripts/194.md`
- `docs/transcripts/201.md`

Current monorepo sources:

- `packages/probe/docs/2026-06-07-apple-fm-first-backend-audit.md`
- `packages/probe/docs/2026-06-07-blueprint-signature-lookup-apple-fm-tool-use-audit.md`
- `packages/probe/docs/probe-apple-fm-backend.md`
- `packages/probe/docs/apple-fm-admitted-mac-acceptance.md`
- `packages/probe/packages/runtime/src/backends/apple-fm/*`
- `packages/probe/packages/runtime/src/fleet/backend-capability.ts`
- `packages/probe/packages/runtime/tests/apple-fm-*`
- `apps/pylon/docs/2026-06-09-probe-to-pylon-port-audit.md`
- `apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md`
- `apps/pylon/docs/proofs/2026-06-13-stranger-probe-no-spend-owner-operated-registered-responder.md`
- `apps/openagents.com/docs/2026-06-10-five-bitcoin-revenue-streams-promise-audit.md`
- `apps/openagents.com/docs/2026-06-10-pylon-training-compute-modes-promise-audit.md`
- `docs/2026-06-10-always-on-fleet-plan.md`
- `docs/pylon/2026-06-10-v03-sprint-agent-economy.md`
- `docs/promises/registry.md`

Git-history sources in this repo:

- `1beb1235f` and pre-`7fe94a951`:
  `src/llm/foundation-models.ts`
- `65ce6a989`, `ea95f2fc2`, `bd41600f7`:
  `swift/foundation-bridge/*`
- `59ecc78e7`, `9bec6f155`, `acc4fd65a`:
  `ios/OpenAgentsCore/Sources/OpenAgentsCore/*`

Sibling/recovered source-material sources:

- `/Users/christopherdavid/work/psionic/docs/audits/2026-03-10-apple-fm-swift-bridge-audit.md`
- `/Users/christopherdavid/work/psionic/docs/FM_BRIDGE_CONSIDERATIONS.md`
- `/Users/christopherdavid/work/psionic/docs/ROADMAP_FM.md`
- `/Users/christopherdavid/work/psionic/docs/FM_API_COVERAGE_MATRIX.md`
- `/Users/christopherdavid/work/psionic/docs/TASSADAR_APPLE_FM_PLUGIN_SESSION.md`
- `/Users/christopherdavid/work/backroom/reference/openagents-docs/*`
  hits for old `foundation-bridge` plans and validation notes.

## Transcript Thesis

### Episode 194

Episode 194 framed Apple Foundation Models as an unusually important local
agent primitive. The concrete claims in the transcript were:

- Apple FM was already useful for agentic codebase search, including
  grep-style tool calls.
- The model was being run on an M2 chip and streamed over WebSockets to a
  mobile device for controlling QuadCode.
- The intended use was to supplement QuadCode/Codex, not replace them.
- The first expected workload reduction was small: local orchestration and
  title summaries could move cloud share from 100 percent to about 95 percent,
  with more offload possible over time.
- The broader thesis was that Apple Silicon might carry a meaningful share of
  future inference if local model APIs and developer adoption mature.

This maps directly to the code we later had: title summarization, local
workspace exploration, code/session search tools, and controller-side routing
around stronger cloud coding agents.

### Episode 201

Episode 201 renamed the market thesis as "Fracking Apple Silicon." It added
the Pylon layer:

- Apple Silicon is stranded compute unless the product supplies discovery,
  standard job packaging, trust, settlement, observability, replay, receipts,
  and routing.
- Pylon was described as node software that makes a user's computer available
  on an open market with a built-in Bitcoin wallet.
- Apple Silicon was the first target because no model download is needed when
  Apple FM is already built into the machine.
- "Go online and earn Bitcoin" remains a market claim, not a runtime claim; it
  only becomes honest when the market plumbing and receipts are live.

The current promise registry keeps the same posture:

- `edge.apple_silicon_local_orchestration.v1` is yellow.
- `pylon.open_compute_market_with_wallet.v1` is yellow.
- `pylon.apple_silicon_button_money.v1` is red.
- `compute.market_plumbing_receipts_routing.v1` is red.

## Historical Native Swift App Work

Before the bridge became the main integration seam, the repo had direct native
Swift Foundation Models usage under `ios/OpenAgentsCore`.

### Title Summarization

`FoundationModelSummarizer.swift` attempted on-device conversation-title
generation:

- compile-gated with `canImport(FoundationModels)`;
- availability-gated on `SystemLanguageModel.default`;
- used a `LanguageModelSession` with no tools;
- asked for 3-5 plain words;
- used low temperature;
- treated guardrail or SDK response-shape failures as fallback signals.

This is the exact kind of small local orchestration Episode 194 described.

### Native Workspace Exploration

`FMTools.swift` projected OpenAgents operations as Apple FM tools:

- `session.list`
- `session.search`
- `session.read`
- `session.analyze`
- `content.get_span`
- `code.grep`
- `fs.list_dir`

`FMOrchestrator.swift` created a `LanguageModelSession` with those tools and
instructions to explore a workspace, analyze recent sessions, inspect code,
and summarize findings. `ExploreOrchestrator.swift` could route into this
native tool-calling loop when `policy.use_native_tool_calling` was set.

This is the earliest concrete "Apple FM does agentic codebase search" path in
the repo history.

### OpenAgents Local Provider

`OpenAgentsLocalProvider.swift` later made Apple FM part of the default local
provider lane on macOS:

- it streamed Foundation Models snapshots as ACP `agentMessageChunk` deltas;
- it kept a `LanguageModelSession` per ACP session;
- it prewarmed the session;
- it had deterministic fallback text if Foundation Models was unavailable;
- it registered a `codex.run` Foundation Models tool that emitted ACP tool
  call and tool update events.

That lane is important because it shows the product boundary: Apple FM was
used as a local coordinator and delegator around Codex, not as the whole
coding-agent runtime.

## Historical TypeScript Bun Client Work

The earliest Bun lane studied how to call Foundation Models from JavaScript.
`docs/research/deep-research/bun-foundation-models.md` concluded that Apple FM
was a native Apple framework, not a Bun/Node package or cloud API, so the
pragmatic path was a local Swift bridge.

`src/llm/foundation-models.ts` then implemented a Bun/Effect client:

- default port `11435`;
- binary discovery through `FM_BRIDGE_PATH`, repo `bin/foundation-bridge`,
  Swift build output, `~/.local/bin`, `/usr/local/bin`, and Homebrew paths;
- macOS gating;
- health checks against `/health`;
- optional auto-start with `Bun.spawn`;
- OpenAI-shaped chat request/response conversion;
- graceful failure on unavailable bridge/model;
- later singleton lock handling to prevent concurrent bridge starts;
- `listModels` support before the TypeScript modules were removed.

This TypeScript client was deleted in the Rust/Swift/Psionic reshaping and
later replaced by the current Probe/Pylon Bun/Effect backend contract.

## Historical Swift Foundation Bridge

The Swift bridge was real, buildable source code in this repo history, not just
a plan.

### First Bridge Shape

At `65ce6a989` and `ea95f2fc2`, `swift/foundation-bridge` exposed:

- `GET /health`;
- `GET /v1/models`;
- `POST /v1/chat/completions`;
- later session endpoints.

The README documented requirements:

- macOS 26+;
- Apple Silicon;
- Apple Intelligence enabled;
- Xcode/Swift compiler for building.

`ChatHandler.swift` imported `FoundationModels`, checked
`SystemLanguageModel.default.availability`, created `LanguageModelSession`s,
called `respond(to:)`, applied basic `GenerationOptions`, and estimated usage
roughly from character counts. The later one-shot implementation deliberately
created a fresh session per request to avoid context accumulation.

### Session, Streaming, Structured Generation, And Tools

The later bridge generation at `bd41600f7` added the shape the current runtime
still mirrors:

- explicit `SessionCreateRequest`;
- model use-case and guardrail configuration;
- `ToolDefinition` with `arguments_schema`;
- `ToolCallbackConfiguration` containing callback URL and `session_token`;
- transcript JSON and typed transcript restore;
- `POST /v1/sessions/{id}/responses`;
- `POST /v1/sessions/{id}/responses/structured`;
- `POST /v1/sessions/{id}/responses/stream`;
- snapshot stream events and terminal completion events.

`RemoteTool` implemented Apple's `Tool` protocol and called back to the
controller over HTTP with:

- `session_token`;
- `tool_name`;
- generated structured `arguments`.

The bridge decoded the controller response as tool output or surfaced a typed
tool error back through Foundation Models.

### Why It Was Wiped From Current Root

The current `main` no longer contains root-level `swift/foundation-bridge` or
the Swift iOS app tree. Relevant history includes:

- `f5919c766` rebuilding the repo as a Bun/Effect workspace;
- `2f1ba3abd` deprecating root-level `swift` among other root material;
- the later mobile direction switching toward Expo/React Native.

The current repository therefore preserves the Apple FM contract and product
plans, but not the actual Swift bridge source under the current root. If the
bridge needs to be restored, use git history and the Psionic retained bridge
docs as source material instead of inventing a new contract.

## Psionic Apple FM Work

The sibling `psionic` repo preserved and expanded the bridge lane before the
current openagents rebuild.

The March 10 Psionic audit found:

- active `swift/foundation-bridge/` package;
- Rust desktop sidecar supervision in `apps/autopilot-desktop`;
- health polling, start/stop, and generation commands;
- provider-mode lifecycle wiring;
- UI readiness/blocker display;
- provider inventory, NIP-90 capability selection, and delivery-proof metadata.

Its gaps were also explicit:

- Apple FM was not yet the default user-facing local inference lane;
- the early bridge was minimal and lossy;
- token accounting was approximate;
- request parameters were limited;
- packaging and sidecar distribution needed hardening.

`ROADMAP_FM.md` then moved the reusable contract into
`crates/psionic-apple-fm`:

- typed system-model availability, use cases, guardrails, and unavailable
  reasons;
- model listing;
- explicit session lifecycle;
- transcript export/import;
- generation options and sampling modes;
- first-class text generation;
- snapshot streaming;
- structured generation with schema-derived Rust types;
- real Apple FM tool callbacks through a loopback runtime;
- typed Foundation Models error mapping.

`FM_BRIDGE_CONSIDERATIONS.md` is the strongest retained operational spec:
Swift calls Apple; Rust talks to Swift over localhost; desktop owns discovery,
launching, packaging, UI, and user-facing readiness. It also records the
packaging rule that shipped apps should include `FoundationBridge.app` and
launch it as a helper bundle rather than asking users to build Swift locally.

## Current Probe/Pylon Runtime Work

The current monorepo's retained Apple FM implementation lives under
`packages/probe/packages/runtime/src/backends/apple-fm`.

### Backend Contract

Current constants:

- backend kind: `apple_fm_bridge`;
- profile: `apple-fm-local`;
- model: `apple-foundation-model`;
- default base URL: `http://127.0.0.1:11435`;
- readiness path: `/health`;
- attach mode: `attach_existing`;
- auth mode: `none`;
- stream mode: `snapshot`.

Base URL resolution preserves the old order:

1. explicit assignment/profile override;
2. `PROBE_APPLE_FM_BASE_URL`;
3. `OPENAGENTS_APPLE_FM_BASE_URL`;
4. default loopback URL.

### Readiness And Receipts

The client performs live `GET /health` checks and returns typed readiness:

- `ready`;
- `unavailable`;
- `unsupported`;
- `malformed`;
- `unreachable`.

Unavailable reasons include:

- `bridge_unreachable`;
- `apple_intelligence_disabled`;
- `unsupported_hardware`;
- `model_unavailable`;
- `permission_denied`;
- `malformed_response`;
- `not_ready`;
- `unknown`.

Receipts are redacted and explicit:

- `probe_backend_availability`;
- `probe_backend_failure`;
- `probe_backend_transcript`.

Usage truth is `exact`, `estimated`, or `unknown`. The runtime intentionally
does not label Apple FM character-derived usage as exact.

### Pylon Loopback Status Projection

As of #5070, Pylon also exposes the runtime Apple FM capability report through
the token-authenticated loopback control API:

- command: `POST /command` with `{ "type": "apple_fm.status" }`;
- response schema: `openagents.pylon.apple_fm.status.v0.1`;
- implementation: `apps/pylon/src/node/apple-fm-status.ts` and
  `apps/pylon/src/node/control-server.ts`;
- regression tests: ready, unsupported, unreachable, and malformed health in
  `apps/pylon/tests/control-protocol.test.ts`.

The projection does not run inference and does not advertise
`probe.backend.apple_fm_bridge` unless live `/health` plus safe Blueprint
projection support are ready. It redacts base URLs and adds blocker refs for
desktop and future operator surfaces.

### Local Autopilot Session Path

As of #5072 and #5073, Pylon and Autopilot Desktop also have a bounded local
chat/tool-session path plus public-safe admitted-Mac evidence for the
user-owned Apple FM mode:

- Pylon accepts `apple_fm.session.start` through the token-authenticated
  control session API only after `apple_fm.status` is ready.
- The session runner uses `apps/pylon/src/node/apple-fm-local-session.ts` to
  call the local bridge through `streamSessionWithTools`.
- The initial tool set is deliberately read-only: `read_file`, `list_files`,
  and `code_search`, all bounded to the configured workspace.
- Autopilot Desktop starts the session through Bun-owned RPC
  (`startAppleFmSession`) and projects only public-safe event summaries into
  the Foldkit webview.
- Regression coverage lives in `apps/pylon/tests/apple-fm-control-session.test.ts`
  and desktop focused tests for control verbs, sanitization, Pylon control,
  and the Foldkit Agent pane.
- The desktop/Pylon loopback integration test lives in
  `apps/autopilot-desktop/tests/apple-fm-loopback-integration.test.ts`.
- The admitted-Mac runbook and public-safe smoke evidence live in
  `docs/apple-fm/2026-06-15-local-autopilot-admitted-mac-runbook.md` and
  `docs/apple-fm/2026-06-15-local-autopilot-admitted-mac-smoke-evidence.md`.

This is source and fake-bridge evidence for local user-owned tool/chat. It is
not a market-provider claim, hosted-compute fallback, Codex-parity claim, or
proof that the signed installer bundles/supervises the helper.

### CLI Surface

Current Probe CLI commands:

- `probe apple-fm status`;
- `probe apple-fm smoke`;
- `probe apple-fm tool-stream-demo`.

The status command does not run inference. The smoke command requires ready
health first. The tool-stream demo now flows through static Blueprint registry
fixtures, signature lookup, a backend-independent tool-menu planner, and the
Apple FM projector before creating the Foundation Models session.

### Tool Callback Lane

Current supported Apple FM tool names are:

- `read_file`;
- `list_files`;
- `code_search`;
- `shell`;
- `apply_patch`;
- `consult_oracle`;
- `analyze_repository`;
- `propose_action_submission`.

The current callback runtime owns:

- callback token validation;
- approval-pending and refused states;
- round-trip limits;
- transcript entries;
- redacted callback receipts;
- resume from Probe transcript state.

The Swift bridge callback payload shape is still recognized:

- body `session_token`;
- body `tool_name`;
- body `arguments`;
- response `{ "output": "..." }` on success.

### Blueprint Projection

Apple FM has one important constraint: tools must be selected before session
creation. It cannot safely discover and add arbitrary tools mid-session. The
runtime therefore uses a preflight path:

1. lookup Blueprint Program Signatures;
2. plan a bounded Probe tool menu;
3. project the menu into Apple FM-compatible generation schemas;
4. create the Apple FM session with only that selected menu;
5. record Program Run evidence after the stream.

The projector rejects unsafe schemas before session creation, including
unbounded `additionalProperties`. It adds Apple FM-friendly details such as
root object title and `x-order`, and maps `propose_action_submission` as
approval-required.

Program Run evidence is evidence-only:

- no deploy;
- no email;
- no source mutation;
- no spend;
- content redacted;
- callback refs and receipt refs only.

### Capability Reporting

`reportAppleFmBackendCapability` reports Apple FM as a runner capability only
when live health and Blueprint projection support are green. The report carries
Apple Silicon and Apple Intelligence as required facts, snapshot streaming and
tool callback support, and redacted availability receipts. Unsupported states
remain visible but are not advertised as usable capacity.

## Pylon Planning And Market Use

Pylon planning treats Apple FM as the first easy local inference backend, not
as the whole market.

### Pylon v0.3 Runtime Port

`apps/pylon/docs/2026-06-09-probe-to-pylon-port-audit.md` says Probe runtime
code was ported into Pylon as `@openagentsinc/pylon-runtime`, including Apple
FM client, readiness contract, streaming fixtures, callback tool sessions,
Blueprint tool projection, acceptance cases, receipts, and Program Run
evidence.

`apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md`
then scopes the launch:

- Pylon is the public node package;
- Apple FM support is one included local runtime;
- public earning claims still require registration, heartbeat,
  wallet-readiness, assignment closeout, proof, and settlement gates.

### Compute Revenue Plans

The product-side compute audits keep two separate truths:

- Apple FM inference can become sellable through NIP-90 kind 5050 with settled
  receipts.
- Broad "one install earns Bitcoin" copy remains blocked until fresh v0.3
  live work/payment/settlement evidence exists.

The 2026-06-13 owner-operated stranger-probe proof is the clearest current
Apple FM market evidence: a registered provider loop used local Apple FM at
`http://127.0.0.1:11435`, issued a payment-required quote, processed the
request, and published a kind-6050 Apple FM completion. It was explicitly
no-spend and owner-operated, so it is evidence for registered capacity serving
a stranger-shaped request, not evidence for independent paid market demand.

### Always-On Fleet

`docs/2026-06-10-always-on-fleet-plan.md` explains why the Pylon work depends
on machines staying online. From the Apple FM perspective, this matters because
an Apple FM backend that is ready for five minutes is not sellable market
supply unless the Pylon process also:

- registers;
- heartbeats;
- declares the capability;
- reports wallet readiness;
- restarts after failure;
- avoids stale public counters.

### Benchmark And GEPA Plans

The Pylon benchmark docs include Apple FM as a route to compare against Codex,
Probe+Codex, local Qwen, SHC, and Pylon. They also preserve an important
negative boundary: GEPA/benchmark optimizer acceptance does not grant runtime
promotion authority, and no Apple FM MLX fine-tuning lane is proven by those
benchmark docs.

## Boundaries And Non-Claims

Do not overclaim these points:

- Apple FM is not a complete Codex replacement in current evidence.
- Apple FM support does not imply exact token accounting.
- Apple FM support does not imply Apple adapter training, MLX training,
  DPO/GRPO, or distributed neural training.
- A local `apple_fm_bridge` readiness report is not a market receipt.
- A Pylon registration/heartbeat is not settlement evidence.
- Owner-operated no-spend NIP-90 proof is not independent paid-stranger proof.
- The current monorepo does not retain the Swift bridge source at root.
- The current runtime should not expose all possible tools to Apple FM by
  default; the selected tool menu must be bounded before session creation.
- Callback URLs, callback tokens, local paths, prompts, tool inputs, provider
  payloads, wallet material, and transcript contents must stay out of public
  receipts.

## What To Preserve

The durable integration decisions are:

- keep Apple FM as a named backend kind, not a generic "local inference"
  alias;
- keep the loopback bridge contract on `127.0.0.1:11435`;
- keep attach-only readiness as the default runtime posture;
- keep Apple Silicon and Apple Intelligence as explicit requirements;
- keep snapshot-stream semantics, not fake token deltas;
- keep usage truth typed as exact/estimated/unknown;
- keep Probe/Pylon transcript and tool-execution authority outside the model;
- keep Blueprint preflight for Apple FM tool menus;
- keep Action Submission as the boundary for direct effects;
- keep admitted-Mac live tests separate from default CI;
- keep unsupported/unavailable distinct from failed.

## Restoration Notes

If the Swift bridge has to be restored into this monorepo:

1. Start from git history `bd41600f7:swift/foundation-bridge`.
2. Cross-check against Psionic's `FM_BRIDGE_CONSIDERATIONS.md` and
   `ROADMAP_FM.md` before editing.
3. Preserve the current Probe/Pylon TypeScript contract rather than inventing
   new endpoint names.
4. Prefer packaging `FoundationBridge.app` as a helper bundle for shipped
   macOS apps.
5. Keep bridge launch/supervision in the app/node owner, not in the generic
   contract crate/package.
6. Re-add tests for health, unsupported hardware, estimated usage,
   session-stream snapshots, transcript restore, callback token binding, and
   redaction.

## Current Honest State

Apple FM is one of the better-proven local backend ideas in the OpenAgents
history. It has:

- direct native Swift usage history;
- a restored buildable Swift bridge helper;
- a mature Psionic contract/client plan;
- current Probe/Pylon Bun/Effect runtime support;
- current fake-bridge CI coverage;
- Pylon loopback `apple_fm.status` readiness projection;
- Autopilot Desktop public-safe Apple FM readiness/mode UI;
- a Pylon/Desktop bounded local Apple FM chat/tool session path with
  fake-bridge coverage;
- admitted-Mac runbooks and source/local smoke evidence for the Autopilot
  loopback path;
- modeled local-session power/kWh denominator evidence retained in Apple FM
  control-session proofs (#5074);
- one owner-operated NIP-90 Apple FM provider proof.

It is not yet a green public earning promise. The green path is concrete:
restore or package the bridge where needed, keep Pylon online, pass an
admitted-Mac local Autopilot smoke for the user-owned tool/chat path, advertise
`apple_fm_bridge` only after live health, serve a paid NIP-90 or assignment job
with Apple FM, settle sats to an admitted payout target, and record public-safe
receipts without leaking local runtime contents. The modeled local-session kWh
estimate is not measured telemetry and is not AO/kWh unless joined to a verified
accepted-outcome receipt.
