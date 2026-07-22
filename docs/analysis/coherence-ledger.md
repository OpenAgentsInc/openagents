# Coherence ledger

Append-only record of coherence sweeps per the
[flywheel process](./coherence-flywheel.md). One entry per sweep.
Tracking issue: [#9160](https://github.com/OpenAgentsInc/openagents/issues/9160).
Aggregates only. No transcripts, quotes, or private paths.

## Entry 1 — 2026-07-21 (baseline)

- Commit context: grader introduced (metric `coherence-screen-v1`).
- Command: `pnpm run grade:coherence`
- Scope: full local corpora, 1,397 conversations graded, 267 skipped.

| Source | Graded | Mean | Median | Needs review |
| --- | ---: | ---: | ---: | ---: |
| Codex CLI | 1,253 | 90.6 | 100 | 181 (14.4%) |
| Claude Code | 144 | 90.6 | 100 | 21 (14.6%) |

- Signals: Codex 1,014 profanity turns, 68 correction turns, 371
  interrupts. Claude Code 365, 21, 71.
- Known open gate-class defect: #9159 (hidden Full Auto in ordinary
  chat). The screen cannot detect it without user complaint. Its fix
  must land with a replayable fixture.
- Defects fixed this sweep: none (baseline only).
- Fixtures added this sweep: none (baseline only).
- Ratchet target for the next entry: needs-review rate below this
  baseline on the same window, and one #9159-class fixture in the
  fixture set.

## Entry 2 — 2026-07-22 (issue #9159 correction)

- Commit context: issue #9159 correction, metric `coherence-rubric-v1`.
- Command: the focused programmatic command in the
  [validation record](./2026-07-22-conversation-coherence-programmatic-validation.md).
- Scope: two corrected Desktop fixtures and one source-defect fixture.

| Fixture | Score | Grade | Disposition |
| --- | ---: | ---: | --- |
| Source identity thread | 9 | F | fail |
| Corrected identity thread | 100 | A | pass |
| Corrected delegated route | 100 | A | pass |

- The corrected fixture mean is 100.
- No corrected fixture needs review.
- The values are complete rubric scores. Do not compare them with the
  `coherence-screen-v1` corpus mean.
- Defects fixed this sweep: #9159 hidden Full Auto, forced routing,
  unrelated answer promotion, and late route presentation.
- Fixtures added this sweep: exact identity with all delegates ready,
  ordinary delegation authority, explicit Full Auto authority,
  unrelated delegate result, and delegated route order.
- Ratchet target for the next entry: run these fixtures through the
  programmatic real-provider control surface in #9161.

## Metric note — 2026-07-22 (`coherence-screen-v2`)

The screen now also computes a deterministic complexity score and tier per
conversation ([complexity rubric](./complexity-rubric.md)) and reports
complexity-weighted coherence in aggregates. Coherence deductions are
unchanged, so per-conversation coherence scores stay comparable with
entry 1. Trend comparison for the new complexity fields starts at the next
entry.
