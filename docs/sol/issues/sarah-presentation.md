# P1 PARALLEL: Sarah presentation quality — avatar, opener, and UI polish

## Priority posture

This is a dedicated parallel lane. It improves Sarah's presentation and
quality without blocking P0 Fleet Command. Text-first conversation and fleet
control remain usable throughout.

## Consolidated scope

The former OAV/SQ child issues are folded here:

1. Pre-rendered opener and semantic-cache takes through the owned renderer.
2. Repair opener defects, add the no-initialism and pricing-deflection scripts,
   and capture owner playback verdicts.
3. Complete the audio-first experiment matrix and choose one real-time and one
   offline recipe.
4. Run value-gated Ditto, SoulX-FlashHead, LatentSync, FLAIR/RIFE, and prosody
   comparisons only where they can change that decision.
5. Keep full-strength per-frame GFPGAN banned; preserve license checks and
   objective A/V gates.
6. Improve turn latency, ASR, fallback, responsive layout, Blueprint
   readability, motion, accessibility, and visual polish.
7. Preserve deploy simulators, cadence/invalid-crop fail-closed checks, and
   text-only degradation.
8. Implement a typed media admission ladder: text is the availability floor;
   pre-rendered media is opportunistic and never delays input; realtime video
   is a leased enhancement returning `available | queued | text_only |
   unavailable`; queues have bounded waits and expire to text rather than
   reserving invisible capacity.
9. Separate conversation health from media health using the FC-3 state model.
   A fresh frame/transport lease is required for LIVE; simulator lease expiry
   must produce `media=stale + conversation=text_live` and an explicit reconnect
   action, never a frozen LIVE badge.
10. Measure cost and admission truth: marginal cost per active minute, slot
    utilization, queue time, abandonment, recovery, and text fallback. Record
    honest cost classes for pre-rendered, realtime, and offline-only tiers.
11. Run paired within-owner crossover trials on fixed task classes across
    text-first, audio, realtime video, and pre-rendered-opener-plus-text.
    Measure time to correctly scoped action, time to verified outcome,
    recoveries/interventions, correct state comprehension, repeat-use
    preference after receipts, and marginal cost. Publish medians and bounded
    raw trials; do not imply population significance from tiny N.

## Pipecat disposition — extract patterns, reject foundation

The decision record is
[`docs/sarah/2026-07-09-pipecat-voice-infra-audit.md`](../../sarah/2026-07-09-pipecat-voice-infra-audit.md).
Pipecat 1.5.0 at audited upstream commit `5b75654` is a BSD-2-Clause reference
and one bounded ASR/VAD experiment, not Sarah's orchestration foundation.

Extract:

- `smart-turn-v3.2` end-of-turn inference, Whisper feature extraction, and its
  single-thread CPU-executor pattern;
- Silero VAD-segmented streaming STT with interim and exactly-once final
  transcripts, using `faster-whisper` for the owned lane;
- qualified interruption: VAD speech-start, a minimum-words/backchannel guard,
  interruption-grade audio chunks, and queue flush through hydralisk's existing
  `interrupt` control;
- per-stage STT/LLM/TTS TTFB, time-to-first-audible, end-of-turn, word-timestamp,
  and user-to-Sarah latency receipts;
- tracked task cancellation/cleanup, heartbeat, behavioral-eval scenario, and
  aiortc ICE-restart patterns where they strengthen existing contracts.

Do not replace `apps/sarah` Effect orchestration, Effect Native state/intents,
Khala inference or receipt authority, hydralisk's owned renderer/TTS, or the
current text-first media-admission floor. Do not add Daily/Pipecat Cloud as a
dependency, adopt Pipecat's avatar SaaS connectors, create an RTVI-owned client
state universe, or run the experiment in the renderer process/event loop.

## Owned voice slice order and advancement gates

1. Port smart-turn model/features with the audited upstream ref, BSD notice,
   model digest, and an explicit upgrade/re-verification rule.
2. Run owned ASR as a separate process on the existing GPU host behind
   `SARAH_ASR=browser|owned`: Silero VAD -> `faster-whisper` -> smart-turn ->
   final utterance into the existing `/sarah/api/avatar/speak` contract. Compare
   a pinned Pipecat sidecar with a native FastAPI port; Web Speech remains the
   typed rollback/fallback.
3. Once VAD is trustworthy, wire qualified barge-in to the existing interrupt
   verb. The same change must add the behavior-contract row and simulator
   oracle; VAD-qualified speech-start to interrupt acknowledgement is p95 <=
   500 ms and queued audible output stops p95 <= 750 ms.
4. Add stage TTFB/TTFA, word timestamps/caption sync, user-to-Sarah latency,
   and scripted live behavioral scenarios before choosing whether the sidecar
   stays or the same architecture is kept as a native port.

Advancement requires:

- the existing avatar voice-to-voice target remains p50 <= 800 ms, with stage
  p50/p95 rather than an aggregate-only number;
- exactly one final utterance reaches the existing conversation/brain path;
  reconnects, interim text, and fallback cannot duplicate a turn;
- renderer watchdog and simulator FPS/cadence gates stay green under concurrent
  ASR load, and sidecar failure cannot wedge or restart the renderer;
- raw mic audio is ephemeral by default; interim transcripts and metrics are
  owner/session scoped and never enter public receipts;
- text, fleet control, and browser-ASR fallback remain usable throughout; and
- a failed Pipecat experiment selects the native port or browser fallback. It
  never converts into foundation adoption by inertia.

## Decision discipline

- Human playback in motion is required; stills are insufficient.
- Offline winners do not expand real-time scope unless they fit the frame and
  cost budget.
- Every experiment either selects a production recipe, rejects a candidate, or
  closes with a could-not-prove note.
- No experiment enters the queue without naming the production decision and
  threshold it can change plus the candidate that will be removed afterward.
- Schedule the first crossover alongside the first Sarah fleet canary; do not
  wait for the media lane to declare itself ready.
- Presentation work may not delay the first multi-stream Sarah fleet burn.

## Exit

Sarah opens naturally, converses reliably, degrades cleanly, and presents the
fleet canvas clearly. One receipted production real-time recipe and one offline
recipe are selected; admission/cost telemetry and a paired crossover receipt
exist; the media-lease simulator proves stale video cannot display LIVE or
disable text/fleet control; and the remaining experiment backlog is closed
rather than left as an indefinite model ladder. The owned-ASR experiment has an
explicit accept/reject receipt, browser fallback remains typed and live,
qualified barge-in meets its behavior contract, stage-latency/TTFA receipts
exist, and renderer contention/watchdog gates pass.
