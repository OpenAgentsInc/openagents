# Coder's harness router against the oracle

Status: measured 2026-10-12 for #11461. Run it again with
`gym coder routing` (add `--json` for the versioned report).

Coder's router v0 (`crates/coder-new/src/router.rs`, question set
`route-v1`, policy `route-policy-v1`) picks the harness a chat's work runs
on: Claude Code or Coder's own loop. The report prices those picks against
the best possible choice (the oracle) and against the fixed rule, which
takes the first admitted harness (Claude Code when it is signed in).

## How it is measured

- **Terminal-Bench.** Each of the eight development tasks' instructions went
  to the gateway door as a chat's first message. The judgments and the
  policy's picks are in
  [`route-v1-tasks.json`](../../bench/terminal-bench/profiles/route-v1-tasks.json);
  a coder-new test checks that the picks are what the frozen policy makes of
  them. Claude Code stands for the `claude-code-opus` arm and the Coder loop
  for `coder-one-jevprobe3-luna`, both with three trials per task. The
  questions and the policy were never tuned on these tasks, so they are held
  out for the router.
- **Recorded turns.** Every real turn writes a routing record: System One's
  probabilities, the outcome, whether the turn's checks passed (its last
  test or check command and Coder's file checks), and whether the person
  re-asked it (the same message again, or a continuation of a turn that did
  not finish). Turns are grouped by the judged kind and size.
- **Objective.** J = cost + $0.0001 × seconds + $1 × (1 − success), as in
  the [outcome matrix](../optimization/coder-components.md#the-objective).
  Regret is a pick's J minus the oracle's.

## Results

On the eight tasks route-policy-v1 sends every task to Claude Code (the
`plan-work` step), which is also the fixed rule's pick:

| | Mean regret per task | Tasks where the pick is the oracle's |
| --- | ---: | ---: |
| Router v0 (route-policy-v1) | $0.0784 | 3 of 8 |
| Fixed rule (Claude Code first) | $0.0784 | 3 of 8 |
| Oracle over the two harnesses | $0 | 8 of 8 |

The oracle sends five tasks to the Coder loop, which passed them 3 of 3 for
a few tenths of a cent each (`build-cython-ext`, `fix-code-vulnerability`,
`fix-git`, `git-leak-recovery`, `sqlite-db-truncate`), and keeps Claude Code
on the three the loop failed at least once (`cancel-async-tasks`,
`headless-terminal`, `log-summary-date-ranges`).

Recorded real turns: 36 turns so far, too few to compare per class (the gate
needs 20 per side). Answer-class turns went to the Coder loop and all
succeeded. 141 older lines without a chat were written by test runs before
records were limited to real chats; the report leaves them out.

## Promotion gate

A router policy becomes the default only when its mean regret on held-out
tasks is below the fixed rule's, and recorded turns with at least 20 turns
on both sides don't show it worse. route-policy-v1 **holds**: on these tasks
it picks what the fixed rule picks. It differs only on answers, which it
sends to the cheaper Coder loop, and this task set has none. The next policy
version's headroom is repository work the loop passes reliably. It needs
held-out tasks beyond these eight before it can be tuned, because tuning on
them would use up their held-out status.

## Escalation at checkpoints (#11474)

Coder moves work to another harness only at a checkpoint: a turn that ended
with failed checks, an error, a usage limit or a stall, or a message asking
for the same thing again. System One (`escalate-v1`) judges whether to stop,
try again or move, and `escalate-policy-v1` decides. Coder never sends a
message the person didn't write (#11568): a retry is theirs to send, and a
move puts the briefing for the stronger harness in the input, where they can
send it or write their own. `gym coder routing`
prices the loop first, with Claude Code taking over when its checks fail,
on the three tasks the loop failed at least once:

| Task | Moves | J, loop then Claude Code | J, Claude Code alone |
| --- | ---: | ---: | ---: |
| `cancel-async-tasks` | 33% | $0.0679 | $0.1885 |
| `headless-terminal` | 33% | $0.0569 | $0.1505 |
| `log-summary-date-ranges` | 100% | $0.1009 | $0.0965 |

Loop-then-escalate wins on two of the three (mean $0.0752 against $0.1452).
It loses on `log-summary-date-ranges`, where the loop always fails and its
attempt is added cost. The replay assumes the checkpoint catches every
failure and that Claude Code after a handoff does as well as alone; neither
is measured here. On the tasks where it wins, it stays ahead while the
checkpoint misses fewer than 33% of the loop's failures. The recorded
escalation lines will show the real trigger's false accepts and the
fallback's success as they accumulate.
