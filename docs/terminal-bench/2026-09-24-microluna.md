# Microluna against Luna-in-Codex

2026-09-24. Issue [#9585](https://github.com/OpenAgentsInc/openagents/issues/9585),
the Luna pivot's [Microluna](../coder/design/microluna.md). This is the first
comparison of Microluna, Coder One's in-process Luna executor, against Luna in
the Codex CLI, on mini-tasks and on a few Terminal-Bench 4.0 tasks.

## Summary

- **On the four mini-tasks, Microluna passed 9 of 12 and Luna-in-Codex 8 of
  12**, at 73% of Luna-in-Codex's list-price cost and 85% of its time. Both
  arms failed only `log-severity`, and every one of those nine failures was a
  CRLF line ending the grader rejects, not a wrong count. No check looks for
  line endings, so the loop's checks and Jev both called the CRLF file done.
- **The mini-handoff loop works as designed.** On `log-severity` the
  `data.date-boundaries` check contradicted a requirement after the first
  session, Jev chose `retry`, and the second session fixed the seven-day
  range; the counts were right by session two, and only the CRLF ending kept
  the grader from passing it.
- **On three TB4 tasks, Microluna scored 0 of 3.** Only `coq-block-bound`
  has a graded baseline pair, also a failure. The other tasks lack completed
  baseline pairs and do not establish equal outcomes across all three.
  Microluna ran
  bounded sessions, checked each one, and stopped rather than wandering.
- **Two failure modes the runs exposed are now fixed** (commit
  `76c04cfa00`): a session that made no edit could still end the loop as
  done, and a repair session that lost its provider to a broken stream got no
  retry. Evidence is now required before a done, and a transient transport
  failure is resent.

The Luna spend across every run in this document is about $0.11, well under
the $10 budget.

## The arms

All three run on GPT-6 Luna, Codex CLI 0.155.1's model, on the operator's
ChatGPT-account login.

| Arm | What runs |
| --- | --- |
| `codex` | Luna in the Codex CLI, one session, as `coder-one minitask run --executor codex --model gpt-6-luna`. The Luna-in-Codex baseline. |
| `microluna` | Coder One's Microluna executor, the mini-handoff loop ([`microluna-v1.json`](../../crates/coder-one/policies/microluna-v1.json)): the requirement map in up to four groups, short sessions per group, `verify.checks` and a Jev move between them. |
| `microluna-verdict` | The same, plus the combined verdict (`checks::verdict`, [#9584](https://github.com/OpenAgentsInc/openagents/issues/9584)) as a handoff signal after each session. It landed mid-run, so it has one full pass over the four tasks. |

Each arm ran three attempts on each of the four mini-tasks, interleaved by
attempt, with live Jev. Cost is a list-price estimate: Luna at $0.10 per
million uncached input tokens, $0.01 cached, and $0.50 output, the rates in
`crates/coder-one/src/delegate.rs`; Jev at $0.042 per million input tokens
(`jev-1.13.0`), output free. Codex reports no cost, so the Luna figure is an
estimate for every arm.

## Mini-task results

Passes over three attempts, mean agent time, mean Luna cost, mean Jev cost,
and mean sessions per attempt.

| Task | `codex` (Luna-in-Codex) | `microluna` | `microluna-verdict` |
| --- | --- | --- | --- |
| `log-severity` | 0/3 · 61 s · $0.0028 + Jev $0.0002 · 1.0 | 0/3 · 67 s · $0.0030 + Jev $0.0004 · 3.0 | 0/3 · 85 s · $0.0036 + Jev $0.0005 · 3.3 |
| `interactive-terminal` | 3/3 · 184 s · $0.0092 + Jev $0.0004 · 1.0 | 3/3 · 126 s · $0.0046 + Jev $0.0005 · 3.0 | 3/3 · 135 s · $0.0045 + Jev $0.0005 · 3.3 |
| `cancel-cleanup` | 2/3 · 90 s · $0.0032 + Jev $0.0003 · 1.0 | 3/3 · 94 s · $0.0039 + Jev $0.0004 · 3.7 | 3/3 · 93 s · $0.0039 + Jev $0.0005 · 4.0 |
| `git-recovery` | 3/3 · 19 s · $0.0013 + Jev $0.0001 · 1.0 | 3/3 · 15 s · $0.0006 + Jev $0.0001 · 1.0 | 3/3 · 12 s · $0.0005 + Jev $0.0001 · 1.0 |
| **Total** | **8/12** · 1064 s · Luna $0.0495 + Jev $0.0028 | **9/12** · 906 s · Luna $0.0361 + Jev $0.0043 | **9/12** · 974 s · Luna $0.0378 + Jev $0.0045 |

The difference is one task, `cancel-cleanup`, where Luna-in-Codex failed one
attempt (0 of 2 started tasks cleaned up before the call returned) and both
Microluna arms passed all three. The sample is small; this isn't a
significant pass difference. The observed differences are cost and time:
Microluna spent 73% of Luna-in-Codex's Luna list price and 85% of its time,
because its sessions are short and cache the shared prefix, where the Codex
CLI runs one long session and resends a growing context.

Including Jev, the totals are $0.0404 against $0.0523, about **23% less
estimated total cost**, rather than the 27% reduction in Luna charges alone.
The traces suggest that shorter sessions and caching contribute, but this
comparison does not isolate their individual effects.

### The `log-severity` failures are all CRLF

Every `log-severity` failure in every arm — nine of nine — wrote a
`summary.csv` with the right counts but CRLF (`\r\n`) line endings. The
grader compares lines exactly, and its lines have no carriage return. The
mini-task's own scenario checks (`data.message-severity`,
`data.date-boundaries`) pass on the CRLF file because they parse the counts,
and the combined verdict reads the report as done. This is the pivot's
"checks don't tell the truth yet" problem in miniature: the checks and the
grader disagree, and nothing in the loop looks for the line ending the grader
cares about. It's a check to add ([#9584](https://github.com/OpenAgentsInc/openagents/issues/9584)),
not a Microluna bug.

### The handoff loop, on one run

`log-severity`, the `microluna` arm, the run that passed the counts by
session two (from
[`microluna.md`](../coder/design/microluna.md#how-to-watch-microluna)):

```text
session 1 (R1, R2, R3) done · 6 turns · wrote summarize.py, checked it
  checks ▸ R3 contradicted: data.date-boundaries, total counts null
  handoff ▸ Jev chose retry → session 2 on R1, R2, R3
session 2 (R1, R2, R3) done · 6 turns · fixed the seven-day range
  checks ▸ R1, R2, R3 observed
  handoff ▸ Jev chose next → session 3 on R4, R5, R6
session 3 (R4, R5, R6) done · 5 turns · next
session 4 (R7, R8, R9) done · 4 turns · next → closing checks
```

The counts were correct after session two; the grader still rejected the run
for CRLF.

## Terminal-Bench results

Three TB4 tasks from [#9583](https://github.com/OpenAgentsInc/openagents/issues/9583)'s
list, on the Codex-only `coder-one-microluna-v1` arm — no Claude credential,
one attempt each, matched to the baseline's tasks. The artifact is
`coder-one 0.1.0 (74936255bc)`.

| Task | Microluna (`microluna-v1`) | Luna-in-Codex (#9583 baseline, attempt 1) | Luna + Jev (#9583, attempt 1) |
| --- | --- | --- | --- |
| `coq-block-bound` | 0 · 347 s · Luna $0.0078 + Jev $0.0008 · 6 sessions | 0 · 447 s · — | 0 · 94 s · $0.0049 |
| `shadow-relay` | 0 · 826 s · Luna $0.0334 + Jev $0.0012 · 6 sessions | Not run | Not run |
| `uefi-bootkit` | 0 · 1340 s · Luna $0.0524 + Jev $0.0012 · 6 sessions | Unfinished when the baseline stopped | Infrastructure loss: disk full |

Every Microluna trial in this table scored 0. The completed
[#9583 baseline report](2026-09-24-luna-tb4-baseline.md) records 0/23 graded
attempts across both arms; the original 0/20 count was an interim subset.
That result does not establish that the model cannot solve these tasks.
Microluna's loop ran its sessions, checked each, chose retry on
contradicted requirements, and stopped at its bounds. The baseline's cost on
`coq-block-bound` is a dash because that trial reported no cost; the
Luna-plus-Jev arm's cost is its `usage.json` figure.

The `uefi-bootkit` run also exposed the fixed transport bug: its sixth
session lost its provider to a broken stream and the loop ended early. With
the retry from `76c04cfa00`, that session would have resent the request.

## What the runs changed

Two fixes landed from these runs and from #9586's strategy fingerprints,
which found that the one Microluna TB4 run ran six sessions that each ended
without an edit while the loop still called the task done:

- **Evidence before done.** A session's move ends the loop or advances past
  the last group only when the session edited a file and ran a command after
  its last edit (`executor.microluna.require_evidence`, on by default).
- **Read before editing.** `executor.microluna.read_first` runs the first
  session of each group read-only — reproduce and run the task's tests —
  before any edit. It's off in `microluna-v1` and on in
  [`microluna-v2.json`](../../crates/coder-one/policies/microluna-v2.json),
  for the next matched measurement.
- **Retry a broken stream.** A transient transport failure is resent.

## Evidence

- Mini-task runs: `~/.openagents/coder-one/minitasks-9585/`.
- TB4 trials: `~/.openagents/terminal-bench/jobs/tb4--microluna-v1--*--9585-r1/`.
- The #9583 baseline: `~/.openagents/terminal-bench/experiments/luna-tb4-9583/`.
