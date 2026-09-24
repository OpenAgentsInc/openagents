# Terminal-Bench status

Updated on 2026-09-23 with a completed matched-executor pilot, the `coderos`
trace audit, and public Harbor comparisons. This page summarizes retained
evidence; it does not report the execution host's live queue.

## Latest status

**Strategy fingerprints: Fable's winners read longer before their first
edit and edit in more rounds than Luna.** Every step of 505 trajectories
on the 14-task Luna baseline subset is placed in a phase, by rules for 66%
of steps and by Jev for the rest, for $0.81 of Jev in total. Fable's
winners make their first edit later than Coder One's executor in 10 of 11
tasks (5.7 minutes against 2.1) and later than Luna in 7 of 10 (5.1
against 2.3). They edit in more rounds than Luna in 8 of 11 tasks, and test
more per edit than Fable's losers in 8 of 11. Retries, written plans, and a
final check don't separate winners from losers. These are candidate moves
for Luna, not measured gains, and Luna has 20 graded attempts so far. See
the [ranked moves](2026-09-24-strategy-fingerprints.md) (issue
[#9586](https://github.com/OpenAgentsInc/openagents/issues/9586)).

**Truthful checks: a verdict from the final report catches almost four
times the failures today's checks catch.** Every graded Coder One trial
with a composition record, 317 on 58 tasks, is labeled with its verifier
reward and split by task. On the 32 held-out tasks, today's checks
caught 6 of 60 failures (10%, 5–20%) at 55% precision. A verdict fitted
on the other half catches 22 of 60 (37%, 26–49%) at 59% precision
(43–74%), exact McNemar p = 0.0009. It uses three things: Jev's answers
to two questions over the executor's final report, and the self-report
detector. No scenario kind, requirement state, or support state separates
passes from failures on both halves. The precision gain isn't
significant, and Luna is barely measured. See the
[results](2026-09-24-truthful-checks.md) (issue
[#9584](https://github.com/OpenAgentsInc/openagents/issues/9584)).

**Tunable v10 against v7 on four near-miss tasks: equal passes, 23% less
cost, no pass from persistence.** v10 judges persistence rounds against
what the checks flag and runs them on GPT-6 Sol. Each arm passed 2 of 4
first attempts (15–85%), with every task tied (exact McNemar p = 1), so the
operator's rule stopped the experiment after attempt 1. v10 cost $4.43 an
attempt against $5.74 and spent 75% less on persistence, but no round
resolved a flagged failure, and the checks it keys on don't separate passes
from failures. See the [results](2026-09-24-persist-v10.md) (issue
[#9570](https://github.com/OpenAgentsInc/openagents/issues/9570)).

**Escalation to GPT-6 Astra on a failed check: 12 escalations, no
rescue.** Tunable v9-escalate runs Codex on GPT-6 Astra only when a check
fails or the executor reports a failure. Over 31 graded trials on eight
tasks, it escalated 12 times and rescued none: 0 of 12 (95% Wilson
0–24%), for $20.40 of Astra. The self-report trigger kept Astra's
candidate 8 times because Astra didn't admit a failure, and all 8 failed.
The check trigger fired on a false alarm in 2 of 4 trials. The gate never
saw 8 of the 20 failed trials. See the
[results and what the gate should key on instead](2026-09-24-escalation-on-failed-check.md)
(issue [#9571](https://github.com/OpenAgentsInc/openagents/issues/9571)).

**Per-task effort on six TB4 tasks: routing missed the cost bar and one
task.** Tunable v9 picks medium or xhigh per task from Jev's features,
fitted on the unused task pool. It passed 8 of 15 graded attempts (30–75%),
against fixed xhigh's 10 of 14 (45–88%) and fixed medium's 7 of 15
(25–70%), for 76% of fixed xhigh's cost, above the 60% ceiling. It ran
`gsea-proteomics` at medium and failed it 3 of 3, where xhigh passed 2 of 3,
and it raised two tasks that medium already passes. Stopped at 44 of 54
attempts once no winner was possible. See the
[results](2026-09-24-effort-routing.md) (issue
[#9569](https://github.com/OpenAgentsInc/openagents/issues/9569)).

**Version arc through v10: configuration pays, the controller doesn't yet.**
The Jev briefing, the lean executor, and effort produced every measured
saving. Across 81 graded trials in the running experiments, final checks
that all passed preceded 19 passes and 19 failures, and escalation fired 12
times without recovering a failure. See the
[version arc and ranked next experiments](2026-09-24-version-arc.md).

**Tunable v8 persistence on three near-miss tasks: cheaper rounds, no
credited pass.** Over three attempts per task, `production-planning` passed 2
of 3 and `bun-sourcemap-leak` 1 of 3, and `cargo-flight-dispatch` passed 0
of 3. The guard kept the `production-planning` passes by putting back a
persistence round that made a check fail. The executor's own tests passed in
every round, so no round showed progress through them. Persistence cost $2.63
a trial and the whole trial $6.53, against v6's $8.26 mean on other tasks. See the
[results](2026-09-23-persist-v8.md) (issue
[#9570](https://github.com/OpenAgentsInc/openagents/issues/9570)).

**Matched controller test on 10 TB4 tasks: no measurable pass-rate gain, 68%
more cost.** With Claude Code on Opus 5.5 at medium effort in both arms,
Coder One's v8 controller passed 18 of 30 attempts (42–75%) and the plain
executor 15 of 30 (33–67%); the paired difference isn't distinguishable
from chance (exact McNemar p = 0.51). Coder One cost $45.42 against $27.04
and took 2.2 times the agent time. Persistence rounds produced the one
large win, `mvcc-lsm-compaction` at 3 of 3 against 0 of 3, and most of the
extra cost; checks, repair, and escalation were nearly idle. See the
[results, attribution, and traces](2026-09-23-matched-controller-targeted.md)
(issue [#9567](https://github.com/OpenAgentsInc/openagents/issues/9567)).

**Matched executor controls: 12 attempts completed.** With the same Opus 5.5,
medium effort, six tools, system prompt, cache policy, and outer budgets,
plain Claude passed 6/6 for $6.64 and 37.0 agent-minutes; Coder passed 5/6
for $6.14 and 34.4 minutes, including Jev. That is 7.6% less usage cost and
7.2% less time with one fewer pass, so it does not establish an efficiency
win at equal success. One plain grade was recovered from unchanged output
after a collection error; the sensitivity result excludes that entire pair.
See the [full analysis, protocol, and traces](2026-09-23-matched-opus-controller.md).
These two-task development runs are separate from the TB4 suite below.

**The highlighted wins now have retained traces.** The
[revised task-win analysis](2026-09-23-task-win-analysis.md) audits 20 local
trials and seven public failures. Opus 5.5 produced all ten selected
Coder solutions. The traces explain specific numerical, interface, and
lifecycle differences, but do not establish that Jev's repairs caused
the wins. They also expose wrong-path checks and missed semantic failures.

**TB4 results need a quota audit.** Incident note `4b6c619770` reports 21 attempts
that hit Claude's usage limit but were graded anyway. It reports 19 moved
and rerun without reconciling their replacements or the remaining two.
The published scoreboard predates that note. Its totals below are a
historical snapshot, not corrected pass rates.

| TB4 arm | Passes / graded, usage-limited excluded | Mean cost per graded trial | Status |
| --- | ---: | ---: | --- |
| Coder One tunable v2 | 19/45 | $1.42 | First pass nearly complete; requeued trials rerunning. |
| Coder One tunable v3 | 5/7 | $5.79 | Effort-sensitive tasks only. |
| Coder One tunable v4 | 3/6 | $5.34 | Tasks v2 failed that some row solved. |
| Coder One tunable v5 | 1/3 | $8.90 | Includes `cargo-flight-dispatch` at 25/27 tests, a task no row has solved. |
| Coder One tunable v6 | 0/0 | — | Running on all 66 tasks, GPU tasks included. |
| Claude Code / Opus 5.5 | 10/27 | $2.28 | Same host and model as Coder One. |

Counts come from `tools/tb4_scoreboard.py`, which leaves out every trial
whose Claude or Codex session hit a usage or rate limit (#9564), so they
are reconciled for the incident above.

**Same host, same model, same tasks.** On the 26 TB4 tasks with a valid
trial on both sides, Coder One v2 passed 11 for $32.87 and 182 minutes of
agent time, and Claude Code on Opus 5.5 passed 9 for $59.27 and 295
minutes: two more passes for 45% less money and 38% less time. Coder One
was cheaper on 24 of the 26 tasks. The cost result is strong; the accuracy
difference is two tasks, one of them plausibly chance. Two trials are
excluded, one per side, because they didn't measure the agent. See the
[full assessment](2026-09-23-coder-one-vs-claude-code-tb4.md).

**Against the leaderboard on v2's 45 tasks**, matched per task: GPT-6 Astra
at max expects 23.6 passes at $6.99 a trial, and Fable 5.1 at max 24.0 at
$13.17. Coder One v2 passed 19 at $1.42: about 80% of the top rows'
accuracy at a fifth to a ninth of their cost.

See the [full 66-task matrix and corrected comparisons](tb4-results.md)
and [incident record and reconciliation requirements](data-quality.md).
The arms cover different task sets. The previous cost comparison used
full-suite leaderboard averages against a subset; matching the 36 v2
tasks changes reference costs to $6.74 and $13.38 per trial, from $9.90
and $18.92. Claims about relative capability await the quota audit.

**Eight-task development results remain promising, with limits:**

- **Luna plus the coverage packer: 24/24.** Recomputed cost is a **$0.1504
  lower bound**, with four unpriced dispatches; agent time is 385.0 s.
  Both figures sum eight per-task means, not all 24 trials. The prior
  “86% cheaper” claim is not established with missing charges.
- **Turning off monitor stops: 22/24.** Git recovery and vulnerability
  failures escaped the checks. Better check coverage remains the next
  requirement for reliable Luna-first execution.
- **Tunable Opus: 24/24**, $0.5219 and 253.3 s, versus direct Opus's
  $1.0863 and 306.4 s on the same basis. Routing selected Opus every time.
- **Coverage packing on log summaries: 3/3**, versus the older Jev-probe
  v3's 0/3. It preserves data records that the old briefing dropped.

Details and evidence: [tunable results](2026-09-23-tunable-results.md).
These are small development screens, not held-out benchmark estimates.

**Harness:** TB4 has 66 pinned tasks; CPU and RTX 4080 oracle checks pass.
`fp8-rmsnorm-gemm` requires an H100 and cannot run on the current Linux
host. A usage-limited trial is now its own outcome: Coder One exits 6,
the harness withholds the reward, the scheduler requeues the trial and
pauses the provider until the limit resets, and a host-wide cap runs at
most two Claude trials at once
([#9564](https://github.com/OpenAgentsInc/openagents/issues/9564);
[runbook](runbook.md#schedule-a-suite)). The trials graded before that
still need the quota audit. See [harness validation](tb4-results.md#harness-validation-and-remaining-coverage).

## Results and analysis

| Document | Contents |
| --- | --- |
| [Strategy fingerprints, Fable against Luna and Coder One](2026-09-24-strategy-fingerprints.md) | Every step of 505 trajectories on the Luna baseline subset placed in a phase, per-trajectory fingerprints, a worked Fable-against-Luna example, the candidate moves ranked with task counts, effect sizes, and run and step citations, what didn't separate winners from losers, and the Jev cost. |
| [Truthful checks, calibrated against graded runs](2026-09-24-truthful-checks.md) | The 317-trial label set split by task, every check signal's fail precision, failure recall, and pass rate with Wilson intervals, the combined verdict's held-out numbers against today's checks, the signals it drops, and what labeled data would close the issue. |
| [Tunable v10 against v7 on four near-miss tasks](2026-09-24-persist-v10.md) | Persistence judged against what the checks flag, on GPT-6 Sol, against the matched v7 arm: first attempts on four tasks, cost and rounds per attempt, persistence attribution, and why the checks can't key progress. |
| [Per-task effort on six TB4 tasks](2026-09-24-effort-routing.md) | Tunable v9's effort routing against fixed medium (v2) and fixed xhigh (v3): the pool fit, Wilson intervals, the exact McNemar test, cost and time per arm and task, why `gsea-proteomics` was routed to medium, and every attempt. |
| [Coder One's version arc](2026-09-24-version-arc.md) | Every version from the Gemini loop to tunable v10: what each changed, its hypothesis, tasks, passes, cost, and verdict; the strategies over time; what improves and what's stuck; measurement lessons; and the ranked next experiments. |
| [Matched controller test on 10 TB4 tasks](2026-09-23-matched-controller-targeted.md) | Plain Claude Code against Coder One's v8 controller on the same Opus executor, three interleaved attempts on each of 10 tasks: Wilson intervals, the exact McNemar test, cost and time per arm and task, every attempt, and each controller component's share. |
| [Matched Opus controller experiment](2026-09-23-matched-opus-controller.md) | Twelve fresh attempts with executor controls held fixed: cost, time, steps, failure analysis, sensitivity, retained traces, and explicit retention gaps. |
| [Escalation to GPT-6 Astra on a failed check](2026-09-24-escalation-on-failed-check.md) | Twelve escalations on a failed check or a self-reported failure rescued none (0 of 12, 0–24%), for $20.40 of Astra. Why each trigger failed, the failures the gate missed, and what it should key on instead. |
| [Check recall on retained TB4 trials](2026-09-23-check-recall.md) | The labeled set of 61 graded trials, how often each version of the checks flags the verifier's failures and passes, what flagged each failure, and a live confirmation on the target tasks. |
| [What the TB4 runs so far show](2026-09-23-what-we-have-learned.md) | Where every arm stands, nine lessons (configuration over controller, blind checks, effort, headroom, persistence, cost, routing, infrastructure, evidence), and the ranked improvements. |
| [Coder One against Claude Code on TB4](2026-09-23-coder-one-vs-claude-code-tb4.md) | The same-host, same-model comparison on 26 tasks: per-task results, where the two extra passes and the cost gap come from, and how strong each claim is. |
| [Targeted experiment template](targeted-experiment-template.md) | How to run a repeated, interleaved comparison with `tbench experiment` and publish its Wilson intervals, paired test, losses, and quota use. |
| [TB4 results](tb4-results.md) | Full matrix, arm versions, matched comparisons, harness checks, and refresh limits. |
| [Coder One's task-level wins](2026-09-23-task-win-analysis.md) | Detailed reconstruction of ten wins, local and public failures, controller gaps, costs, conditional completion scenarios, and an upgrade plan. Includes a complete evidence index. |
| [TB4 leaderboard](tb4-leaderboard.md) | Retained public reference: 27 rows, five trials per task, source inconsistencies. |
| [Tunable results, September 23](2026-09-23-tunable-results.md) | Coverage packing, routing, repair, escalation, and monitor experiments. |
| [Development results, September 22](development-results.md) | Four- and eight-task comparisons, cost rankings, individual trial tables. |
| [Original run analyses](2026-09-22-run-analyses.md) | Per-run cost, steps, tokens, time, and delegate behavior. |
| [Measurement and pricing](measurement.md) | Column definitions, aggregation, cost provenance, and historical rates. |
| [Data quality](data-quality.md) | Quota audit, invalid attempts, missing charges, and host differences. |
| [TB4 failure analysis](2026-09-23-tb4-failure-analysis.md) | First failures and the check, routing, and persistence gaps. |
| [Luna/Jev assessment and upgrade plan](2026-09-22-luna-jevprobe-upgrade.md) | Architecture diagrams, detailed probe failures, and proposed upgrades. |
| [Winning runs](winning-runs-analysis.md), [routing](2026-09-22-routing.md), [packing study](2026-09-22-pack-study.md) | Earlier mechanism analyses and replay studies. |

## Run, inspect, and extend

- Run: [targeted experiments](targeted-experiment-template.md), [operating notes](runbook.md), [harness guide](../coder/terminal-bench.md),
  [delegate runbook](coder-one-delegate-runbook.md), and [resilience](resilience.md).
- Replay: [head-to-head traces](../gym/head-to-head.md), with all published Fable 5.1 TB4 attempts and local Coder One versions.
- Inspect: [Gym TUI](../gym/terminal-bench-tui.md), [Gym CLI](../gym/terminal-bench-cli.md),
  [retained traces](../../bench/terminal-bench/traces/), and [evidence retention](runbook.md#retain-the-evidence).
- Extend: [tunable components](../optimization/coder-components.md),
  [implemented policies](../coder/guides/coder-one-tunable.md),
  [episode contract](../coder/terminal-bench-contract.md), and
  [Coder v0.5 design and goldens](../coder/design/coder-terminal-v05-algorithm-and-goldens.md).
- Inspect prompts: [Claude system prompt](claude-code-delegate-prompt/system-prompt.md),
  [captured request](claude-code-delegate-prompt/request.json), and
  [delegate prompts](delegate-prompts/).
- Track: [harness #9530](https://github.com/OpenAgentsInc/openagents/issues/9530),
  [Coder One #9531](https://github.com/OpenAgentsInc/openagents/issues/9531),
  [TB4 #9558](https://github.com/OpenAgentsInc/openagents/issues/9558), and
  [suite infrastructure #9559](https://github.com/OpenAgentsInc/openagents/issues/9559).

After a run, follow the [publication procedure](runbook.md#after-each-run).
Keep this page to current status and links; add detailed results and
analyses to the documents above or a new dated report.
