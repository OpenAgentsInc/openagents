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

## Entry 2 — 2026-07-22 (created conversations, seven live lanes)

- Commit context: metric `coherence-screen-v2`, graded through the live
  harness program (openagents#9161, ai repo through `0ab4eba`).
- Scope: 12 created conversations across the program, graded at creation.

| Conversation | Lanes | Coherence | Complexity |
| --- | --- | ---: | --- |
| Codex basic identity | 1 | 100/A | C0(0) |
| Codex sub-agent mini-project | 1 (+2 sub-agents) | 100/A | C3(63) |
| Multi-harness parallel | 2 | 100/A | C1(24) |
| Multi-harness sequential handoff | 2 | 100/A | C2(27) |
| Multi-turn continuity (7 lanes, one each) | 1 each | 90/A each | C0-C1 |
| Seven-lane orchestrated mini-project | 7 | 100/A | C4(89) |

- The correction signal fired at exactly the designed deduction on all
  seven multi-turn conversations. The seven-lane conversation is the
  first C4 datapoint: full coherence at heavy orchestration.
- Defects found by the live runs and fixed upstream: Claude projection
  totality, Codex resume flag order, ACP tool-id charset violations,
  unanswered permission requests, and advertised-but-unimplemented
  client capabilities.
- Fixtures added: env-gated live smokes per lane plus the seven-lane
  scenario (`ai` repo `src/*.live.test.ts`).
- Ratchet versus entry 1: complexity coverage rose from none to C4 with
  coherence held at 90-100. The needs-review sweep over the local
  corpora was not re-run this entry.

## Entry 3 — 2026-07-22 (first full-corpora sweep under coherence-screen-v2)

- Command: `pnpm run grade:coherence`
- Scope: full local corpora, 1,439 conversations graded, 267 skipped.

| Source | Graded | Mean | Complexity mean | Tiers C2+ | Complexity-weighted coherence |
| --- | ---: | ---: | ---: | ---: | ---: |
| Codex CLI | 1,279 | 90.8 | 56.1 | 1,167 | 87.2 |
| Claude Code | 160 | 91.2 | 43.6 | 102 | 81.1 |

- First trend point for the v2 complexity fields. The headline signal:
  complexity-weighted coherence sits BELOW the raw mean on both
  toolchains (87.2 versus 90.8, 81.1 versus 91.2). Frustration
  concentrates in complex conversations, which is exactly the failure
  band the metric exists to expose and the flywheel must attack.
- Ratchet targets for the next sweep: raise complexity-weighted
  coherence on both sources, and close the Claude Code gap (81.1) —
  its C4 conversations carry most of the deductions.
