# Autopilot Voice Integration Audit

Date: 2026-03-18
Branch audited: `main`
Scope: retained MVP desktop app in this repo, relevant archived voice surfaces in `~/code/backroom`, active local GCP project state for `openagentsgemini`, and the product note in `/Users/christopherdavid/code/alpha/voice.md`.

## Executive Summary

Autopilot does not currently have a shipped voice lane.

What is real today:

- the retained desktop app has strong app-owned text runtime seams for chat, Apple FM, GPT-OSS, desktop control, and provider execution
- the GCP project `openagentsgemini` is active and already has real Cloud Run, Compute Engine, Artifact Registry, Secret Manager, and Vertex-adjacent APIs enabled
- the archived tree contains two useful voice references:
  - a local Whisper push-to-talk Rust crate
  - browser voice UI components for mic selection, speech input, voice selection, and audio playback
- `crates/codex-client` already knows about `thread/realtime/appendAudio`

What is not true today:

- `apps/autopilot-desktop` has no mic capture, no STT worker, no TTS worker, no voice state, no chat mic button, and no voice control-plane surface
- the retained local runtime seams are text-only
- the retained desktop Codex lane does not expose `thread/realtime/appendAudio`
- the active GCP project does not yet have the dedicated speech APIs enabled
- there is no retained voice credential plan for shipping desktop clients safely

Bottom line:

- voice is very addable now
- the smallest credible path is not model-native full-duplex audio
- the smallest credible path is app-owned push-to-talk STT plus spoken assistant replies, with final transcript text as the canonical chat state
- Google Cloud is a viable backend for that path, but the first implementation still needs explicit API enablement, auth rules, privacy rules, and desktop wiring

## MVP And Ownership Fit

Per `docs/MVP.md`, voice is not a core MVP gate. The MVP gate is still:

- install
- use Autopilot
- go online
- get paid
- withdraw sats

That means voice should be treated as a buy-side capability upgrade, not a new authority boundary or a new provider-market product.

Per `docs/OWNERSHIP.md`, the right owner is `apps/autopilot-desktop`:

- mic capture
- STT/TTS orchestration
- chat-pane UX
- desktop-control exposure
- app policy for privacy, logging, and retention

The wrong owners would be:

- `crates/wgpui` for app-specific voice behavior
- `crates/openagents-provider-substrate` for buy-side voice UX
- reviving archived backroom crates directly into the retained tree

The working rule should be:

- keep voice app-owned until the retained product contract stabilizes
- only extract reusable audio primitives later if there is a real second consumer

## Audit Inputs

Primary retained-tree files inspected:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/v01.md`
- `docs/headless-compute.md`
- `docs/transcripts/214.md`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/panes/chat.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/codex_lane.rs`
- `apps/autopilot-desktop/src/apple_fm_bridge.rs`
- `apps/autopilot-desktop/src/local_inference_runtime.rs`
- `apps/autopilot-desktop/src/local_runtime_capabilities.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
- `crates/codex-client/src/client.rs`
- `crates/codex-client/src/types/thread.rs`
- `crates/wgpui/Cargo.toml`
- `crates/wgpui/README.md`
- `crates/wgpui/src/bleeps.rs`

Archived files inspected for reference only:

- `/Users/christopherdavid/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/voice/Cargo.toml`
- `/Users/christopherdavid/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/voice/src/lib.rs`
- `/Users/christopherdavid/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/voice/src/audio_capture.rs`
- `/Users/christopherdavid/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/voice/src/transcriber.rs`
- `/Users/christopherdavid/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/voice/src/model_manager.rs`
- `/Users/christopherdavid/code/backroom/openagents-rust-deprecation-2026-02-11/openagents/crates/voice/src/session.rs`
- `/Users/christopherdavid/code/backroom/openagents-code-archive/2026-02-25-oa-audit-issue-2212-openagents-com-legacy/apps/openagents.com/config/ai.php`
- `/Users/christopherdavid/code/backroom/openagents-code-archive/2026-02-25-oa-audit-issue-2212-openagents-com-legacy/apps/openagents.com/resources/js/components/ai-elements/speech-input.tsx`
- `/Users/christopherdavid/code/backroom/openagents-code-archive/2026-02-25-oa-audit-issue-2212-openagents-com-legacy/apps/openagents.com/resources/js/components/ai-elements/mic-selector.tsx`
- `/Users/christopherdavid/code/backroom/openagents-code-archive/2026-02-25-oa-audit-issue-2212-openagents-com-legacy/apps/openagents.com/resources/js/components/ai-elements/voice-selector.tsx`
- `/Users/christopherdavid/code/backroom/openagents-code-archive/2026-02-25-oa-audit-issue-2212-openagents-com-legacy/apps/openagents.com/resources/js/components/ai-elements/audio-player.tsx`
- `/Users/christopherdavid/code/backroom/reference/planning/18-autopilot.md`

Local GCP commands run during this audit:

- `gcloud config get-value account`
- `gcloud config get-value project`
- `gcloud config get-value compute/region`
- `gcloud config get-value compute/zone`
- `gcloud services list --enabled --project openagentsgemini`
- `gcloud artifacts repositories list --project openagentsgemini --location us-central1`
- `gcloud compute instances list --project openagentsgemini`
- `gcloud run services list --project openagentsgemini --region us-central1`
- `gcloud run jobs list --project openagentsgemini --region us-central1`
- `gcloud iam service-accounts list --project openagentsgemini`
- `gcloud secrets list --project openagentsgemini`

## Product Intent Already Exists

The repo is already on record that voice belongs in Autopilot.

`docs/transcripts/214.md` explicitly frames the desired interface as:

- voice command support in the Autopilot interface
- a more Tony Stark / Jarvis-like experience
- voice as part of the same GUI wrapper around Codex and the local machine

So the question is not whether voice fits the product.
The question is which implementation path gets there without destabilizing the retained MVP loop.

## What Exists In The Retained Desktop App

### 1. Chat is text-only

`apps/autopilot-desktop/src/app_state.rs` defines `ChatPaneInputs` with only:

- `composer`
- `thread_search`

There is no voice input state, no selected microphone, no recording state, no speaking state, no partial transcript buffer, and no voice settings.

`apps/autopilot-desktop/src/panes/chat.rs` paints:

- the transcript
- the composer
- the send button

There is no mic button, no hold-to-talk affordance, no waveform, no live transcript preview, and no speaker/playback affordance.

### 2. The local runtime seams are text seams

`apps/autopilot-desktop/src/local_inference_runtime.rs` is app-owned and well-shaped, but it is intentionally text generation only.

The core job shape is:

- request id
- prompt string
- optional requested model
- execution params

The completion shape is:

- output string
- metrics
- provenance

That is a good seam for MVP text inference.
It is not a speech runtime seam.

`apps/autopilot-desktop/src/apple_fm_bridge.rs` is also text-generation oriented.
It supervises the Swift bridge and handles chat/text/session operations, but it does not provide speech recognition or TTS.

### 3. The desktop control plane has no voice surface

`docs/headless-compute.md`, `apps/autopilot-desktop/src/desktop_control.rs`, and `apps/autopilot-desktop/src/bin/autopilotctl.rs` expose:

- local runtime
- Apple FM
- GPT-OSS
- provider control
- wallet
- buy mode
- logs
- panes

There is no `voice status`, `voice start`, `voice stop`, `voice transcribe`, or `voice speak` control path.

That matters because the repo now treats desktop control as the operator truth surface for verification.
If voice lands, it should eventually be visible here too.

### 4. The Codex client already knows about realtime audio, but the desktop lane does not

`crates/codex-client/src/client.rs` and `crates/codex-client/src/types/thread.rs` already support:

- `thread/realtime/start`
- `thread/realtime/appendAudio`
- `thread/realtime/appendText`
- `thread/realtime/stop`

But `apps/autopilot-desktop/src/codex_lane.rs` only exposes:

- `ThreadRealtimeStart`
- `ThreadRealtimeAppendText`
- `ThreadRealtimeStop`

There is no retained `ThreadRealtimeAppendAudio` command kind in the desktop lane.

This is the single clearest “full voice comms” gap in the current app:

- the client library has the audio method
- the retained desktop lane still stops at text

### 5. There is generic audio plumbing in `wgpui`, but it is not active in the app

`crates/wgpui` has:

- an optional `audio` feature
- a rodio-backed audio engine
- a `BleepCategory::Voice`

But `apps/autopilot-desktop/Cargo.toml` currently depends on:

- `wgpui` with `default-features = false`
- `features = ["desktop"]`

So the retained desktop app does not currently enable the reusable audio lane.

This means:

- there is a generic playback substrate available
- but no current desktop voice playback is wired to it
- and the app should not assume voice output exists today

## What Exists In Backroom

### 1. Archived local Whisper prototype

The archived `voice` crate is a real local transcription prototype, not a stub.

It includes:

- `cpal` microphone capture
- `whisper-rs` transcription
- HuggingFace model download/caching
- a push-to-talk style `VoiceSession`
- event callbacks like `RecordingStarted`, `TranscriptionStarted`, `TranscriptionComplete`

That is useful reference material for:

- app state machine shape
- hold-to-talk ergonomics
- validation rules for too-short or silent audio
- background worker patterns

It is not a retained-tree drop-in for current MVP because:

- it lives in archived backroom
- it downloads Whisper models from HuggingFace, which is orthogonal to the GCP path requested here
- it would pull local-model complexity back into a repo that was intentionally pruned

Conclusion:

- use the archived `voice` crate as design reference only
- do not restore it wholesale unless explicitly asked

### 2. Archived browser voice UI components

The legacy web archive contains:

- `speech-input.tsx`
- `mic-selector.tsx`
- `voice-selector.tsx`
- `audio-player.tsx`

Those components show useful product patterns:

- Web Speech API when available
- MediaRecorder fallback when not
- explicit microphone selection
- explicit voice selection
- explicit audio playback controls

They do not prove a retained backend voice pipeline.
They are UI references, not evidence of current desktop voice support.

### 3. Legacy cloud audio/transcription did not use GCP speech

The legacy Laravel config in `config/ai.php` shows:

- `default_for_audio = openai`
- `default_for_transcription = openai`
- Gemini present as a provider, but not the default transcription/audio lane

So even the archived cloud app does not show a retained Google Cloud speech implementation.

That means the GCP voice path is effectively new work, even though the broader GCP project already exists.

### 4. Archived product planning already wanted local-first voice with cloud fallback

`/Users/christopherdavid/code/backroom/reference/planning/18-autopilot.md` describes:

- push-to-talk voice commands
- local-first transcription when policy allows
- cloud STT fallback
- confidence gating
- live HUD feedback
- destructive-action confirmation rules

That planning direction still makes sense.
The retained desktop app simply has not implemented it yet.

## Active GCP Situation

The active local gcloud context during this audit resolved to:

- project: `openagentsgemini`
- region: `us-central1`
- zone: `us-central1-a`

The project is not hypothetical.
It already has live operational surfaces.

### 1. Enabled services relevant to this question

Observed enabled services include:

- `artifactregistry.googleapis.com`
- `cloudbuild.googleapis.com`
- `compute.googleapis.com`
- `run.googleapis.com`
- `secretmanager.googleapis.com`
- `aiplatform.googleapis.com`
- `generativelanguage.googleapis.com`
- `vision.googleapis.com`
- `visionai.googleapis.com`

Not observed in the enabled list:

- `speech.googleapis.com`
- `texttospeech.googleapis.com`

That is an important finding.

The project already has enough Google Cloud footing to host a voice service.
But the dedicated speech APIs are not yet on.

### 2. Existing artifact and runtime footprint

Observed Artifact Registry repositories:

- `l402`
- `openagents-control-service`
- `openagents-nexus`
- `openagents-runtime`
- `openagents-symphony`
- `openagents-web`
- `thirdparty`

Observed Compute Engine instances:

- `nexus-mainnet-1`
- `nexus-staging-1`
- `oa-bitcoind`
- `oa-lnd`
- `symphony-mainnet-1`

Observed Cloud Run services:

- `l402-aperture`
- `l402-wallet-executor`
- `oa-convex-backend-nonprod`
- `oa-convex-dashboard-nonprod`
- `openagents-control-service-staging`
- `openagents-runtime`
- `openagents-web`
- `openagents-web-staging`

Observed Cloud Run jobs:

- `openagents-maintenance-down`
- `openagents-migrate`
- `openagents-runtime-migrate`

Operational implication:

- if a small voice proxy or gateway is needed, the project already has an established way of shipping containerized services

### 3. Existing service accounts and secrets are not voice-shaped

The current service-account inventory is centered on:

- Nexus
- Symphony
- Convex non-prod
- default compute

The current secret inventory is centered on:

- L402
- Lightning
- web app
- runtime
- non-prod backend support

There is no observed retained service account or secret naming pattern for:

- speech
- TTS
- voice
- audio
- Gemini speech
- Vertex speech

Conclusion:

- the project can host voice
- but voice has not been provisioned as a first-class surface yet

## Recommendation: What To Add Now

The right immediate decision is:

1. Phase 1 now: push-to-talk STT plus spoken assistant replies
2. Phase 2 later: realtime voice conversation and interruptibility

### Phase 1 Now

Definition:

- user presses and holds a mic control
- desktop records a short clip
- clip is sent to Google STT
- final transcript is inserted into the existing chat flow as ordinary text
- assistant responds through the existing text stack
- final assistant text is optionally synthesized through Google TTS and played back

Why this is the right first landing:

- it fits the current text-owned chat architecture
- it does not require changing provider/runtime truth
- it keeps deterministic replay centered on text, not raw audio
- it avoids blocking on the desktop Codex audio gap
- it is enough to make Autopilot feel like voice works

Recommended Google lane for Phase 1:

- STT: Google Cloud Speech-to-Text using the Chirp 3 path described in `/Users/christopherdavid/code/alpha/voice.md`
- TTS: Google Cloud Text-to-Speech using one fixed Chirp 3 HD voice first

Reason to start with fixed Cloud TTS instead of Gemini-TTS:

- lower product ambiguity
- more predictable voice identity
- fewer “style prompt” decisions during the first ship
- easier debugging when the main task is proving the lane works

`Gemini-TTS` should stay a later option once the basic loop is reliable.

### Phase 2 Later

Definition:

- streaming STT partials
- barge-in while the assistant is speaking
- incremental transcript display
- optional model-native realtime audio once desktop Codex lane supports `thread/realtime/appendAudio`

This is the right later phase because it depends on gaps that are still real:

- no desktop audio-append lane for Codex today
- no voice state machine in app state today
- no voice playback or interruption contract today

### Architecture Shape

The new implementation should stay app-owned under `apps/autopilot-desktop`.

Suggested module shape:

- `apps/autopilot-desktop/src/voice/mod.rs`
- `apps/autopilot-desktop/src/voice/state.rs`
- `apps/autopilot-desktop/src/voice/worker.rs`
- `apps/autopilot-desktop/src/voice/gcloud.rs`
- `apps/autopilot-desktop/src/voice/audio_capture.rs`
- `apps/autopilot-desktop/src/voice/audio_playback.rs`

Suggested command/update pattern:

- copy the worker style already used by `apple_fm_bridge.rs`
- keep a command queue
- keep a drained update queue
- keep side effects off the render path

Suggested app state model:

- `idle`
- `recording`
- `transcribing`
- `ready_to_submit`
- `submitting`
- `speaking`
- `failed`

Suggested canonical persisted state:

- final transcript text
- whether voice was used
- provider and latency metadata
- optional voice-id label for output voice

Suggested non-canonical transient state:

- raw PCM samples
- VAD/interim transcript chunks
- playback buffer
- waveform display cache

This matters for replay safety:

- the canonical chat state should remain text-first
- raw audio should not become required to reconstruct chat history

### Files That Should Change First

Phase 1 should touch app-owned files only:

- `apps/autopilot-desktop/src/app_state.rs`
  - add voice session state
  - add chat voice input state
  - add settings fields for enable/disable and selected voice
- `apps/autopilot-desktop/src/panes/chat.rs`
  - add mic button
  - add recording/transcribing/speaking indicators
  - add optional replay button for spoken assistant output
- `apps/autopilot-desktop/src/input.rs`
  - add pointer and keyboard handling for press-to-talk
  - add escape/cancel behavior
- `apps/autopilot-desktop/src/input/actions.rs`
  - start recording
  - stop recording
  - submit transcript
  - play/stop spoken reply
- `apps/autopilot-desktop/src/desktop_control.rs`
  - later, expose voice status and test hooks
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`
  - later, add `voice status`, `voice transcribe`, and `voice speak` commands for verification

Files that should not be changed first:

- `crates/wgpui`
- `crates/openagents-provider-substrate`
- archived backroom voice crates

### GCP Changes Required

Minimum project deltas:

1. Enable APIs:
   - `speech.googleapis.com`
   - `texttospeech.googleapis.com`
2. Decide auth mode:
   - dev-only direct ADC
   - or shipping-safe proxy
3. Add a dedicated voice service account if a proxy is used
4. Add a budget/usage guardrail for voice requests

Recommended auth split:

- internal/dev now:
  - allow direct desktop calls using ADC on trusted machines
  - use `gcloud auth application-default login` or equivalent local credential flow
- shipping/public:
  - do not embed long-lived service-account credentials in the desktop app
  - front voice calls with a Cloud Run voice gateway in `openagentsgemini`

Why a gateway is the safer ship path:

- protects GCP credentials
- centralizes budgeting and abuse controls
- gives one place to enforce clip-length and character-length caps
- makes later provider switching possible without changing desktop UX

Recommended gateway shape if you choose to add it:

- Cloud Run service, for example `openagents-voice-gateway`
- dedicated Artifact Registry repo or reuse an existing general runtime repo
- dedicated service account with only the speech/TTS roles it needs
- no persistent raw-audio storage by default
- logs redacted so request logs never capture raw audio or base64 payloads

### Privacy, Logging, And Determinism Rules

These rules should be treated as non-negotiable for the first ship:

- do not persist raw microphone audio by default
- do not write base64 audio chunks into desktop-control logs
- do not make raw audio part of replay or sync truth
- show a clear UI indicator whenever cloud voice is active
- give users an explicit way to mute spoken replies
- require an explicit opt-in if any debug audio retention is ever added

Recommended audit-safe metadata only:

- clip duration
- transcription latency
- synthesis latency
- transcript length
- selected voice label
- backend label
- success/failure reason bucket

## Why Not Restore The Archived Whisper Crate

The archived local Whisper path is useful reference, but restoring it now would be the wrong move because:

- it reintroduces model download and local speech-model lifecycle into the retained MVP repo
- it is not the GCP path requested here
- it increases local setup friction
- it competes with the cleaner “text stays canonical, speech is an edge service” shape

If offline speech becomes important later, the archived Whisper design can be revisited as:

- a fallback backend behind the same app-owned voice worker seam

It should not be the first retained implementation.

## Why Not Make “Full Voice Comms” The First Milestone

If “full voice comms” means:

- always-on mic
- streaming STT
- streaming TTS
- interruption while speaking
- model-native audio turns

then the retained repo is not one patch away from that.

The blockers are real:

- no voice state in chat
- no audio append command in the desktop Codex lane
- no current playback contract
- no cloud auth policy
- no logging/privacy contract

If “full voice comms” means:

- speak to Autopilot
- hear Autopilot answer back

then you can land that now with Phase 1.

That is the right interpretation for the next implementation step.

## Recommended Immediate Sequence

1. Enable `speech.googleapis.com` and `texttospeech.googleapis.com` in `openagentsgemini`.
2. Decide whether the first build is:
   - internal direct-to-GCP via ADC
   - or proxy-backed via Cloud Run
3. Add an app-owned voice worker in `apps/autopilot-desktop`.
4. Add a chat-pane mic button and hold-to-talk flow.
5. Submit final transcript text into the existing chat pipeline.
6. Add spoken playback of final assistant replies.
7. Add mock-backend tests so the voice state machine is deterministic without real GCP calls.
8. Add desktop-control and `autopilotctl` voice status surfaces once the lane is stable.
9. Only after that, wire streaming STT and evaluate `thread/realtime/appendAudio`.

## Recommended Follow-On Issues

- `Autopilot Voice Worker`
- `Chat Pane Push-To-Talk UI`
- `Google Cloud STT Backend`
- `Google Cloud TTS Backend`
- `Voice Privacy And Log Redaction`
- `Voice Mock Backend And Fixture Tests`
- `Desktop Control Voice Status`
- `Codex Lane Realtime AppendAudio`

## Final Recommendation

Use Google Cloud for voice now, but keep the product contract text-first.

The best immediate ship is:

- app-owned push-to-talk capture
- Google STT for transcription
- existing text chat/runtime for reasoning
- Google TTS for spoken replies
- no raw audio persistence
- no archive restoration
- no crate extraction

That path is small enough to land soon, consistent with the retained MVP architecture, and it leaves a clean upgrade path toward true realtime voice later.
