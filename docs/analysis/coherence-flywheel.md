# Coherence flywheel

- Date: 2026-07-21
- Status: active process (owner-directed)
- Audience: agents that develop or test conversation surfaces
- Related methods: [rubric](./conversation-thread-coherence-rubric.md),
  [deterministic screening](./deterministic-coherence-screening.md),
  [ledger](./coherence-ledger.md)
- Tracking issue: [#9160](https://github.com/OpenAgentsInc/openagents/issues/9160)
- Result authority: analysis only

## Purpose

The owner direction: dogfood the Full Auto system continually. Grade the
conversations that agents create while they test and improve it.
Coherence must improve over time while the tests stay hard, including
multi-agent handoffs. This document is that process.

The flywheel is: create conversations, grade them immediately, fix the
worst defect, convert it to a fixture, and record the sweep. Every
rotation raises the floor. No rotation may lower it.

## The loop

Run one rotation for every session that changes routing, delegation,
provider handoff, or Full Auto behavior. Run a full rotation at least
weekly even without changes.

1. **Create.** Exercise the current build with the scenario matrix
   below. Real owner usage counts as created material too.
2. **Screen.** Run `pnpm run grade:coherence -- --since <window>` over
   the local corpora. Grade the session you just produced explicitly by
   passing its transcript path.
3. **Assess.** Apply the complete rubric to every `needs_review` thread
   and every tripwire hit. Producer self-scores are candidates, never
   acceptance.
4. **Fix.** Take the single worst confirmed defect first. Land the fix
   with a replayable fixture that preserves user input, typed route
   state, typed mode state, actions, and result.
5. **Record.** Append one entry to the
   [coherence ledger](./coherence-ledger.md): date, commit, aggregate
   scores, needs-review rate, defects found, fixtures added.
6. **Ratchet.** Compare with the prior entry. The mean must not fall.
   The needs-review rate must trend down across entries. A hard-fail
   gate defect blocks release evidence regardless of score.

## Scenario matrix

Keep the tests hard. A rising score against easy scenarios is not
progress. Each rotation must include scenarios from at least three rows.

| Scenario | What it stresses | Known failure class |
| --- | --- | --- |
| Identity or greeting message with delegate lanes ready | Route authority, D1, D8, gates G1 and G2 | #9159: hidden Full Auto on "hey who are you" |
| Ordinary question during a running Full Auto run | Mode isolation, D3 | Chat pressure evicting run context (#8967 origin) |
| Owner-started Full Auto run to cap | Lifecycle truth, D6, D7 | Cap settle window (`b58e2b6934`) |
| Mid-run provider handoff Codex to Claude | Multi-agent continuity, D2, D5 | Mission loss across providers (#9000) |
| Rotation on typed provider failure | Causal continuity, D2 | Misclassified failure taxonomy |
| Pause, resume, and stop during a live turn | State consistency, D6 | Stop control display drift |
| App restart during a run | Sequence truth after reload, D6 | Legacy migration defect (0.1.0 residual) |
| Delegated answer promoted to primary slot | Relevance and provenance, D4, D5 | #9159 promotion without relevance check |
| User interrupt mid-turn, then a corrected request | Recovery, D1, D7 | Ignored correction, repeated wrong work |
| Multi-hour run with many continuation turns | Long-horizon coherence, D2, D7 | Silent stall, weak continuation objective |

Add a row whenever a new confirmed defect class appears. Never remove a
row because it keeps failing.

## Grading conversations that agents create

An agent that tests a conversation surface grades its own output in the
same session:

- Claude Code sessions live under `~/.claude/projects/<project>/`.
- Codex rollouts live under `~/.codex/sessions/<yyyy>/<mm>/<dd>/`.
- Desktop delegate and Full Auto threads produce run reports and
  receipts. Their transcript analyzer
  (`full-auto-run-analyzer`) covers the run layer. The rubric covers the
  conversation layer.

Grade first, then fix. Do not edit or select transcripts to raise a
score. Deleting an unflattering conversation before a sweep is evidence
tampering and voids the ledger entry.

## Roles and honesty rules

- The producing agent screens and self-assesses. That output is a
  candidate.
- An independent session re-grades before any release or public claim,
  per the rubric and the assurance boundary.
- Metric changes (lexicons, deductions, scenario rows) land with tests
  and a ledger note. A metric change resets trend comparison at that
  entry.
- Aggregate numbers are public-safe. Raw transcripts, quotes, and local
  paths stay on the machine.

## Relation to Full Auto release work

The flywheel does not replace the #8979 release gates. It feeds them:
every confirmed gate-class defect becomes a fixture, and the fixture set
runs against each release candidate. The status picture lives in
`docs/fable/2026-07-21-full-auto-status-audit.md`. The public-facing
extension of this process is the
[public coherence benchmark spec](./public-coherence-benchmark-spec.md).
