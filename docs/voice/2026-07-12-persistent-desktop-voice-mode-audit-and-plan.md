# Persistent Desktop voice mode — architecture audit and delivery plan

- **Date:** 2026-07-12
- **Status:** proposed, challenged expansion; not implementation authority or a product promise
- **Destination:** `apps/openagents-desktop`
- **Related authority:** `docs/sol/MASTER_ROADMAP.md`
- **Historical inputs:** Sarah/Hydralisk records in `docs/sarah/` and their deleted source in Git history

## 0. Decision summary

OpenAgents Desktop can support a user-opened, long-lived, full-duplex voice
session that remains connected while the user works. The right shape is a
dedicated authenticated media plane owned by the Desktop host and server, with
voice-derived actions entering the same typed command, approval, idempotency,
outcome, receipt, and Sync paths as keyboard or pointer actions.

The proposed mode must not be implemented as:

- a resurrection of Sarah, `/sarah/*`, the deleted `apps/sarah` package, a
  persona, avatar, opener, or the stopped Hydralisk GPU service;
- arbitrary server control over the renderer;
- raw audio flowing through Khala Sync or the Runtime Gateway event stream;
- a voice-only command authority; or
- an unqualified promise to save every frame in Cloud SQL.

There is one blocking product-policy conflict. The current Sol roadmap permits
explicit persona-neutral ASR, TTS, and barge-in, but says **no ambient recording
and no raw-audio retention by default**. A persistent open microphone plus
automatic retention of every frame materially expands that decision. It needs
an explicit owner decision, privacy/threat model, consent contract, retention
tiers, deletion/export behavior, and a bounded implementation issue before the
retention lane begins.

Recommended split:

1. Build the persistent transport and voice-session lifecycle with ephemeral
   media first.
2. Prove transcripts and a very small safe action set through existing command
   authority.
3. Add streaming TTS and barge-in.
4. Only then run an explicit opt-in retained-audio experiment backed by object
   storage, with SQL storing metadata and policy state rather than audio blobs.

## 1. Product intent

The Desktop toolbar exposes a Voice control. Clicking it opens an explicit,
visible, persistent two-way session:

- the client captures microphone audio and continuously sends bounded audio
  chunks;
- the server acknowledges ordered chunks, performs VAD/ASR/turn detection,
  and may stream audio replies;
- transcripts can create ordinary messages or propose registered UI/runtime
  actions such as follow-up, steer, interrupt, focus, or open;
- consequential actions retain the same visible confirmation and policy gates
  as text; and
- the session survives normal network changes through bounded reconnect,
  sequence acknowledgement, and gap recovery without duplicating commands or
  inference turns.

“Persistent” means the user intentionally starts one durable connection that
can span many turns until muted, stopped, revoked, expired, suspended by
policy, or terminated. It does not mean invisible background surveillance or
an application that silently reopens the microphone after restart.

The control must always display distinct state for microphone capture, network
egress, server listening, audio retention, and server playback. Muting capture
must stop egress, not merely hide a waveform.

## 2. Current Sol fit and conflict

### What the roadmap already authorizes

`docs/sol/MASTER_ROADMAP.md` decision 21 authorizes persona-neutral
conversational voice using the same typed command and outcome path as text.
R6 and R7 require voice follow-up/interruption and cross-device fault testing.
The portable-session pathway further describes provisional/final ASR,
editable transcripts where consequences warrant, streaming TTS, and barge-in
mapped to typed interruption.

The proposal touches nearly every reliability gate:

| Gate | Voice-mode implication |
| --- | --- |
| R1 | Owner/device identity, explicit consent, session scope, revocation |
| R2 | Ordered cursors, idempotency, duplicate suppression, gap/refetch behavior |
| R3 / D5 | Existing Fleet steer, approve, interrupt, stop, and durable outcomes |
| R4 | Lost ACK, replay, cancellation, generation fencing, network migration |
| R5 / D1–D4 | Desktop bridge, command registry, permissions, settings, diagnostics |
| R6 | Text-equivalent voice follow-up and barge-in without phone/host authority expansion |
| R7 | Signed build, privacy telemetry, suspend/restart/revocation and dogfood faults |

### What it conflicts with

The same decision explicitly rejects ambient recording, raw-audio retention by
default, and voice-only authority. The proposed “save all audio frames” policy
is therefore a challenged roadmap expansion, not an implementation detail.

Before retained audio ships, add a new owner decision that answers:

- Is retention off by default, opt-in per session, or opt-in at account/org
  policy level?
- Does the indicator distinguish “microphone live” from “audio retained”?
- What are the default and maximum TTLs? Who can extend them or impose legal
  hold?
- Can a user inspect, export, and delete audio and derived transcripts?
- What happens to backups, embeddings, analytics, and model-training copies on
  deletion?
- Which regions and storage custodians are allowed?
- Are bystanders or multi-speaker rooms supported, and how is their consent
  represented?
- Does free-tier data-sharing apply, or is voice governed by a distinct
  disclosure and training policy?

Until those answers land, the only honest default is ephemeral raw audio with
bounded in-memory buffering and separately governed transcript persistence.

## 3. Existing Desktop seams to extend

The active destination is the greenfield Effect Native/Electron app at
`apps/openagents-desktop`, not the legacy `apps/autopilot-desktop`,
`clients/khala-desktop`, or Sarah clients.

### Renderer boundary

The renderer remains tokenless and least-authority. It uses:

- `src/preload.cts` as the only renderer bridge;
- `src/runtime-gateway-contract.ts` for closed schema-decoded queries,
  commands, and event subscriptions;
- `src/runtime-live-subscriptions.ts` for generation-fenced subscriptions;
- `src/desktop-command-contract.ts`, `src/desktop-command-host.ts`, and
  `src/renderer/command-registry.ts` for one action vocabulary; and
- `src/runtime-gateway.ts` for host composition.

Add closed voice session intents such as:

- `voice.session.start`
- `voice.session.stop`
- `voice.session.mute`
- `voice.session.unmute`
- `voice.session.setRetention`
- `voice.proposal.confirm`
- `voice.proposal.reject`

The renderer receives a bounded projection, for example:

```text
disabled -> requesting_permission -> connecting -> listening
listening -> speech_detected -> transcribing -> listening
listening -> awaiting_confirmation -> executing -> listening
listening <-> speaking
* -> reconnecting -> listening | degraded | stopped
```

Projection fields may include public-safe refs, connection generation,
sequence/ACK watermark, mic/egress/retention/playback flags, last transcript
status, proposed command ID, and typed blocker. They must not include bearer
tokens, socket credentials, raw audio, absolute audio paths, provider payloads,
unredacted logs, or arbitrary control-frame bodies.

### Host and utility-process boundary

Electron main owns OS permission truth, session authorization, lifecycle,
revocation, and the schema-decoded renderer projection. A dedicated utility
process should own capture, resampling, VAD hints, packetization, encryption,
reconnect, jitter/backpressure buffers, playback, and device changes. This
keeps sustained audio work away from renderer responsiveness and follows the
existing requirement to move CPU-heavy services behind a utility process.

The Runtime Gateway may start/stop the voice scope and publish bounded state,
but raw audio should not traverse its ordinary event IPC. Do not expose a raw
`MessagePort`, generic IPC channel, localhost credential, or socket handle to
the renderer.

### Authority boundary

Server speech interpretation may yield text or a **proposed registered
command**. It may never yield an arbitrary “click selector,” JavaScript
snippet, filesystem path, shell command, or renderer mutation.

Each proposal is decoded against the Desktop command contract and bound to:

- owner, device, voice session, conversation, and exact target refs;
- a stable intent/idempotency identity;
- the command's normal readiness, permission, approval, and confirmation
  policy;
- transcript provenance and confidence for explanation, not authority; and
- the durable command outcome/ref already used by text and pointer input.

Raw audio, ASR text, TTS speech, and model prose never prove that an action was
accepted or completed. A timeout remains `unknown_pending_reconcile`.

## 4. Realtime protocol proposal

Use a dedicated bidirectional media transport. WebRTC is attractive for
audio/jitter/codec/NAT behavior and matches Hydralisk experience; an
authenticated binary WebSocket is simpler for an initial desktop-to-cloud
prototype and explicit application-level replay. Select the transport through
a measured spike, but freeze the application frames independently so the
authority contract does not depend on transport choice.

### Client-to-server frames

- `session_hello`: protocol version, session/device/conversation refs,
  generation, capabilities, audio format, requested retention tier.
- `audio_chunk`: monotonically sequenced audio, capture timestamps, duration,
  codec/format, key epoch, content digest, VAD hint; bounded payload.
- `capture_state`: muted/unmuted, device change, OS interruption, background or
  suspend transition.
- `transcript_correction`: user-edited text bound to a hypothesis/final ref.
- `proposal_decision`: confirm/reject a specific typed command proposal.
- `playback_state`: started/drained/interrupted for qualified TTFA/barge-in.
- `ack`, `heartbeat`, `rekey`, `goodbye`.

### Server-to-client frames

- `session_ready` / `session_rejected`: negotiated format, limits, retention
  receipt, generation, resume/ACK cursor.
- `ack` / `gap` / `must_refetch`: highest contiguous sequence and bounded
  recovery instruction.
- `vad_state`, `transcript_interim`, `transcript_final`: derived speech state
  with provenance/confidence/language.
- `assistant_text_delta` and `assistant_text_terminal`: canonical visible text.
- `tts_audio_chunk`: sequenced playback audio with utterance and interruption
  identity.
- `command_proposal`: only a registered command ID plus decoded bounded
  arguments, target refs, risk/confirmation posture, and proposal expiry.
- `command_ack` / `command_outcome_ref`: references to the normal authority
  path, not a parallel completion claim.
- `interrupt` / `cancel_playback`: scoped cancellation with generation and
  utterance ref.
- `limits`, `degraded`, `rekey`, `heartbeat`, `session_closing`.

Every frame needs a protocol version, tenant/owner scope established by the
authenticated channel, voice session ref, generation, sequence where relevant,
timestamp, bounded size, and strict schema decoder. Frame identity must resist
replay across sessions or generations. Unknown frame versions fail closed.

### Delivery semantics

- Audio transport is at-least-once under reconnect; chunk digests and sequence
  windows make storage/ASR ingestion idempotent.
- A final utterance ref is emitted exactly once per accepted turn. Interim
  hypotheses are replaceable and never execute actions.
- Replay of transcript/output frames is delivery-only: it does not rerun ASR,
  inference, persistence, or commands.
- Command proposals expire and bind to exact transcript/turn/target refs.
- Slow clients are bounded and detached or degraded; no unbounded queue.
- Generation fencing prevents an old resumed connection from accepting audio,
  playback, or decisions after a new connection becomes active.

## 5. Storage and data model

“Cloud SQL etc.” should become a split storage design:

### Object storage for audio

Store encrypted, immutable, bounded audio objects by owner/session/track/time
partition. Prefer short segments (for example, several seconds) or finalized
utterance objects rather than one object per 10–20 ms transport packet. Keep
the original packet sequence and digests in manifests so later analysis can
reconstruct loss and timing without making object-count cost pathological.

Use envelope encryption with a tenant/session key epoch, lifecycle TTL,
regional placement, retention lock only where policy explicitly requires it,
and audit logs for read/delete/export operations. No public object URLs.

### SQL for manifests and policy state

Cloud SQL/Postgres is appropriate for:

- voice session, generation, owner/device/conversation refs;
- consent disclosure/version and retention policy receipt;
- ordered chunk/segment manifests, digests, capture/server timestamps, codec,
  byte count, gaps, and object refs;
- utterance, transcript, model/ASR provenance, and correction refs;
- command proposal, confirmation, intent, and durable outcome refs;
- encryption key epoch, deletion/export/legal-hold state; and
- aggregate public-safe latency, loss, and cost metrics.

Do not store audio blobs in relational rows. Do not put raw audio or object
credentials in Khala Sync. Sync may distribute redacted session/transcript/
command projections needed by clients.

### Derived data is separately governed

Deletion and retention must name each class independently: raw audio,
normalized audio, transcript hypotheses, final/corrected transcripts,
speaker/voice features, embeddings, summaries, eval samples, model-training
copies, command receipts, and aggregate metrics. Deleting raw audio does not
implicitly prove all derived copies were removed; the deletion receipt must
state the exact classes and remaining lawful records.

## 6. Sarah and Hydralisk: what to reuse

Sarah was deliberately removed at commit `13bc1e7443`; `/sarah/*` is a 404
tombstone and its GPU host serves nothing. Git history is an archive, not a
package to restore.

Useful generic patterns from the deleted source include:

- `voice-stream-coordinator.ts`: authenticated exact scope, one executing
  stream per scope, linked cancellation, delivery-only replay, public-safe
  errors, and exactly one terminal record side effect;
- `voice-fragment-coalescer.ts`: bounded grouping for cumulative ASR
  fragments, one shared canonical result, busy rejection, active-group and
  timeout limits;
- `conversation-stream-fanout.ts`: bounded sequence replay, lag eviction,
  terminal complete/error/aborted/overflow states, one canonical record; and
- `realtime-token-guard.ts`: origin/CSRF, rate, active-session, TTL, daily
  budget, and alert concepts, while replacing its process-local Maps with
  distributed admission authority.

Useful Hydralisk/Pipecat-derived infrastructure patterns:

- typed audio and control frames;
- server-side VAD, segmented streaming STT, interim/final distinction, and
  semantic end-of-turn detection;
- immediate interruption frames with playback-buffer flush and backchannel
  guards;
- small interruption-grade output chunks;
- STT/LLM/TTS TTFB and first-audible TTFA metrics;
- tracked task ownership, heartbeat, cancellation, and close; and
- ICE restart/renegotiation concepts if WebRTC wins the transport spike.

Do not adopt the old browser final-only SpeechRecognition path. It caused
fragmenting and delayed barge-in. Do not colocate sustained ASR/VAD or GPU
inference with a latency-sensitive event loop: Hydralisk froze until blocking
render work moved off-loop and gained watchdog/silence-passthrough protection.

The reusable quality gates are decoded-frame media truth, bounded LIVE leases,
session eviction, authoritative stop, stale-generation fencing, and text
control remaining usable when media fails.

## 7. Threat, privacy, and abuse audit

Persistent voice increases the sensitivity and attack surface beyond Sarah's
text transcript flow. The threat model must cover:

- bystander, household, workplace, meeting, and notification capture;
- misleading UI when OS permission, selected device, mute, egress, retention,
  or playback state disagree;
- ambient prompt injection and speech from speakers/TVs;
- replayed or synthesized speaker audio and false identity assumptions;
- cross-tenant/session/generation frame replay;
- stolen resume tokens and long-lived connections after sign-out/revocation;
- partial uploads, duplicated chunks, silent loss, clock skew, and corrupt
  media used for later analysis;
- raw audio/transcripts leaking into logs, crash dumps, analytics, support
  bundles, traces, or model-provider errors;
- retention beyond disclosed TTL, incomplete deletion, backup resurrection,
  unauthorized export, region drift, and legal-hold ambiguity;
- unbounded bandwidth, storage, ASR/TTS/model spend, reconnect storms, and
  slow-consumer memory pressure; and
- accessibility, language, accent, speech impairment, and noisy-room failures
  that could turn low-confidence ASR into high-impact actions.

Controls include conspicuous UI and OS indicators, explicit session start,
muting that stops capture/egress, short-lived scoped channel credentials,
rekeying, per-frame limits/digests, generation fencing, quotas, backpressure,
encrypted storage, access audit, redacted diagnostics, confidence-aware review,
text fallback, and normal confirmation for destructive/spend/credential/
writeback/permission/isolation actions.

No speaker recognition should be treated as authentication. No semantic model
should bypass the registered command selector or invent commands through
keyword matching.

## 8. Reliability and observability budgets

Freeze exact budgets during the protocol spike. Initial targets to test:

| Measure | Initial target |
| --- | --- |
| Start click to authenticated session ready | p95 <= 2 s on warm network |
| Speech onset to VAD state | p95 <= 150 ms |
| Barge-in to server interrupt acknowledgement | p95 <= 500 ms |
| Barge-in to audible playback stop | p95 <= 750 ms |
| Final utterance to first assistant text | decomposed STT/LLM p50/p95 |
| Final utterance to first audible output | explicit TTFA p50/p95 |
| Heartbeat stale detection | bounded and visible, no indefinite listening state |
| Reconnect | resumes one generation or fails closed; never two active sessions |

Measure capture-to-ingest delay, packet/segment loss, gap recovery, ASR
interim/final latency, end-of-turn latency, inference first token, TTS first
byte, TTFA, playback underrun, interruption, reconnect, retained bytes,
deletion lag, and per-session cost. Metrics use refs and aggregates; logs do
not contain raw audio or transcripts.

## 9. Ordered delivery plan

### V0 — decision and contracts

- Add the owner decision resolving ambient/persistent capture and raw-retention
  defaults.
- Write privacy, consent, retention, deletion/export, training-use, tenancy,
  and regional custody contracts.
- Register behavior contracts for visible mic/egress/retention truth, mute,
  text fallback, confirmation, and media failure.
- Freeze `VoiceSession`, `AudioChunk`, `ServerControl`, proposal, ACK/gap,
  generation, and retention-receipt schemas.
- Model bounded session/generation/sequence/reconnect behavior; turn meaningful
  counterexamples into protocol tests.

**Exit:** schemas reject cross-session replay, two active generations,
unbounded frames, unknown controls, command completion from prose, and retention
without a matching consent/policy receipt.

### V1 — transport simulator, no microphone

- Build client/server frame codecs and deterministic simulators.
- Prove connect, heartbeat, ACK, gap, reconnect, rekey, backpressure, revoke,
  and close.
- Compare WebRTC and authenticated binary WebSocket with real network changes.

**Exit:** fault injection produces no duplicate accepted chunk, inference turn,
or action and no unbounded queue.

### V2 — explicit persistent capture, ephemeral media

- Add the Desktop toolbar control and bounded session projection.
- Host/utility process owns OS permission, capture, resampling, packetization,
  and transport.
- Start only after an explicit click; mute stops egress; suspend/restart does
  not silently reopen capture.
- Retain raw audio only in bounded buffers required for realtime processing.

**Exit:** packaged Desktop survives start/mute/device-change/suspend/network-
change/revoke/stop with truthful indicators and zero retained raw audio.

### V3 — VAD, ASR, and transcript truth

- Add process-isolated server VAD, segmented streaming ASR, interim/final
  frames, and semantic end-of-turn.
- Emit exactly one final utterance; allow visible correction before risky use.
- Persist only the explicitly permitted derived transcript class.

**Exit:** noisy/reconnect/replay fixtures and real speech produce one final
turn; low-confidence ambiguity cannot silently execute an action.

### V4 — small safe action set

- Route `message/follow-up`, `steer`, `interrupt`, and harmless `focus/open`
  proposals through the existing Desktop command registry.
- Require ordinary confirmations and outcomes. Keep text controls visible.
- Do not add ad hoc keyword routing; use the central typed semantic selector
  and bounded decoder after command selection.

**Exit:** voice and text produce the same intent schema and durable result;
lost ACK/reconnect never duplicates execution.

### V5 — two-way speech and barge-in

- Stream canonical assistant text and TTS audio.
- Bind playback chunks to utterance/generation refs.
- VAD/interim speech triggers qualified interruption and immediate buffer
  flush; backchannel guards reduce false interruption.

**Exit:** first-audible and interruption budgets pass; failed audio never hides
text or command state.

### V6 — opt-in retained-audio experiment

- Begin only after V0's owner/privacy decision.
- Store encrypted segments in object storage and manifests in SQL.
- Implement TTL, delete/export, legal hold, access audit, cost limits, regional
  policy, and explicit retention receipts.
- Provide a clear per-session retained/not-retained indicator and controls.

**Exit:** retention cannot start without matching consent; deletion/export and
expiry are receipt-proven across raw and derived data classes; raw media never
appears in Sync, logs, analytics, or support bundles.

### V7 — R7 dogfood and release

- Exercise long sessions, background/minimize/suspend, device changes, offline,
  lost ACK, server deploy, revocation, update/rollback, quota, storage failure,
  ASR/TTS degradation, and incident-safe diagnostics.
- Test at least one Desktop-to-mobile continuation without transferring the
  Desktop microphone or its channel authority.

**Exit:** signed owner-accepted receipt with no hidden capture, forked session,
  duplicate command, stale playback, leaked media, false outcome, unbounded
  cost, or undeletable retained data.

## 10. Open decisions

1. Is persistent mode explicit click-to-open only, or may a wake word activate
   capture? Recommendation: explicit click only for the first release.
2. WebRTC or binary WebSocket for V1? Decide by spike; keep frame schemas
   transport-neutral.
3. Which ASR/TTS providers and local fallbacks satisfy latency, privacy,
   language, and custody requirements?
4. Is transcript review mandatory only for high-risk commands, or configurable
   for all messages?
5. What is the first harmless UI command set beyond follow-up and interrupt?
6. Is audio retention a paid/privacy tier, an eval-only experiment, an org
   policy, or never a default product behavior?
7. Which exact raw and derived data classes may be used for later analysis or
   training, and under what revocable consent?
8. What are the session duration, idle timeout, bandwidth, retained-byte, ASR,
   TTS, and inference caps?

## 11. Recommendation

Proceed with V0–V2 as a bounded persistent-transport program under the active
Desktop architecture, with ephemeral audio and no action execution at first.
That proves the genuinely new technical value without prematurely coupling it
to permanent surveillance, model authority, or a storage policy.

Treat retained audio as V6: a separately authorized, explicit opt-in data
product with its own receipts. Reuse Sarah/Hydralisk's hard-won frame,
cancellation, fanout, media-truth, and operational lessons, while keeping its
deleted product surface dead. The durable invariant is simple: voice is a
modality into OpenAgents' existing authority system, never an authority of its
own.
