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
