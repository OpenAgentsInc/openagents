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

## Decision discipline

- Human playback in motion is required; stills are insufficient.
- Offline winners do not expand real-time scope unless they fit the frame and
  cost budget.
- Every experiment either selects a production recipe, rejects a candidate, or
  closes with a could-not-prove note.
- Presentation work may not delay the first multi-stream Sarah fleet burn.

## Exit

Sarah opens naturally, converses reliably, degrades cleanly, and presents the
fleet canvas clearly. One receipted production real-time recipe and one offline
recipe are selected; the remaining experiment backlog is closed rather than
left as an indefinite model ladder.
