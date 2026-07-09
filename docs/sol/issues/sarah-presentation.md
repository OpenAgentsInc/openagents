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
rather than left as an indefinite model ladder.
