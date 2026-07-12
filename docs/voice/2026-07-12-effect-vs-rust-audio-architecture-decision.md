# Effect vs Rust for persistent audio — architecture decision

**Implementation status (AUDIO-1, #8734):** accepted. The canonical Effect
contract is `packages/audio-contract`; the media-only Rust mirror is
`crates/oa-desktop-audio`; both consume
`fixtures/audio-contract/media-v1.json`. Later audio leaves may extend those
surfaces but may not widen Rust into application or policy authority.

**Measured AUDIO-2 result (#8735):** the Effect/Bun gateway packaged the
official Google Node gRPC client and completed a real Chirp 3 streaming smoke
on private Cloud Run. Its cancellation/runtime threshold passed, so the ADR's
Rust-gateway fallback is not activated.

**Measured AUDIO-4 result (#8737):** the narrow Rust helper now provides real
`cpal` capture/playback and direct TLS WebSocket media transport while Electron
main retains permission, grant, lifecycle, and finalizer authority. Rust's
boundary did not expand into renderer, command, Sync, retention, or credential
authority, so the accepted hybrid decision remains intact.

- **Date:** 2026-07-12
- **Status:** accepted planning decision for [AUDIO-0 #8733](https://github.com/OpenAgentsInc/openagents/issues/8733)
- **Owner direction:** Rust is permitted for this track when it is the better systems boundary; do not force the whole feature into Effect
- **Related plan:** [`2026-07-12-persistent-desktop-voice-mode-audit-and-plan.md`](./2026-07-12-persistent-desktop-voice-mode-audit-and-plan.md)

## Decision

Use a **hybrid architecture**, divided by authority and realtime constraints:

- **Effect / TypeScript owns the application and cloud control plane:** the
  canonical Effect Schema contract, Electron host supervision, Runtime Gateway
  commands/projections, Effect Native UI, owner/session policy, semantic command
  selection, existing conversation/command outcomes, the private Cloud Run
  gateway, Google STT/TTS adapters, storage orchestration, and receipts.
- **Rust owns one narrow Desktop media-engine executable:** microphone capture,
  sample conversion/resampling, packetization, local sequence/digest, bounded
  audio buffers, the direct authenticated media WebSocket, playback, device
  changes, and realtime cancellation/barge-in buffer flush.
- **The renderer owns neither.** It receives only bounded Effect projections
  and emits closed typed intents.

The planned Rust home is `crates/oa-desktop-audio`, packaged and signed as an
OpenAgents Desktop helper. Electron main supervises it through a closed,
versioned, line-delimited local control protocol. The helper sends media
directly to `apps/openagents-audio`; raw audio does not pass through the
renderer, preload, Runtime Gateway event stream, or ordinary main-process IPC.

This is not a return to Tauri, WGPUI, or a Rust application shell. It is the
same pattern already accepted for systems infrastructure and native provider
executables: a narrow process-opaque helper behind an Effect contract.

## Why this is the informed split

### Effect is the right place for application authority

The current Desktop is already built around:

- Effect Schema-decoded Runtime Gateway requests, responses, and subscriptions;
- a tokenless renderer and fixed preload methods;
- one registered command vocabulary for pointer, keyboard, palette, menu, and
  future model-proposed actions;
- Effect-scoped lifecycle, interruption, replacement, and disposal;
- Khala Sync projections and durable command outcomes; and
- Effect Native UI and executable behavior contracts.

Reimplementing any of those in Rust would create a second application model.
The voice feature would then need to keep two copies of owner/session scope,
command policy, schema versions, approvals, outcome grammar, and UI state in
agreement. That is exactly the duplication the Runtime Gateway and Effect
Native architecture were designed to prevent.

Effect is also the more pragmatic first cloud gateway:

- `apps/khala-live-hub` already supplies a tested Bun/Cloud Run WebSocket,
  authentication, catch-up, health, and deployment pattern.
- Google's official Node libraries provide the supported Speech-to-Text V2 and
  Text-to-Speech gRPC clients. Effect can wrap those promise/stream callbacks at
  a named perimeter while retaining typed resource scopes and cancellation.
- Existing TypeScript/Postgres/Cloud Storage code and deployment conventions
  reduce the work needed for session admission, manifests, quotas, metrics, and
  receipts.
- The semantic action bridge must call the existing TypeScript command and
  conversation services. Keeping it in the control plane avoids another RPC
  boundary around authority.

The cloud service may run under the runtime required by the supported Google
gRPC client while remaining authored as Effect/TypeScript. AUDIO-2 must prove
the exact packaged runtime; it may not assume Bun compatibility from typecheck
alone.

### Rust is the right place for the Desktop realtime media loop

The Electron utility-process idea is attractive in prose but insufficient by
itself. A Node-style utility process does not automatically provide a trusted
native microphone API. Browser `getUserMedia` would put capture and raw audio
inside the renderer, widening the boundary we explicitly want to keep
tokenless and projection-only. Native Node audio modules add their own ABI,
prebuild, and callback-safety burden.

Rust already has proven libraries and historical OpenAgents code for this job:

- `cpal` for cross-platform microphone/device capture;
- `rodio` or a lower-level output backend for playback;
- mature PCM/sample conversion and codec crates;
- Tokio plus WebSocket/TLS libraries for long-lived transport;
- bounded channels and owned task cancellation without blocking Electron; and
- the historical OpenAgents Rust voice playground as a concrete capture/WAV/
  lifecycle/test reference.

A Rust helper gives the operating system one clearly owned media process. It
can keep audio off JavaScript heaps, react to device callbacks without renderer
jank, flush playback promptly on barge-in, enforce hard byte/time bounds, and
die closed when Electron revokes or exits. Crash isolation is better than
loading a native audio addon into Electron main.

The helper is also portable to Windows/Linux later without changing renderer
or Runtime Gateway contracts. Platform-specific entitlements, device behavior,
and codecs remain contained behind the same control protocol.

## Alternatives considered

### All Effect / TypeScript

**Attractive because:** one language, direct use of existing contracts, easier
mocking, fewer build systems.

**Rejected for the Desktop media loop because:** the clean Node utility-process
path has no built-in native microphone; renderer capture violates the intended
trust boundary; native Node addons couple audio safety and packaging to
Electron's ABI; realtime audio bytes and device callbacks would add pressure to
the application process.

Effect remains correct for every non-media-loop responsibility.

### All Rust

**Attractive because:** excellent realtime/native performance, one binary could
own WebSocket, Google gRPC, codecs, storage, and playback.

**Rejected for the MVP because:** it would duplicate application authority,
discard working Cloud Run/TypeScript patterns, require new Rust Google auth/
gRPC/storage/SQL integration at once, and create a much larger new operational
surface. Rust Google Cloud client maturity and generated-proto maintenance are
additional risk compared with Google's supported Node/Python clients.

An all-Rust gateway can be reconsidered only if AUDIO-2's measured Effect/
Google-client spike fails a named criterion.

### Renderer `getUserMedia` plus direct WebSocket

**Rejected:** simplest demo, wrong product boundary. It gives privileged live
web code microphone capture, raw media, and channel state; makes reload/crash/
permission behavior harder to fence; and bypasses host supervision.

### Electron main plus native Node audio addon

**Rejected for MVP:** better than renderer capture but worse isolation and
packaging than a helper process. A crash or callback defect affects the main
process, native prebuilds must match Electron ABI, and raw audio still traverses
the application runtime.

### Rust gateway plus Rust Desktop helper from day one

**Deferred:** technically coherent but expands the first Rust exception from a
bounded native media engine into a second backend stack. Do this only with
evidence that the Effect gateway cannot meet latency, cancellation, memory, or
Google-client reliability budgets.

## Exact boundary

```text
Effect Native renderer
  typed intent/projection only
        |
preload + Electron main (Effect)
  start/stop/mute/config/status; process supervision; no raw media
        |
closed local control protocol (stdio or inherited private socket)
        |
oa-desktop-audio (Rust)
  mic -> PCM/resample -> bounded chunks -> authenticated media WebSocket
  speaker <- bounded TTS chunks <- generation/utterance checks <- WebSocket
        |
apps/openagents-audio (Effect/TypeScript on Cloud Run)
  auth/admission/frames -> Google STT/TTS -> commands/outcomes
  retained segments -> Cloud Storage; manifests/receipts -> Cloud SQL
```

### Effect-to-Rust local control messages

Effect may send only bounded commands such as:

- `configure` with public endpoint, short-lived opaque channel credential,
  voice session/generation, format, device preference, and limits;
- `start`, `mute`, `unmute`, `stop`, `set_output`, `cancel_utterance`;
- `rotate_credential` and `shutdown`.

Rust may return only bounded public-safe state such as:

- ready/unavailable and device summaries;
- capture/egress/playback/mute state;
- connection generation, ACK watermark, loss/backpressure counters;
- utterance playback start/drain/cancel;
- typed error/blocker and final shutdown receipt.

No raw audio, transcript, provider credential, object path, arbitrary log line,
or server command crosses this local control channel. Transcripts and command
proposals travel through the authenticated Effect application path and Runtime
Gateway projection, not back through the helper.

### Wire contract conformance

`packages/audio-contract` remains canonical. AUDIO-1 will provide:

- Effect Schema codecs and fixtures;
- a language-neutral binary-envelope specification;
- generated or hand-maintained Rust serde mirrors restricted to media frames;
- shared golden vectors for valid and invalid versions, sizes, sequence,
  generation, digests, and close/error frames; and
- cross-language conformance tests run in both package and Cargo suites.

The Rust helper must not learn command policy, Sync shapes, conversation bodies,
retention SQL, or model semantics. It implements media transport, not agent
authority.

## Packaging and security consequences

The helper adds real release work:

- build universal/per-architecture binaries for supported Desktop targets;
- place the executable outside ASAR under a fixed manifest path;
- include its digest/version in the Desktop component manifest;
- sign it inside the macOS application and cover it with hardened-runtime
  entitlements appropriate to microphone/audio access;
- verify executable presence, mode, signature, architecture, and digest before
  launch;
- launch with a scrubbed environment, fixed args, no ambient PATH resolution,
  no shell, and a private inherited control channel;
- use one owned process scope with bounded startup/shutdown and kill-on-parent-
  exit behavior; and
- include helper version/state in public-safe diagnostics without local paths,
  tokens, audio, or transcript text.

This cost is justified only because the helper materially narrows the media
boundary and removes native audio work from Electron. AUDIO-4 must prove
packaged behavior early, not defer it to AUDIO-8.

## Performance and reliability budgets

The split is accepted only if the real implementation proves:

| Budget | Initial gate |
| --- | --- |
| helper cold start to ready | p95 <= 500 ms on the named dogfood Mac |
| capture callback to gateway send | p95 <= 50 ms excluding network |
| sustained helper memory | bounded during a 60-minute session; no growth with transcript length |
| playback cancellation after local interrupt | p95 <= 100 ms before network acknowledgement |
| helper crash | Desktop shows degraded/stopped; no automatic hidden recapture |
| main/renderer responsiveness | no material regression during full-duplex audio |
| process teardown | capture, socket, playback, and child exit settle once within 2 s |

Cloud end-to-end STT/TTS and audible barge-in budgets remain in the master voice
plan. These native budgets isolate whether a miss is local media, network,
Google, or application authority.

## Falsifiers and fallback

The Rust-helper decision should be reversed for the MVP if AUDIO-4 proves all
of the following with a pure Effect utility process and no renderer capture or
native-addon risk:

1. native mic and playback work in packaged macOS/Windows/Linux artifacts;
2. no raw media or socket authority enters renderer/preload/Runtime Gateway;
3. latency, cancellation, memory, crash isolation, and device-change budgets
   pass; and
4. packaging/signing is materially simpler than the Rust helper.

The Effect-gateway decision should be reversed to a Rust gateway if AUDIO-2
shows, with a reproducible spike, that the supported Google streaming client
cannot run reliably in the chosen Cloud Run TypeScript runtime or misses
bounded memory/cancellation/rotation budgets. That decision must preserve the
same `packages/audio-contract` wire and may not move command/Sync authority into
Rust.

## Issue implications

- **AUDIO-1 #8734:** owns Effect schemas, media-only Rust mirrors, and golden
  cross-language conformance.
- **AUDIO-2 #8735:** defaults to an Effect/TypeScript Cloud Run gateway and must
  include a Google-client runtime spike before broad implementation.
- **AUDIO-4 #8737:** owns `crates/oa-desktop-audio`, the closed local control
  protocol, Electron supervision, packaging, signing, and helper fault tests.
- **AUDIO-5/#8738 and AUDIO-6/#8739:** remain entirely Effect Native/TypeScript.
- AUDIO-6 permits the Rust helper to validate and forward bounded server
  transcript/activity/proposal frames. Rust still cannot classify utterances,
  choose commands, apply policy, or claim outcomes; those remain Effect-owned.
- **AUDIO-7 #8740:** Google streaming synthesis stays in the Effect gateway;
  Rust owns output buffering/playback/cancel only.

**Measured AUDIO-7 result (#8740):** the Effect gateway's official Google
client streamed Chirp 3 HD with a 198 ms synthesis TTFB in the deployed smoke.
The signed Rust helper now owns only validation, resampling, the bounded output
queue, underrun observation, and prompt speech-ref-fenced flush. This further
supports the hybrid decision rather than widening either side's authority.
- **AUDIO-8 #8741:** proves cross-language contract compatibility, signed helper
  custody, crash/upgrade behavior, and the full latency decomposition.

## Final rationale

“Effect or Rust?” is the wrong unit of decision. The system has two different
kinds of work:

- reasoning about identity, state, policy, commands, outcomes, UI, storage, and
  service lifecycles; and
- moving realtime audio between native devices and the network with strict
  timing and bounded memory.

Effect is the stronger language/runtime architecture for the first. Rust is the
stronger implementation substrate for the second. The hybrid keeps the Jarvis
experience native and responsive without creating a Rust fork of the
OpenAgents application model.
