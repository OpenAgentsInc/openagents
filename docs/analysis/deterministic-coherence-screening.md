# Deterministic coherence screening

- Date: 2026-07-21
- Status: active analysis method
- Audience: agents, product reviewers, and test authors
- Related method: [conversation thread coherence rubric](./conversation-thread-coherence-rubric.md)
- Tracking issue: [#9160](https://github.com/OpenAgentsInc/openagents/issues/9160)
- Result authority: analysis only

## Purpose

The coherence rubric needs evidence review or a model grader for its
semantic dimensions. That is slow. This screen is the fast, repeatable,
machine-checkable layer. It runs in seconds over thousands of local
conversations. It produces a numeric screening score per conversation.

The screening score is a proxy, not the rubric score. A high screening
score does not prove coherence. A low screening score is a strong signal
that the thread needs the complete rubric assessment. The screen exists
so agents can hill-climb: the score must trend up across builds, and a
regression must block before release evidence forms.

## The tool

The grader lives at `scripts/grade-conversation-coherence.ts` with its
pure core in `scripts/coherence-core.ts` and tests in
`scripts/grade-conversation-coherence.test.ts`.

```sh
pnpm run grade:coherence
pnpm run grade:coherence -- --since 2026-07-14 --worst 20
pnpm run grade:coherence -- --json /tmp/coherence.json --evidence
pnpm run grade:coherence -- <path-to-transcript.jsonl>
pnpm run test:coherence
```

It parses two local transcript formats:

- Codex CLI rollouts under `~/.codex/sessions` (`event_msg` lines:
  `user_message`, `agent_message`, `turn_aborted`, `patch_apply_end`).
- Claude Code sessions under `~/.claude/projects` (`user` and
  `assistant` lines with content blocks). Subagent sidechains and
  harness-injected lines are excluded.

Output stays on the local machine. Raw conversation content never leaves
the machine. The `--json` report contains signal kinds and counts only,
unless the operator passes `--evidence` for local diagnosis.

## The metric

Each conversation with at least one full user and assistant exchange
starts at 100 points. The screen deducts points for user-visible
frustration signals. This follows the owner directive: take off points
when the user swears or corrects the agent.

| Signal | Detection | Deduction | Cap |
| --- | --- | --- | --- |
| Profanity | Lexicon match in a user message after the first turn. | 15 per turn | 45 |
| Correction | Phrase match such as "no, you", "not what I asked", "you did this wrong", "still broken". | 10 per turn | 40 |
| Interrupt | `turn_aborted` events or `[Request interrupted by user]` markers. | 5 each | 20 |

Grade bands mirror the rubric: A at 90 or more, B at 80, C at 70, D at
50, F below 50. Disposition is `screening_pass` at 80 or more, otherwise
`needs_review`. A `needs_review` thread gets the complete rubric
assessment, including the hard-fail gates.

Known limits, accepted deliberately:

- Quoted or pasted text can false-positive the lexicons.
- A silent bad answer with a polite user produces no deduction.
- The screen cannot detect the rubric gates G1 through G6. The #9159
  defect thread would screen clean if the user never complained. The
  rubric tripwires and replayable fixtures cover that class.

## Baseline: 2026-07-21

First full run over the owner's local corpora (1,397 conversations
graded, 267 skipped without a full exchange):

| Source | Graded | Mean | Median | A | B | C | D | F | Needs review |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Codex CLI | 1,253 | 90.6 | 100 | 932 | 140 | 29 | 94 | 58 | 181 (14.4%) |
| Claude Code | 144 | 90.6 | 100 | 114 | 9 | 1 | 11 | 9 | 21 (14.6%) |

Signal totals: Codex carried 1,014 profanity-flagged turns, 68
correction-flagged turns, and 371 interrupts. Claude Code carried 365,
21, and 71. The two toolchains screen at the same mean, so the current
frustration rate is a property of the work, not one toolchain.

The number to hill-climb is the `needs_review` rate. Roughly one in
seven local conversations shows user frustration. The target is a
falling trend per week and per build, recorded in the
[coherence ledger](./coherence-ledger.md).

## Evolution path

1. **Now.** Deterministic screen on every routing, delegation, or Full
   Auto change. Ledger entry per sweep.
2. **Next.** A model grader that applies the rubric dimensions D1
   through D8 and the gates to every `needs_review` thread, producing
   the rubric report format. Producer self-scores stay candidates.
3. **Then.** Replayable conversation fixtures for every confirmed
   defect, run against each candidate build, per the rubric's product
   section. The public benchmark
   ([spec](./public-coherence-benchmark-spec.md)) consumes the same
   records.

Tune the lexicons in `scripts/coherence-core.ts` with a test for each
change. Do not weaken a deduction to make a build look better. That is a
metric change and needs a ledger note with a reason.
