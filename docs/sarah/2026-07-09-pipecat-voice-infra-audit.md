# Pipecat voice-infrastructure audit — foundation for Sarah, or pattern mine? (2026-07-09)

Owner directive: deeply evaluate Pipecat (`pipecat-ai/pipecat`, cloned
read-only at workspace `projects/repos/pipecat`, HEAD `5b75654` of
2026-07-09, release 1.5.0 of 2026-07-04) as a potential FOUNDATION for
Sarah's voice infrastructure, seam by seam against the stack that went
live today. Baseline: the owned pipeline flipped to production on
2026-07-09 (`f5f9cb3725`) — browser Web Speech ASR + `apps/sarah` Bun
server + the hydralisk TTS/avatar services on the L4 GPU host. Related
issues: #8594 (Sarah epic), #8600 (Khala-gateway brain lane, closed
today), #8610 (presentation-quality consolidation epic — owns voice
quality incl. the ASR/latency lanes), #8621 (SQ-4 renderer hardening,
closed today; follow-ups fold into #8610). Sibling docs:
`docs/sarah/2026-07-09-oav-quality-strategy.md`,
`docs/sarah/2026-07-09-owned-avatar-video-pipeline-spec.md`,
`docs/sarah/2026-07-09-liveavatar-integration-assessment.md`.

## 0. Verdict up front

- **Do not adopt Pipecat as the foundation.** Our production surface is a
  working, contract-tested, three-tier system (browser / Bun server / GPU
  host) whose hard parts — MuseTalk realtime rendering, the
  idle/listen/speak state machine, the Khala-gateway brain lane with exact
  receipts and typed fallback — are exactly the parts Pipecat does NOT
  provide. Pipecat's avatar story is four SaaS connectors (HeyGen, Tavus,
  Simli, LemonSlice); it has no local lip-sync renderer, and its Python
  asyncio pipeline would sit next to MuseTalk GPU inference — the precise
  class of event-loop contention hydralisk just finished paying for
  (`d3a8fdb`: render moved to `asyncio.to_thread`, stall watchdog,
  silence-passthrough).
- **Do extract, aggressively.** Pipecat is the best-engineered open
  reference we have seen for the two seams where we are weakest and the
  owner has already asked for improvement: (1) an owned server-side ASR
  lane (VAD-segmented streaming STT with interim+final transcripts — our
  browser ASR is final-only, `interimResults=false`), and (2) VAD +
  turn-taking (Silero ONNX with start/stop hysteresis, plus a bundled
  8.7 MB `smart-turn-v3.2` end-of-turn ONNX model, BSD-2-Clause, CPU-only)
  which directly attacks the #8600 fragment-coalescing problem. Its
  interruption architecture, TTS text-aggregation machinery, and
  TTFB/TTFA metrics vocabulary are directly portable as patterns.
- **License is clean.** BSD-2-Clause (verified in `LICENSE`: "BSD 2-Clause
  License, Copyright (c) 2024–2026, Daily"), SPDX headers on every source
  file. Daily is the steward but Daily-the-SaaS is an optional extra
  (`daily = [ "daily-python>=0.29.0,<1" ]` in `pyproject.toml`); the
  `SmallWebRTCTransport` path is pure aiortc — the same WebRTC library
  hydralisk already uses — with zero Daily dependency.
- **Smallest honest slice:** a bounded, flag-gated ASR+VAD sidecar
  experiment on the existing GPU host (own process, own event loop,
  feeding final utterances into the existing `/sarah/api/avatar/speak`
  contract), while porting the smart-turn ONNX model and the
  interruption/metrics patterns natively into hydralisk regardless of the
  experiment's outcome. Details in §6.

## 1. What Pipecat actually is

An open-source Python (>=3.11) framework for realtime voice/multimodal
agents, ~2 years old and very active (1.5.0 released 2026-07-04; HEAD
merge landed the day of this audit). Architecture, verified in source:

- **Frames** (`src/pipecat/frames/frames.py`, 2,238 lines, 142 frame
  classes): every unit of data or control is a dataclass frame —
  `InputAudioRawFrame`, `TTSAudioRawFrame`, `TranscriptionFrame` /
  `InterimTranscriptionFrame`, `LLMTextFrame`, `InterruptionFrame`,
  `UserStartedSpeakingFrame` / `VADUserStoppedSpeakingFrame`,
  `BotStartedSpeakingFrame`, `MetricsFrame`, … Frames flow DOWNSTREAM
  (input → output) or UPSTREAM (errors/acks). `SystemFrame`s bypass
  queues for urgency; `UninterruptibleFrame`s survive interruption
  flushes.
- **FrameProcessor** (`src/pipecat/processors/frame_processor.py`): the
  unit of composition. Each processor consumes frames, pushes results
  down/upstream, and owns its async tasks via a `TaskManager`
  (`self.create_task(...)` with tracked cancellation and timeouts — a
  discipline worth copying on its own).
- **Pipeline / PipelineWorker** (`src/pipecat/pipeline/`): `Pipeline`
  chains processors; `PipelineWorker` (formerly `PipelineTask`,
  deprecated 1.3.0) wraps a pipeline with source/sink processors, sends
  `StartFrame`, and adds heartbeat monitoring (`on_heartbeat_timeout`
  event new in 1.5.0). A `WorkerRunner` owns signal handling and a
  `WorkerBus` (in-process asyncio queue by default; pgmq/Redis for
  distributed workers) for multi-agent handoff and fan-out.
- The canonical voice bot is literally seven processors
  (`examples/voice/voice-google.py`):

  ```
  transport.input() → stt → user_aggregator → llm → tts →
  transport.output() → assistant_aggregator
  ```

- **Services** (`src/pipecat/services/`, 60+ providers): thin adapters
  over base classes `STTService`, `TTSService`, `LLMService` — the
  framework's real value is in those base classes, not in any one
  adapter.
- **Observability**: `observers/` (turn tracking, user↔bot latency,
  debug/LLM/metrics/transcription log observers), OpenTelemetry tracing
  (`tracing` extra), the RTVI protocol for client-visible events, a
  realtime pipeline debugger (Whisker), and a behavioral eval harness
  (`src/pipecat/evals/` — scripted user turns with latency, text, and
  LLM-judge assertions run against a live bot via `pipecat eval`).

## 2. License and commercial posture (incl. the Daily question)

- `LICENSE` is **BSD-2-Clause**, copyright Daily. No CLA-gated core, no
  open-core split visible in the repo; per-file SPDX headers throughout.
- Daily coupling is **optional in code, real in gravity**: `daily-python`
  (the Daily SaaS SDK) is one optional extra among many; LiveKit and
  Vonage transports are peers. The dev runner (`src/pipecat/runner/
  run.py`) serves a plain `POST /api/offer` SDP-exchange endpoint for
  `SmallWebRTCTransport` — the same offer/answer shape as our WHEP-style
  `offer_url` contract. Pipecat Cloud is Daily's paid hosting, but
  nothing in `src/` requires it.
- **SmallWebRTC maturity**: `transports/smallwebrtc/` is a full
  first-class transport (~1,000-line `transport.py` plus `connection.py`
  and `request_handler.py`): aiortc `RTCPeerConnection` with configurable
  ICE servers, renegotiation, ICE restart, connection-state callbacks,
  raw audio/video tracks with auto-silence padding, and app-message data
  channels. It is the default "webrtc" path in every example — clearly
  production-intended, not a demo shim. Verdict: **Pipecat is fully
  usable with zero Daily SaaS dependency.**
- SaaS-vs-self-hostable posture across the adapters that matter to us:
  Google TTS (`services/google/tts.py`, `texttospeech_v1` streaming with
  explicit Chirp 3 HD handling — the same SaaS lane as hydralisk's
  `ChirpStreamingAdapter`) and Google STT (`services/google/stt.py`,
  `speech_v2` streaming) are cloud SaaS; self-hostable STT exists
  (Whisper via `faster-whisper`/MLX in `services/whisper/stt.py`, FunASR,
  Moonshine), self-hostable TTS exists (Piper, Kokoro, XTTS), and the LLM
  seam is `BaseOpenAILLMService` with a `base_url` override
  (`services/openai/base_llm.py:139`) plus an Ollama adapter — i.e.,
  adopting Pipecat would not change our SaaS/self-host mix; it only
  re-plumbs it.
- Ecosystem gravity still matters: docs, examples, client SDKs, and the
  CLI funnel toward Daily-operated hosting. Adopting the framework means
  tracking a roadmap steered by a vendor. As a patterns source, that risk
  is zero.

## 3. Seam-by-seam comparison against the live Sarah stack

Baseline verified in-repo today: `apps/sarah/src/ui/avatar-session.ts`
(browser `SpeechRecognition`, `continuous=true`, `interimResults=false`,
final utterances POSTed to `/sarah/api/avatar/speak`; recvonly WebRTC
video+audio via the WHEP-style `offer_url`, raw-SDP POST);
`apps/sarah/src/services/owned-renderer.ts` (`speakOwnedAvatarTurn`,
brain turn through the Khala gateway per #8600 — model `openagents/khala`
with `x-openagents-demand-kind: internal` self-attribution, exact
receipts, caps, typed fallback; `toSpeakableText` markdown stripping;
`splitSpeakableSentences` sentence groups; PCM s16le 24 kHz mono chunks —
600 ms first, 1 s subsequent — to the render-service control API; spoken
greeting on mint); behavior contracts in
`apps/sarah/src/contracts/avatar-ux-contracts.ts` (greets-first,
hears-speech, slot-never-wedges) with the synthetic-prospect smoke
`apps/sarah/scripts/sarah-avatar-e2e-smoke.mjs`; and hydralisk
(`hydralisk/tts/` Chirp 3 HD streaming + CosyVoice2 clone +
`normalize.py`; `hydralisk/avatar/` MuseTalk realtime, LiveTalking-
derived idle/listen/speak state machine, aiortc egress, session
eviction/liveness, simulator deploy gate, 24 fps).

| Seam | Sarah today | Pipecat | Assessment |
| --- | --- | --- | --- |
| ASR | Browser `SpeechRecognition`, final-only (`interimResults=false`), Chrome-routed, un-owned | `STTService` base with streaming and `SegmentedSTTService` (VAD-gated segments); 20+ adapters incl. self-hostable Whisper/FunASR; interim+final transcript frames; VAD-aware reconnect; STT TTFB metrics | **Pipecat clearly ahead.** This is our named gap (owner wants owned ASR; #8610 folds the ASR lane). `SegmentedSTTService` — buffer speech between VAD start/stop, transcribe once — is the right architecture for an owned Whisper lane on the L4. |
| VAD / turn-taking | None server-side; endpointing is whatever Chrome does; fragment coalescing handled downstream per #8600 | Silero ONNX bundled (2.3 MB, CPU, dedicated single-thread executor — `audio/vad/silero.py`, `vad_analyzer.py:90`); default turn stack: `VADUserTurnStartStrategy` + `TranscriptionUserTurnStartStrategy` to open, `TurnAnalyzerUserTurnStopStrategy(LocalSmartTurnAnalyzerV3)` to close — a bundled 8.7 MB end-of-turn ONNX model over Whisper log-mel features (`audio/turn/smart_turn/local_smart_turn_v3.py`) | **Pipecat far ahead; most extractable piece.** A semantic end-of-turn model beats fixed silence timeouts and attacks fragment coalescing at the source. Model + feature code are BSD-2-Clause and framework-independent. |
| LLM adapter | `speakOwnedAvatarTurn` → `runOwnedSarahTurn` → Khala gateway (`openagents/khala`, internal demand attribution): exact receipts, caps, typed fallback (#8600, closed) | `BaseOpenAILLMService` with `base_url` override (the `openai` SDK is a core dep), 20+ vendor adapters, context aggregators, `@tool` function calling | **Ours wins for us.** Their adapter is generic plumbing with no concept of receipts, caps, or typed fallback — and the Khala gateway already IS an OpenAI-compatible-shaped seam. Nothing to gain; contracts to lose. |
| TTS streaming / sentence aggregation | `toSpeakableText` + `splitSpeakableSentences` (min 40-char groups, first audio after the first group); PCM 24 kHz chunks 600 ms/1 s; spoken-form `normalize.py` pre-synthesis on the GPU host | `TTSService` base: NLTK-backed `match_endofsentence` (`utils/string.py:125`) via `SimpleTextAggregator`, token-vs-sentence aggregation modes, per-type text transformers (our `normalize.py` hook point), output transport re-chunks all audio into 10 ms-multiple frames for interruptibility (`base_output.py:131-135`), word-timestamp plumbing, TTFB plus the new TTFA metric | **Comparable core; their edges are portable.** Our sentence-group streaming exists and is production-measured. Worth porting: TTFA (time-to-first-*audible*; Chirp/CosyVoice pad leading silence our 600 ms first-chunk budget can't currently see), 10 ms interruption-grade chunking, word timestamps for caption sync. |
| Barge-in / interruption | Turn-level only: a NEW user turn interrupts an in-flight utterance (`sendControl(..., {type:"interrupt"})`, `owned-renderer.ts:537`; `on_interrupt` in `hydralisk/avatar/state.py`). No sub-second barge-in: the browser sends only finals, so she talks over the user until Chrome finalizes | First-class: turn-start strategy calls `broadcast_interruption()` (`turns/user_turn_processor.py:194-195`) → `InterruptionFrame` flushes queued bot audio in the output transport (`base_output.py` `handle_interruptions`) and cancels TTS; `MinWordsUserTurnStartStrategy` guards against backchannel ("mm-hm") false triggers; uninterruptible frames survive the flush | **Pipecat ahead on the trigger, we already have the verb.** The missing piece is server-side detection (VAD + interim transcripts) to fire our existing `interrupt` control mid-utterance — which the ASR/VAD lane above provides. Min-words guarding is the pattern to copy with it. |
| Transport | Browser ↔ GPU host directly: WHEP-style raw-SDP offer (session ref as capability), aiortc answerer with track pairing after `setRemoteDescription` (`efb17d1`), ICE-state peer-connected detection (`77c6dc2`/`fd3a410`), keepalive-vs-warmup race fixed (`f291823`) | `SmallWebRTCTransport` on the same aiortc, plus Daily/LiveKit/Vonage, websocket, and telephony serializers (Twilio/Telnyx/Plivo/…) | **Parity for our topology; their breadth is optionality.** We already run aiortc with the battle scars paid for. `smallwebrtc/connection.py` (renegotiation, ICE restart, delayed cleanup on disconnect) is a correctness reference for renderer-hardening follow-ups, not a replacement. Telephony serializers become relevant only if Sarah gets a phone number. |
| Avatar / video out | `hydralisk/avatar/`: MuseTalk realtime on OUR L4, catalogued footage, idle/listen/speak at 24 fps, session eviction/liveness, simulator deploy gate | Video "services" are SaaS connectors only: HeyGen, Tavus, Simli, LemonSlice (`services/simli/video.py` consumes `TTSAudioRawFrame` → vendor API → video frames; `examples/video-avatar/` is six vendor files). Transports can carry raw video (`OutputImageRawFrame`, clock-paced `video_out_framerate`, `base_output.py:910`) but nothing renders a face locally | **Pipecat has nothing for us here.** The owned renderer is the moat (per the LiveAvatar assessment and the OAV program). If we ever ran Pipecat, hydralisk-avatar would slot exactly where Simli's connector sits — proof the seam is clean, and proof Pipecat replaces none of the hard work. |
| Metrics / observability | Behavior contracts + deploy smokes (greeting deadline, speak turn, slot eviction); historical Sol budgets (ack/run-ref ≤5 s, first capacity ≤15 s, first progress/blocker ≤30 s, heartbeat ≤15 s, typed delayed/stalled states; media failure must not remove text control — [archived C0–C3 analysis](https://github.com/OpenAgentsInc/backroom/blob/dec8ae52/archive/openagents-sol-docs-2026-07-12/july9/2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md)) | `MetricsFrame` with TTFB/TTFA/processing/usage/turn/smart-turn models (`metrics/metrics.py`), turn-tracking and user↔bot latency observers, OTel tracing, behavioral evals with latency assertions | **Different layers; theirs is the vocabulary ours needs.** Per-stage TTFB decomposition (STT/LLM/TTS) is what makes Sol-budget enforcement actionable per turn; Pipecat has no equivalent of our contracts/receipts layer. Port the vocabulary, keep the contracts. |

Where Pipecat would REPLACE vs SLOT IN, if adopted: it could replace the
`apps/sarah` orchestration middle (ASR→brain→TTS sequencing) and add the
ASR/VAD front; it could NOT replace hydralisk's renderer (no local
renderer exists — we would write a custom `AIService` bridging
`TTSAudioRawFrame`s to our MuseTalk control API, i.e., keep everything we
built), and it would demote hydralisk-tts to a custom `TTSService`
subclass. The realistic insertion is the other direction: Pipecat-derived
ASR+VAD as a front on the same GPU host, feeding our existing renderer
and server contracts.

## 4. The honest costs of adoption

1. **Python asyncio next to GPU inference — we already paid this bill
   once.** Pipecat is disciplined about it (Silero and smart-turn run in
   dedicated single-thread executors — `vad_analyzer.py:90`,
   `base_smart_turn.py:78`), but the pipeline, transports, and every
   service callback share one asyncio loop. hydralisk's owner-facing
   mid-utterance freeze came from exactly this class: the render loop
   died/blocked silently on sentence-gap audio, fixed by moving render to
   `asyncio.to_thread`, adding a stall watchdog thread with
   `faulthandler` dumps, and silence-passthrough (`session.py
   tick_async`/`_watchdog`, commit `d3a8fdb`). Running a second large
   asyncio framework in the same process as MuseTalk multiplies that
   surface. Any Pipecat use must be process-isolated from the renderer —
   non-negotiable.
2. **Framework opinions vs typed contracts.** Sarah's guarantees are
   behavior contracts with oracles in the test sweep, exact receipts on
   the brain lane, Sol budgets with typed stalled/delayed states, and a
   simulator deploy gate. Pipecat's guarantees are frame-flow invariants
   and heartbeats. Mapping ours onto their frame lifecycle is real work
   (e.g., `sarah.avatar_slot_never_wedges.v1` would have to survive their
   interruption/flush semantics), and Pipecat deprecates fast
   (PipelineTask→PipelineWorker at 1.3.0; InputParams→Settings churn
   across services) — every upgrade becomes a contract-re-verification
   event.
3. **Migration risk to a production surface that shipped today.** The
   stack passed e2e from the public internet this evening; #8621 closed
   with the hardening list burned down. A foundation swap re-opens every
   closed risk (ICE, eviction, greeting deadline, keepalive races) for a
   framework whose wins for us concentrate in two seams we can extract in
   days. Rewriting a working Bun/TS orchestration layer in Python also
   runs against the repo-wide Effect Native direction.
4. **Dependency mass.** Core install pulls `openai`, `nltk`, `numba`,
   `onnxruntime`, `pyloudnorm`, `resampy`, `soxr`, `Markdown`, `protobuf`
   — acceptable in a dedicated sidecar container, unwelcome inside the
   carefully staged venvs on the render host.

## 5. What to extract regardless of the adoption decision

Ranked by leverage per effort; all BSD-2-Clause-clean to study and port:

1. **`smart-turn-v3.2-cpu.onnx` + its feature/inference code**
   (`audio/turn/smart_turn/`: `_whisper_features.py`,
   `base_smart_turn.py`, `local_smart_turn_v3.py`). 8.7 MB, CPU-only,
   single-thread executor. Semantic end-of-turn decisions for the owned
   ASR lane; kills fixed-silence-timeout fragmenting.
2. **VAD-segmented STT architecture** (`services/stt_service.py`:
   streaming `STTService` + `SegmentedSTTService`, VAD-aware reconnect,
   `stt_ttfb_timeout`) married to `faster-whisper`
   (`services/whisper/stt.py`) as the owned ASR on the L4.
3. **Interruption semantics** (`broadcast_interruption()` from the
   turn-start strategy; 10 ms output chunking + buffer flush in
   `base_output.py`; `MinWordsUserTurnStartStrategy` backchannel guard)
   as the spec for wiring server-side barge-in onto hydralisk's existing
   `interrupt` control verb.
4. **TTFA metric** (new in 1.5.0: time-to-first-*audible* via short-time
   RMS `detect_speech_onset` in `pipecat.audio.utils`) — directly
   measures the leading-silence padding inside our 600 ms first-chunk
   budget, per TTS adapter.
5. **Behavioral eval harness shape** (`src/pipecat/evals/`: YAML
   scenarios of scripted turns with latency/text/function-call/LLM-judge
   assertions against a running bot) — the natural evolution of the
   synthetic-prospect smoke, and aligned with the SQ-1 lesson that stills
   and word-level STT gates are insufficient.
6. **TaskManager discipline** (tracked `create_task`/`cancel_task` with
   timeouts, cleanup on processor shutdown) and
   `smallwebrtc/connection.py` renegotiation/ICE-restart handling as
   references for renderer-hardening follow-ups under #8610.

## 6. Recommendation and smallest honest slice

**Recommendation: extract-patterns, plus one bounded runtime experiment.
Reject adopt-as-foundation.**

Mapped to the open lane structure (#8610 owns presentation/voice quality
incl. ASR and latency; #8621 is closed — hardening follow-ups land under
#8610):

- **Slice 1 — smart-turn port (no framework dependency).** Port
  smart-turn v3 (bundled ONNX + Whisper log-mel preprocessing) into a
  `hydralisk` turn module behind the existing FastAPI host: ~3 files,
  CPU-only, single-thread executor exactly as upstream does it.
- **Slice 2 — owned ASR sidecar experiment (flag-gated).** Stand up an
  ASR service as a SEPARATE process on the GPU host (own event loop —
  non-negotiable per the `d3a8fdb` scar): Silero VAD → `faster-whisper`
  segments → smart-turn end-of-turn → final utterances POSTed to the
  existing `/sarah/api/avatar/speak` contract, browser mic audio
  delivered via a sendonly track on the already-open peer connection or a
  parallel offer. Two build options, one-day spike each: (a) a native
  FastAPI port using Pipecat's segmentation architecture as reference, or
  (b) an actual `pipecat-ai[whisper,webrtc,silero]` pipeline pinned at
  1.5.0, used ONLY for VAD+STT with a custom sink processor. Option (b)
  is the honest way to evaluate the framework on our hardware; either
  way the seam contract (utterance JSON to the same endpoint) keeps it
  swappable behind `SARAH_ASR=browser|owned`, with Web Speech remaining
  the fallback and the media path untouched.
- **Slice 3 — barge-in trigger.** Once server-side VAD exists, fire the
  existing `interrupt` control on VAD-start + min-words guard while
  SPEAKING; land it with a behavior-contract entry (interrupt-stops-
  audio-within-N-ms) and a simulator assertion in the same change, per
  the behavior-contract registry rule.
- **Slice 4 — metrics vocabulary.** Add per-stage TTFB and TTFA to the
  TTS/render receipts so Sol-budget enforcement is decomposable per turn.

Exit criteria for the Slice 2 experiment: if option (b) on the L4 holds
utterance-finalization latency inside the Sol ack budget on the
synthetic-prospect smoke, with zero event-loop contention against the
renderer process (watchdog stays silent, simulator fps gate passes), it
may stay as the ASR sidecar. If not, option (a) ports the same
architecture natively and we keep the models anyway. Either outcome banks
the extraction; neither makes Pipecat the foundation.
