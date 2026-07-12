# Persistent Desktop voice mode — lineage, vision, architecture, and delivery plan

- **Date:** 2026-07-12
- **Status:** vision and architecture plan; not implementation authority or a product promise
- **Destination:** `apps/openagents-desktop`
- **Related authority:** `docs/sol/MASTER_ROADMAP.md`
- **Historical inputs:** Sarah/Hydralisk records in `docs/sarah/` and their deleted source in Git history

## 0. Vision summary

OpenAgents Desktop can support a user-opened, long-lived, full-duplex voice
session that remains connected while the user works. The right shape is a
dedicated authenticated media plane owned by the Desktop host and server, with
voice-derived actions entering the same typed command, approval, idempotency,
outcome, receipt, and Sync paths as keyboard or pointer actions.

This is the current form of an idea that has run through OpenAgents for years:
the computer should be something you can talk to at the level of intent, not a
pile of forms and text boxes. Onyx put that idea in a pocket: speak a wish,
watch agents work, and inspect details only when needed. Commander moved it to
the desktop and made the aspiration explicit: a Jarvis-like command surface for
a fleet of agents, closer to a strategy-game HUD than a conventional IDE.

Persistent voice mode should join those two strands. Desktop supplies the
high-density visual field, local capabilities, agent topology, files, diffs,
terminals, and approvals. Voice supplies a continuous low-friction way to
direct attention and express intent while the user's eyes and hands are doing
something else. The server supplies comprehension, orchestration, and spoken
response. The result is not merely hands-free chat; it is a conversational
operating layer over the whole OpenAgents workbench.

The proposed mode must not be implemented as:

- a resurrection of Sarah, `/sarah/*`, the deleted `apps/sarah` package, a
  persona, avatar, opener, or the stopped Hydralisk GPU service;
- arbitrary server control over the renderer;
- raw audio flowing through Khala Sync or the Runtime Gateway event stream;
- a voice-only command authority; or
- an unqualified promise to save every frame in Cloud SQL.

Recommended delivery split:

1. Build the persistent transport and voice-session lifecycle with ephemeral
   media first.
2. Prove transcripts and a very small safe action set through existing command
   authority.
3. Add streaming TTS and barge-in.
4. Only then run an explicit opt-in retained-audio experiment backed by object
   storage, with SQL storing metadata and policy state rather than audio blobs.

## 1. The OpenAgents voice lineage

The transcript archive in `docs/transcripts/` shows a consistent product idea
expressed through several generations of software. The names and technical
stacks changed; the desired human-computer relationship did not.

### 1.1 Before Onyx: one simple wish into an inspectable agent system

The earliest OpenAgents arcs established the ingredients that voice would later
compose: agents with plans and actions, inspectable execution graphs, streamed
status, plugins/tools, payments, memory, and a coding-agent HUD. By episodes
103–117, AutoDev was already becoming a pair programmer whose work could be
observed through plans, artifacts, diffs, and a heads-up display. The important
pattern was **simple intent in, structured and inspectable work out**.

Voice was never envisioned as replacing those artifacts. It was a faster front
door to them. A spoken request could be loose and human; the system behind it
still needed plans, tools, progress, evidence, and a place for the user to
intervene.

The Jarvis shorthand predates Onyx. Episode
[`111`](../transcripts/111.md), *Heads-Up Display*, imagined telling the
computer to find the relevant files and then acting on the result inside the
HUD. Episode [`117`](../transcripts/117.md) described the panes, diffs,
notifications, and build state as an agent operating system and explicitly
wanted voice restored so it would feel like Jarvis. Episode
[`127`](../transcripts/127.md) carried the same image into agentic site
building: spoken intent causes the right code and status windows to appear and
fade as the work advances. From the beginning, the envisioned response to
speech was **visible work**, not only spoken prose.

### 1.2 Onyx: “say what you want and stuff should happen”

Episode [`139`](../transcripts/139.md), *Going Mobile*, asked what the proper
form factor for a personal AI agent should be. The answer was not another
dedicated gadget or a dashboard full of controls. It was the phone already in
the user's hand: one obvious control, a spoken wish, and a feed of agents doing
work. The user could drill into providers, budgets, events, and details when
desired, but should not have to micromanage the machinery.

That episode also connected voice to a much larger system:

- a personal agent coordinating other agents and services;
- an event feed showing background work rather than blocking on one chat turn;
- shared skills, tools, and knowledge;
- local and remote compute;
- user-defined budgets and inspectable activity; and
- an open market in which requests could discover capable providers.

Episode [`140`](../transcripts/140.md), *Open-Sourcing Onyx*, compressed the
interaction model to “you say it, Onyx does it.” The important word is **does**.
The aspiration was an agent that becomes more capable through tools and the
network, not a voice skin over question answering.

Episode [`141`](../transcripts/141.md), *One Market*, made Onyx the human
gateway into a global market of AI capabilities. Speech could become
transcription, inference, a tool call, a local/private operation, or a paid
network job. Discovery, negotiation, and payment were meant to happen behind a
simple conversational surface, while remaining open and inspectable.

Episode [`144`](../transcripts/144.md) paired Onyx with Pylon. The phone could
reach files, Git, databases, models, and tools on a trusted computer through an
open capability protocol. This established an enduring topology: the voice
client does not need to hold every capability itself; it can be a safe control
surface for powerful runtimes elsewhere.

Episode [`145`](../transcripts/145.md), *Going Local*, pushed the idea toward a
“Jarvis-y Onyx.” The user could walk around the house, talk to the phone, reach
a trusted desktop model over a persistent connection, inspect files, and
eventually direct coding agents by voice. It also introduced a choice that
still matters: route each part locally, to a trusted owner machine, to a cloud
API, or to a market provider according to privacy, latency, capability, and
cost.

Episodes [`149`](../transcripts/149.md) and
[`150`](../transcripts/150.md) sharpened the durable personal-agent idea. The
user would establish an ongoing relationship with one agent, progressively
grant it GitHub, knowledge, data, model, and market capabilities, and prefer
private/local execution where appropriate. Onyx was described as an open
“Jarvis in your pocket”: persistent and extensible, not a disposable voice
call tied to one model vendor.

### 1.3 Speak to Onyx: the conversational loop becomes real

Episode [`151`](../transcripts/151.md), *Speak to Onyx*, demonstrated fast
hosted Whisper transcription and near-instant model response. Its near-term
roadmap—memory, tools, wallet, and marketplace—again treated voice as the
entrance to growing capability rather than the destination.

Episode [`152`](../transcripts/152.md), *Code by Voice*, supplied the strongest
direct ancestor of this plan. A detailed spoken instruction selected a repo,
described a UI bug, located context through a repo map, invoked write tools,
and produced a working change. The motivation was practical: work without
typing, code away from the desk, and reduce the physical cost of constant
keyboard use. It also exposed the enduring safety model: start read-only,
enable write tools deliberately, bind work to the right repository/branch, and
inspect the result.

Episodes [`159`](../transcripts/159.md)–[`163`](../transcripts/163.md) expanded
the coding loop from one tool call into repo maps, relevant-file selection,
plans, actions, reasoning streams, tests, and repeatable issue-to-PR work.
Episode [`160`](../transcripts/160.md) imagined Onyx sending a push notification
when an agent became stuck and accepting a spoken follow-up. That is an early
version of today's attention loop: agents work asynchronously; voice lets the
human resolve the few moments that need judgment.

### 1.4 Commander: Jarvis meets the agent fleet

Episode [`170`](../transcripts/170.md), *Commander*, made the Jarvis comparison
explicit. The imagined 2030 interface was not a VS Code fork. It was closer to
StarCraft: multiple agents working in parallel, hotkeys and macros for groups,
voice and other high-level inputs, live environments and patches, and a HUD
that surfaces only what needs attention.

This adds an important dimension to the Onyx vision:

- **Onyx:** one conversational front door from anywhere;
- **Commander:** one visual command center for many concurrent agents; and
- **persistent Desktop voice:** a continuous bridge between speech and that
  visual command center.

Episode [`175`](../transcripts/175.md) grounded Commander as a local desktop app
with local inference and future agent/network surfaces. Episode
[`179`](../transcripts/179.md), *Claude Code Commander*, showed multiple coding
sessions and histories in one interface and again named voice and gestures as
the next layer of Jarvis-style interaction.

The StarCraft analogy should guide product behavior. A good commander does not
dictate shell syntax or narrate every token. They set objectives, select a
unit/session/target, redirect work, approve consequential choices, ask for a
status summary, and inspect the battlefield when something is uncertain.
Voice mode should optimize those verbs.

The spatial-HUD synthesis in
`docs/game/2026-06-16-spatial-hud-agentic-mmo-wow-direction.md` preserves the
same desired grammar: configurable panes, in-context diffs, parallel agents,
live cost, hotkeys/macros, and hand plus voice input. Voice should be able to
focus or rearrange attention, ask what changed, select a group, spawn or
interrupt work, and open the exact proposal that needs approval. It should not
make the visual cockpit disappear.

### 1.5 Later product lessons: ambient input plus durable evidence

Later episodes used tools such as Aqua Voice to dictate rich requirements into
coding agents (for example [`188`](../transcripts/188.md) and
[`248`](../transcripts/248.md)). This proved the everyday value of speech even
before OpenAgents owned the whole audio stack: spoken language is excellent for
high-bandwidth intent, corrections, and qualitative product direction.

At the same time, the product evolved toward Sync, durable conversations,
receipts, behavior contracts, agent topology, and cross-device continuation.
Those systems answer the weakness of a pure Jarvis fantasy: speech is fleeting
and ambiguous, while important work needs durable identity and evidence. The
modern opportunity is to combine the naturalness of Onyx/Commander with the
reliability program now captured in Sol.

Episodes [`183`](../transcripts/183.md) and
[`184`](../transcripts/184.md) are useful guardrails. They explored a native
voice/TTS prototype and natural control of remote work, but explicitly treated
voice as first-class rather than making the entire product voice-centric.
StarCraft-style visual updates and push notifications remained peers to
speech. Episodes [`187`](../transcripts/187.md),
[`192`](../transcripts/192.md), and [`193`](../transcripts/193.md) then added
the cross-device shape: synchronized desktop/mobile sessions, phone control of
Codex or Claude running on a desktop, visible remote logs, and the ability to
walk around while work continues. The durable work session survives whichever
screen or microphone is currently attached.

The old Pro operator-canvas audit at
`docs/pro/2026-06-24-pro-operator-ui-revival-audit.md` adds another useful
interaction pattern: natural language mounts contextual HUD panes; sensitive
actions move through dry-run, step-up authentication, approvals, and decision
traces; assistant audio supports barge-in. The current Desktop can recover that
grammar without recovering the obsolete product shell.

### 1.6 The product thesis for persistent voice

The strongest version of voice mode has five jobs:

1. **Capture intent with low friction.** Let the user think out loud, give a
   detailed brief, make a correction, or redirect a running agent without
   changing physical context.
2. **Maintain shared situational awareness.** The system can speak concise
   updates about the currently selected conversation, agent group, diff,
   blocker, approval, or outcome while the visual UI shows the full evidence.
3. **Command the fleet.** Spoken verbs select and steer exact sessions,
   children, targets, and work—not a generic assistant detached from the live
   workbench.
4. **Handle the attention loop.** Agents can ask for clarification or approval;
   the user can answer immediately, defer it, or open the relevant UI context.
5. **Learn from real work, with consent.** Retained audio and its corrections
   can eventually improve endpointing, transcription, routing, and UX, but only
   through an explicit data contract rather than invisible ambient capture.

This is closer to an operating system input mode than a chat feature. The
voice session should know the current WorkContext and visible selection, but it
must also make that binding explicit whenever ambiguity could affect authority.

## 2. Product intent

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

## 3. How the vision fits the current Sol architecture

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

### Retention decision still required

The roadmap's persona-neutral voice direction is already aligned with Onyx and
Commander: voice is a modality over real work, not a separate persona product.
One detail remains undecided. The roadmap currently rejects ambient recording,
raw-audio retention by default, and voice-only authority. Saving every audio
frame therefore needs a specific retention decision rather than being buried
inside transport implementation.

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

## 4. Existing Desktop seams to extend

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

## 5. Realtime protocol proposal

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

## 6. Storage and data model

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

## 7. Prior voice infrastructure patterns worth retaining

The recent Sarah/Hydralisk experiment is one technical reference among the
broader Onyx/Commander lineage. Its product shell is retired, but it produced
useful generic realtime lessons:

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

Avoid the old browser final-only SpeechRecognition path; it fragmented turns
and delayed barge-in. Keep sustained ASR/VAD or GPU inference away from a
latency-sensitive event loop; the prior renderer needed off-loop work,
watchdogs, and silence-passthrough protection.

The reusable quality gates are decoded-frame media truth, bounded LIVE leases,
session eviction, authoritative stop, stale-generation fencing, and text
control remaining usable when media fails.

## 8. Threat, privacy, and abuse audit

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

## 9. Reliability and observability budgets

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

## 10. Ordered delivery plan

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

## 11. Open decisions

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

## 12. Recommendation

Proceed with V0–V2 as a bounded persistent-transport program under the active
Desktop architecture, with ephemeral audio and no action execution at first.
That proves the genuinely new technical value without prematurely coupling it
to permanent surveillance, model authority, or a storage policy.

Treat retained audio as V6: an explicit opt-in data product with its own
receipts. The north star is the old Onyx promise—say what you want and useful
work happens—combined with Commander's Jarvis/StarCraft fleet surface and Sol's
durable evidence. The durable invariant is simple: voice makes the whole
OpenAgents system easier to command; it does not become an authority of its
own.
