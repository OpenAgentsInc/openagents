# Terminal-Bench status

Updated on 2026-09-23 with a completed matched-executor pilot, the `coderos`
trace audit, and public Harbor comparisons. This page summarizes retained
evidence; it does not report the execution host's live queue.

## Latest status

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
| [Matched Opus controller experiment](2026-09-23-matched-opus-controller.md) | Twelve fresh attempts with executor controls held fixed: cost, time, steps, failure analysis, sensitivity, retained traces, and explicit retention gaps. |
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
