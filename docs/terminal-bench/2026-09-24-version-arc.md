# Coder One's version arc, from the Gemini loop to tunable v10

Status: analysis of retained evidence through 2026-09-24 06:32 UTC. No new
trial ran for this document. It reads every policy manifest in
[`crates/coder-one/policies/`](../../crates/coder-one/policies/), the published
Terminal-Bench reports, and the graded trials of four experiments that were
still running or stopping when it was written.

[Current status](README.md) · [What the TB4 runs show](2026-09-23-what-we-have-learned.md) ·
[Tunable guide](../coder/guides/coder-one-tunable.md) ·
[Components design](../optimization/coder-components.md)

## Summary

Coder One started from one question in
[episode 287](../transcripts/287.md): how would you design a coding agent if
language models had no KV cache? The answer was to move the System One work,
looking around, selecting evidence, and judging state, from an expensive
model into cheap code and typed Jev judgments, and to spend the expensive
model only on the fix. Two days and about 25 configurations later, the
evidence says this:

- **What worked was configuration, not control.** Replacing the Gemini
  explorer with Jev probes, running Claude Code with six tools and a
  five-minute cache, and choosing reasoning effort produced every durable
  saving. On the development tasks, the best arm cost 61% less than Claude
  Code on Opus 5.5 at 24 of 24 passes.
- **The controller hasn't earned its cost on Terminal-Bench 4.0 (TB4).** In
  the one matched test, v8's controller passed 18 of 30 against the plain
  executor's 15 of 30 (McNemar p = 0.51) for 68% more money. One task,
  `mvcc-lsm-compaction`, carries the whole difference.
- **Coder One's checks don't tell a pass from a fail.** Across 81 graded
  trials in the running experiments, trials whose final checks all passed
  passed 19 times and failed 19 times. Escalation and persistence act on
  that signal, so both are close to blind.
- **Effort is the strongest measured lever, and the router picks it
  wrongly.** Always-xhigh v3 passed 10 of 14 against medium v2's 7 of 15 on
  the same tasks. v9's effort score ran `gsea-proteomics` at medium and went
  0 of 3, where xhigh went 2 of 3.
- **The Jev briefing is the one controller part with a measured saving.** In
  the matched test, the first executor session cost 20% less than the plain
  executor's whole run.

The standing goal is that Coder is the cheapest and the best on every task,
by choosing a configuration per task rather than by finding one
configuration that wins everywhere. The versions so far added components to
one configuration. The next step is to choose between configurations per
task from observed outcomes, and to stop paying for components that act on
a signal that doesn't discriminate.

## How to read the numbers

- **Dev** tasks are the eight development tasks at upstream pin
  `3b5caaa4863d`: the four **panel** tasks (`fix-git`, `build-cython-ext`,
  `headless-terminal`, `fix-code-vulnerability`) and the four **extended**
  tasks (`cancel-async-tasks`, `git-leak-recovery`,
  `log-summary-date-ranges`, `sqlite-db-truncate`). **TB4** is
  Terminal-Bench 4.0 at `v4.0.0`, 66 tasks with eight-hour agent timeouts.
- The development reports publish the **sum of per-task means**. Every task
  in those screens has the same number of attempts, so this document
  divides that sum by the task count to give the mean per trial. For
  example, $0.4887 over four tasks is $0.122 a trial.
- Cost is recorded model usage at list price, including Jev, from each
  trial's `evaluation/usage.json` or Claude Code's `total_cost_usd`. It is
  not a cash bill on the subscription. GPT-6 costs are computed by hand
  from token counts. See [measurement and pricing](measurement.md).
- Pass intervals are 95% Wilson intervals, computed for this document
  where the source report doesn't give one.
- The published TB4 reports give no agent time for v2 to v6. Those times,
  and Claude Code's 13.9 minutes, are means recounted from the retained
  job records' agent-execution intervals, with usage-limited trials left
  out.
- **Unmatched** marks a comparison whose arms differ in task set, effort,
  executor, or attempt count. Treat its verdict as direction only.
- The in-flight experiments are `effort-9569`, `escalate-9571b`,
  `escalate-9571c`, and `v10-persist-9570`. Their pass counts come from
  `gym terminal-bench experiment report <id> --markdown`. Their costs, agent
  times, check verdicts, and escalation outcomes come from each graded
  trial's `agent/episode/artifacts/composition.json` and
  `evaluation/usage.json` under `~/.openagents/terminal-bench/jobs/`, read
  on 2026-09-24 between 06:27 and 06:32 UTC. The counts can change while
  those schedulers finish.

## Timeline

Each row is one version. **What changed** comes from
`gym coder policy diff A B` on consecutive manifests, or from the
development report for versions that predate policy manifests. The
baseline for each verdict is named in the verdict cell.

### Before policy manifests: 2026-09-22

| Version | What changed | Hypothesis | Tasks and attempts | Passes | Mean cost per trial; mean agent time | Verdict and baseline | Source |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Coder One loop** | A one-command-per-step loop on Gemini 3.8 Flash, with Jev's typed judgments put into each step's prompt. | Small uncached prompts on a cheap model beat a cached frontier agent on cost. | 4 panel tasks, 1 attempt | 3/4 | $0.250; 478 s | **Regressed** against Claude Code on Opus 5.5 (4/4, $0.196, 49 s): dearer, ten times slower, and a timeout. | [dev](development-results.md#headline-four-tasks) |
| Loop, no Jev | The same loop with Jev's per-step hints removed. | Ablation: do the hints help? | 4 panel, 1 | 4/4 | $0.242; 369 s | **No measurable change** from the loop with Jev; it passed one more task and cost slightly less. Per-step hints didn't help. | [dev](development-results.md#headline-four-tasks) |
| **Delegate** (explore 8) | The Gemini loop explores for eight steps, then hands a briefing to Claude Code on Opus 5.5. | A cheap explorer plus a strong finisher beats the strong model alone. | 4 panel, 1 | 4/4 | $0.193; 80 s | **No measurable change** in cost against Claude Code ($0.196), 63% slower. The explorer was 80% to 90% of the Luna delegate's cost. | [dev](development-results.md#headline-four-tasks) |
| Delegate to Luna | The same, finishing on Codex with GPT-6 Luna. | A cheap finisher is enough once the explorer has done the looking. | 4 panel, 1 | 4/4 | $0.033; 127 s | **Improved** on cost against Claude Code; unmatched executor. Luna alone cost $0.008 a trial on the same tasks. | [dev](development-results.md#headline-four-tasks) |
| **Jev brief** | No Gemini explorer: a parallel Jev file survey builds the briefing. | Jev can replace the explorer. | 4 panel, 1 | 4/4 | $0.149; 39 s | **Improved**: 24% cheaper and 20% faster than Claude Code on Opus 5.5. The first arm to beat it. | [dev](development-results.md#headline-four-tasks) |
| **Lean Jev brief** | Claude Code keeps six tools (`Bash`, `Read`, `Edit`, `Write`, `Glob`, `Grep`), which about halves its fixed prompt. | Most of Claude Code's per-call cost is tool definitions. | 4 panel, 3 each | 12/12 (76–100%) | $0.122; 41 s | **Improved**: 25% cheaper and 16% faster than Claude Code (12/12, $0.164, 49 s), matched tasks and attempts. | [dev](development-results.md#repeated-runs-coder-ones-best-configuration-against-opus-55) |
| **Jev probe** | The host runs up to 12 read-only probes in parallel; Jev keeps what the task needs. | Evidence beats hints: put the reflog in front of the model rather than a suggestion to look. | 4 panel, 3 each | Opus: 12/12. Luna: 12/12. | Opus: $0.118; 39 s. Opus low effort: $0.077; 31 s. Luna: $0.0055; 80 s. | **Improved**. Low effort cut Opus cost by 35% at the same passes. Luna passed 12 of 12 at about 1/30 of Claude Code's cost. | [dev](development-results.md#jev-probe-arms-2026-09-22) |
| **Jev probe v2** (`jevprobe2-opus-lean-low-5m`) | A setup pack that runs named install and clone steps, whole files for likely edit targets, a 40-file survey pool, directions to work in few large steps, and a five-minute prompt cache. | Fewer, larger turns and a short cache cut the rest of the bill. | 8 dev, 3 each | Opus: 24/24 (86–100%). Luna: 17/24. | Opus: $0.054; 24 s. Luna: $0.0038; 51 s. | **Improved** for Opus: the best Opus arm, 61% cheaper and 37% faster than Claude Code on the 8 tasks ($0.136, 38 s). **Regressed** for Luna on the panel, 9/12 against v1's 12/12. | [dev](development-results.md#jev-probe-arms-2026-09-22), [routing](2026-09-22-routing.md#the-outcome-matrix) |
| **Jev probe v3** (`jevprobe3-luna`) | Directions to run the task's named checks, exercise every changed path, and audit bulk edits (`brief.directions: batch-checked`); Codex on GPT-6 Luna. | Explicit self-checking makes a cheap executor reliable. | 8 dev, 3 each | Luna: 19/24 (60–91%) | $0.0036; 53 s | **Mixed** against v2 Luna: 11/12 on the panel against 9/12, and 0/3 on `log-summary-date-ranges`. On Opus, the same text made `build-cython-ext` 50% slower. | [dev](development-results.md#jev-probe-arms-2026-09-22), [components](../optimization/coder-components.md#what-todays-evidence-already-says) |
| **Pack Luna** (`pack-luna`) | The coverage packer ranks probes and files together, drops duplicate listings, and keeps data records (`brief.packer: coverage-jev`). | v3 Luna failed because its briefing dropped the records it needed. | 1 extended task, 3 | 3/3 | $0.0031; 24 s | **Improved** against v3 Luna's 0/3 on `log-summary-date-ranges`, with everything else fixed. One task. | [tunable](2026-09-23-tunable-results.md#coverage-packer-on-log-summary-date-ranges-2026-09-23) |

### Tunable Coder One on the development tasks: 2026-09-23

The tunable arms resolve every setting from a manifest. `tunable-opus` is
the lean Opus executor alone; `tunable` adds the rest of the composition.

| Version | What changed | Hypothesis | Tasks and attempts | Passes | Mean cost per trial; mean agent time | Verdict and baseline | Source |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Tunable** (`tunable`) | Against `tunable-opus`: `task.profile` routing between Luna and lean Opus, a Jev monitor that hands off on a stall or a loop, support judgments, and one repair on a detected failure. | Route easy tasks to Luna, watch the session, and repair what checks catch. | 8 dev, 3 each | 24/24 (86–100%) | $0.065; 32 s | **No measurable change** in passes against Jev probe v2 → lean Opus (24/24, $0.054, 24 s), and 21% dearer. Routing sent all 24 trials to Opus: every difficulty score was 0.59 or more. | [tunable](2026-09-23-tunable-results.md#tunable-coder-one-on-the-eight-development-tasks-2026-09-23) |
| **Tunable Luna** (`tunable-luna`) | Routing removed; Codex on GPT-6 Luna first, handing off once to lean Opus on a failed check, a contradicted requirement, no answer, or a monitor stall. | Luna first with an Opus safety net gets Opus's passes at Luna's price. | 8 dev, 3 each | 22/24 (74–98%) | $0.011; 51 s | **Improved** on cost against tunable ($0.065), at two fewer passes, both on `log-summary-date-ranges`. | [tunable](2026-09-23-tunable-results.md#luna-first-escalating-to-opus-2026-09-23) |
| Tunable Luna pack (`tunable-luna-pack`) | Adds the coverage packer. | The packer fix transfers into the full composition. | 8 dev, 3 each | 24/24 (86–100%) | at least $0.019; 48 s | **Improved** against tunable Luna: the packer fixed its two misses. Four Luna dispatches have no recorded charge, so the cost is a lower bound. | [tunable](2026-09-23-tunable-results.md#luna-first-escalating-to-opus-2026-09-23) |
| Tunable Luna v2 (`tunable-luna-v2`) | The monitor's stall and loop handoffs turned off; repair only on an observed check failure (`repair.trigger: checked`). | The monitor's stops were false alarms, 0 right in 22 replays, and wasted Opus money. | 8 dev, 3 each | 22/24 (74–98%) | $0.024; 57 s | **Regressed** against Luna pack (24/24). The false stops had been a crude hedge: without them, Luna failed `fix-git` and `fix-code-vulnerability` once each, and no check caught either. | [tunable](2026-09-23-tunable-results.md#luna-first-escalating-to-opus-2026-09-23) |
| Tunable Luna snapshot | Adds `verify.snapshot`. | Keep the first executor's state for truer check replay. | 3 TB4 tasks, 1 (`try`) | 1/3 | not reported | **Unmeasured**: a smoke run only. | Job `try--coder-one-tunable-luna-snapshot--20260923T162706Z` |

### Tunable Coder One on TB4: 2026-09-23 and 2026-09-24

From v2 on, every TB4 task has an eight-hour timeout, so the long-deadline
rule routes every trial to lean Opus. v2 onward are Opus-first policies.

| Version | What changed (policy diff) | Hypothesis | Tasks and attempts | Passes | Mean cost per trial; mean agent time | Verdict and baseline | Source |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **v2** | Against `tunable`: the coverage packer, and repair only on an observed check failure. On TB4, medium effort on long tasks. | The development composition carries to TB4. | 49 TB4 tasks, 1 each | 24/49 (36–63%) | $2.00; 13.9 min | **Improved** against Claude Code on Opus 5.5 at high effort (10/28, 21–54%, $2.22). On the 26 tasks both graded: 11 against 9 for 45% less money and 38% less time. Unmatched effort, tools, prompt, and cache. | [learned](2026-09-23-what-we-have-learned.md#where-things-stand), [head to head](2026-09-23-coder-one-vs-claude-code-tb4.md#the-claim-corrected) |
| v2, matched pilot (`matched-opus-medium-v2`) | The same controller, with plain Claude Code given v2's exact executor settings. | Does the controller or the configuration produce v2's savings? | 2 TB4 tasks, 3 each, 1,680-second budget | 5/6 (44–97%) | $1.02; 5.7 min | **No measurable change** against plain Claude with the same settings (6/6, $1.11, 6.2 min): 7.6% cheaper with one fewer pass. The savings came from configuration. | [pilot](2026-09-23-matched-opus-controller.md) |
| **v3** | `horizon.long_effort`: medium → xhigh. | Effort, not the controller, limits the tasks where the leaderboard gains with effort. | 7 selected TB4 tasks, 1 each | 5/7 (36–92%) | $5.79; 30.2 min | **Improved**; unmatched task set. Passed `cad-model`, `gsea-proteomics`, and `vba-userform-port`, which v2 failed. | [learned](2026-09-23-what-we-have-learned.md#3-effort-is-the-strongest-lever-measured) |
| v3, matched (`effort-9569`) | Same policy, run beside v2 and v9. | Same. | 6 TB4 tasks, 3 each, interleaved (14 graded) | 10/14 (45–88%) | $2.99; 18.0 min | **Improved** against v2 on the same attempts (7/15, 25–70%, $1.01, 5.8 min): v3 alone passed 3 pairs, v2 alone 0 (McNemar p = 0.25). Three times the cost. | `effort-9569` report and records |
| **v4** | Family routing between lean Opus and Codex on GPT-6 Astra, from a leaderboard table (`route.rule: profile-v2`); a second executor (Astra, then Opus) after an unconfirmed or failed check; executor self-report checks; optional outputs; a behavior-first support budget. | Checks that read the executor's own doubts, and a different executor, recover what Opus fails. | 6 TB4 tasks v2 failed that some row solves, 1 each | 3/6 (19–81%) | $5.34; 25.1 min | **Unmeasured** against a baseline: no arm ran these tasks beside it. The family table is fitted on the same 66 tasks it scores. | [learned](2026-09-23-what-we-have-learned.md#where-things-stand) |
| **v5** | `control.persist`: up to three fresh persistence sessions after checks and repair, with a guard that puts back a worse candidate. | Another session finishes work the first left undone. | 3 TB4 tasks, 1 each | 1/3 (6–79%) | $8.90; 44.1 min | **Unmeasured** for passes. Took `cargo-flight-dispatch` from 19 to 25 of 27 tests over seven sessions for about $12.77. | [learned](2026-09-23-what-we-have-learned.md#5-persistence-makes-progress-but-not-passes-at-a-high-price), [persist](2026-09-23-persist-v8.md#compared-with-earlier-arms-on-these-tasks) |
| **v6** | `persist.max_rounds`: 3 → 2; the second executor runs only after a failed check. | Cut spend on rounds and second executors that don't pay. | 4 TB4 tasks, 1 each | 2/4 (15–85%) | $8.26; 40.9 min | **Unmeasured**: the full-suite run it was meant for didn't finish. | [learned](2026-09-23-what-we-have-learned.md#where-things-stand) |
| **v7** | `verify.behavior` scenarios; `verify.snapshot` of the workspace after the first executor. | Checks that exercise behavior catch the failures other agents avoid. | Offline: 61 retained trials. Live: 9 targets, 1 each; then 3 tasks, 1 each in `v10-persist-9570`. | Offline: flags 14 of 27 failures and 1 of 34 passes. Live: 0/9, then 1/3. | Live in `v10-persist-9570`: $6.90; 44.7 min | **Improved** offline recall against the episodes' own first check (5 of 27), on the set the checks were written from. The live test is **unmeasured**: a revoked login token ended every session. | [recall](2026-09-23-check-recall.md#result), `v10-persist-9570` records |
| **v8** | Persistence keeps its own tests at `/tmp/persist-tests/run.sh`, stops a round with no progress, runs rounds from the second on GPT-6 Sol, allows 4 rounds, and caps rounds at half of what a $10 budget has left. | Stop rounds that change nothing, and run them cheaper. | 3 TB4 near-miss tasks, 3 each | 3/9 (12–65%) | $6.53; 37.6 min | **Improved** on cost against v5 and v6 (persistence $2.63 a trial). No pass credited to persistence; the executor's own tests passed in every round. Unmatched: no control arm. | [persist](2026-09-23-persist-v8.md) |
| v8, matched (`matched-opus-medium-v8`) | v8 with every executor fixed to Opus 5.5 at medium, no routing, no cheap tier. | With the executor fixed, does the controller pass more? | 10 TB4 tasks by a fixed rule, 3 each, interleaved | 18/30 (42–75%) | $1.51; 14.6 min | **No measurable change** in passes against plain Claude Code with the same settings (15/30, 33–67%, $0.90, 6.6 min; McNemar p = 0.51). **Regressed** on cost: 68% more, 2.2 times the time. | [matched](2026-09-23-matched-controller-targeted.md) |
| **v9** | v2 plus a Jev effort score (`effort.rule: sensitivity-v1`): xhigh at a score of 0.4 or more, medium below. Drops v4 to v8's checks, second executor, and persistence. | Choose effort per task and keep v3's passes at lower cost. | 6 TB4 tasks, 3 each, interleaved (15 graded) | 8/15 (30–75%) | $2.54; 13.1 min | **No measurable change** against v3 (10/14; McNemar p = 0.69), at 76% of v3's cost on per-task means, above its 60% bar. Scored `gsea-proteomics` 0.17 to 0.18, ran it at medium, and went 0/3 where v3 went 2/3. | [effort routing](2026-09-24-effort-routing.md), `effort-9569` records |
| **v9-escalate** | v9 plus checks, self-report, behavior scenarios, and a second executor, Codex on GPT-6 Astra, on a failed check or a self-reported failure. No repair. | Escalating to a different executor on a flagged failure turns failures into passes. | 8 TB4 tasks where v7's checks flag a retained failure, 3 each (31 graded over two runs) | 11/31 (21–53%) | $3.68; 25.3 min | **Regressed** on the mechanism: escalation fired 12 times, cost $20.40, and rescued none (0 of 12, 0–24%). Unmatched: the control arm is `nop`, so the passes have no baseline. | [escalation](2026-09-24-escalation-on-failed-check.md), `escalate-9571b` and `escalate-9571c` records |
| **v10** | v8 plus persistence judged by what the checks flag (`persist.judge: checks`), the cheap tier from round 1, and the second executor's candidate kept only when it resolves the failure (`second.keep: resolved`). | Persistence and a second executor should act on what the checks flag, and cheaply. | 4 TB4 tasks, 3 each, against v7 (3 graded each so far) | 1/3 (6–79%) | $5.01; 30.4 min | **Too early**: 1/3 against v7's 1/3, same attempts. So far 27% cheaper and 32% faster; persistence cost $0.75 a trial against v7's $3.46. | `v10-persist-9570` report and records |

The mean for v9-escalate combines both runs: $74.06 and $40.00 over 21 and
10 trials, and 23.4 and 29.2 agent-minutes.

### Policies with no graded trial

The four handoff manifests, `handoff-escalate`, `handoff-planner-worker`,
`handoff-race`, and `handoff-steer`, set `control.handoff.pattern` to
each of the patterns the handoff code implements. `gym coder policy list`
shows no attempt for any of them, so the handoff patterns other than the
Luna-to-Opus escalation are unmeasured.

## The strategies over time

### Move exploration from a model to Jev probes

**Tried:** the Gemini loop with per-step Jev hints, the Gemini explorer in
front of a delegate, the Jev survey briefing, Jev probes v1 to v3, the
coverage packer, and a hill-climbing study of the packer's parameters.

**Evidence:** per-step hints didn't help the Gemini loop; the no-Jev
ablation passed one more task
([dev](development-results.md#headline-four-tasks)). Replacing the explorer
with a Jev briefing made every executor cheaper: Luna made zero exploratory
commands in 12 trials, and its first command was always the fix
([episode 287](../transcripts/287.md), from 01:41:30). In the matched test,
the first Opus session on a Jev briefing cost $21.64 over 30 attempts
against $27.04 for the plain executor's whole run, 20% less, for $0.18 of
Jev ([matched](2026-09-23-matched-controller-targeted.md#analysis)). The
coverage packer turned a 0/3 task into 3/3 on Luna. The packer's
parameter study found no held-out gain, ΔJ +0.0001 against the +0.01 bar
([pack study](2026-09-22-pack-study.md#held-out-confirmation)).

**Status: working.** The briefing is the part of Coder One with a
repeated, matched saving. Its packer defaults are good enough; tuning them
further doesn't pay.

### Delegate to a strong executor

**Tried:** Claude Code on Opus 5.5 as the finisher from the first delegate
arm on; Codex on GPT-6 Astra as v4's family route and v4 to v10's second
executor.

**Evidence:** the task-win audit found that Opus 5.5 produced all ten
highlighted Coder One solutions on TB4
([task wins](2026-09-23-task-win-analysis.md)). As a second executor, Astra
replaced Opus's candidate on `cargo-flight-dispatch` in all three v8
attempts, and the verifier then passed 19 of 27 tests against the 25 of 27
v5 and v6 reached ([persist](2026-09-23-persist-v8.md#what-the-records-show)).
In v9-escalate, the host kept Astra's candidate 8 times, and all 8 failed.

**Status: working for Opus as the first executor; not working for Astra as
a replacement.** Astra from the task's original state hasn't recovered a
task Opus failed on these runs. The v4 claim that Astra recovers such tasks
rests on unmatched single attempts.

### Lean Claude Code: six tools and a short cache

**Tried:** six tools and a replaced headless system prompt, low then medium
effort, and the five-minute prompt cache instead of the subscription's
one-hour cache.

**Evidence:** six tools cut Opus's cost by 25% at 12 of 12
([dev](development-results.md#repeated-runs-coder-ones-best-configuration-against-opus-55)).
One-hour cache writes were 54.1% of the low-effort Opus arm's cost
([winning runs](winning-runs-analysis.md)), and the five-minute cache cut the Jev probe v2 arm's panel cost by 25%
([dev](development-results.md#jev-probe-arms-2026-09-22)). The matched
pilot showed plain Claude Code gets the same savings when you give it these
settings ([pilot](2026-09-23-matched-opus-controller.md)).

**Status: working.** It's the largest single source of the savings that
v2 reported against default Claude Code, and it belongs to the
configuration, not to the controller.

### Route by task: executor and effort

**Tried:** the `task.profile` difficulty rule (tunable), the long-deadline
rule, v4's leaderboard family table, v3's fixed xhigh, and v9's Jev effort
score.

**Evidence:** the oracle over the development arms, choosing the cheapest
configuration that passed three of three on each task, passes 24 of 24 for
$0.0716 in summed means, against $0.4282 for the best fixed arm
([components](../optimization/coder-components.md#what-todays-evidence-already-says)).
No router has reached it: the difficulty rule sent all 24 development
trials to Opus, and a fitted stump and a Jev Choice had higher regret than
fixed Opus on eight tasks ([routing](2026-09-22-routing.md#leave-one-task-out-regret)).
On TB4, every task went to Opus. Effort is the strongest measured lever:
v3 beat v2 by 3 discordant pairs to 0. v9's effort score gave back most of
that gain, because it scored `gsea-proteomics` at 0.17 to 0.18 and ran it
at medium, 0 of 3 against xhigh's 2 of 3.

**Status: not working yet.** The headroom is real and measured; the
routers so far read task text through a score and haven't picked better
than a fixed choice.

### Verify and recover: checks, support, repair, escalation, and persistence

**Tried:** requirement checks and support judgments (tunable), checked
repair (v2), self-report checks and a second executor (v4), persistence
(v5 to v8 and v10), behavior scenarios (v7), escalation to Astra
(v9-escalate), and persistence judged by the checks (v10).

**Evidence:** on v2's 49 TB4 trials, checks reported a failure on 6 trials
while 25 failed, and escalation never fired ([learned](2026-09-23-what-we-have-learned.md#2-checks-rarely-catch-the-failures-that-matter)).
Offline, v7's checks flag 14 of 27 failures, but 11 of the 14 flags are the
executor's own doubts, on the same set the checks were written from
([recall](2026-09-23-check-recall.md#caveats)). In the matched test, 2 of 30
first checks failed, repair ran twice and changed no outcome, support left
141 of 145 requirements unresolved, and escalation never ran. Persistence
produced five of the six Coder-only passes and 52% of the arm's Claude cost
([matched](2026-09-23-matched-controller-targeted.md#controller-components)).
The in-flight records add that the final checks don't discriminate at all,
and that escalation fired 12 times without recovering a failure (see
[the check finding](#the-checks-dont-discriminate)).

**Status: not working, except persistence on tasks where the executor
stops early.** Persistence's one clear win, `mvcc-lsm-compaction`, is a
task where the executor stopped after about a minute. Everything else in
this thread acts on a check signal that carries no information about the
verifier's result.

### Cheap executors: Luna and Sol

**Tried:** Jev probe to Luna, Luna-first tunable arms with an Opus
handoff, and GPT-6 Sol as v8 and v10's cheap persistence tier.

**Evidence:** on the development tasks, Luna with the coverage packer and
an Opus handoff passed 24 of 24 for at least $0.019 a trial, against
Claude Code's $0.136 ([tunable](2026-09-23-tunable-results.md#luna-first-escalating-to-opus-2026-09-23)).
Its reliability came partly from monitor false alarms that handed working
Luna sessions to Opus; with them off, it passed 22 of 24. On TB4, no Luna
arm has a graded experiment. Sol persistence rounds cost $0.16 to $0.61 and
changed no outcome in v8 ([persist](2026-09-23-persist-v8.md#what-the-records-show));
in v10, Sol's first rounds cut persistence cost by 78% against v7 with the
same passes so far.

**Status: working on the development tasks, unmeasured on TB4.** Luna is
the only route to the cheapest cell on most development tasks, and it has
never been tried where the money is.

## What's improving, and what's regressing or stuck

### Consistently improving

- **Cost at equal passes on the development tasks.** On the same four
  panel tasks, each Opus step from the Jev brief to Jev probe v2 with the
  five-minute cache passed every attempt while the cost per trial fell from
  $0.149 to $0.061, a 59% drop ([dev](development-results.md)). The Jev
  brief ran once per task; the later arms ran three times.
- **The first executor session.** The Jev briefing saved 20% on the first
  session in the matched test ([matched](2026-09-23-matched-controller-targeted.md#analysis)),
  and Jev costs about 0.2% of a trial ([learned](2026-09-23-what-we-have-learned.md#6-jev-is-almost-free-the-executor-is-the-whole-bill)).
- **Persistence's price.** Persistence cost fell from about $6.19 a trial on
  `cargo-flight-dispatch` in v5 to $2.63 in v8 and $0.75 in v10's first
  three trials ([persist](2026-09-23-persist-v8.md#compared-with-earlier-arms-on-these-tasks), `v10-persist-9570` records).
- **Measurement.** Each week's comparisons are more matched than the last:
  single attempts, then three attempts on selected tasks, then interleaved
  paired experiments with a fixed task rule.

### Regressing or stuck

- **The controller costs more than it returns.** 68% more cost and 2.2
  times the agent time for 3 net passes that a paired test can't tell from
  chance, all from one task ([matched](2026-09-23-matched-controller-targeted.md#summary)).
  On the three tasks both arms always passed, persistence added $6.51 and
  changed no outcome.
- **The checks don't discriminate.** See the next section. This blocks
  escalation, repair, and persistence, which all trigger on the checks.
- **Cost per trial climbed faster than passes.** From v2's $2.00 to v5's
  $8.90 and v6's $8.26 a trial on TB4, with pass rates on selected tasks
  that no matched arm confirms ([learned](2026-09-23-what-we-have-learned.md#where-things-stand)).
- **Routing.** No router has chosen better than a fixed configuration, and
  v9's effort score lost the one lever that worked on one of six tasks.
- **Escalation.** It fired 0 times in v2's 49 trials and 0 times in the
  matched test. When v9-escalate made it fire, it cost $20.40 and turned
  no failure into a pass.

### The checks don't discriminate

This finding is from the in-flight experiments and was verified from the
composition records for this document. It covers the 81 graded Coder One
trials in `effort-9569`, `escalate-9571b`, `escalate-9571c`, and
`v10-persist-9570` at 06:32 UTC. Each trial is placed by the verdicts in
`final_checks`.

| Final checks | Passed | Failed | Pass rate |
| --- | ---: | ---: | ---: |
| All scenarios passed | 19 | 19 | 50% |
| Inconclusive | 10 | 19 | 34% |
| No scenario ran (all on `embedding-drift-monitor`) | 6 | 0 | 100% |
| A check failed | 3 | 5 | 38% |
| All 81 trials | 38 | 43 | 47% |

A trial whose checks all passed is no likelier to pass than any other
trial. The operator's earlier count, over 80 trials, reported inconclusive
as 16 passes and 18 failures. That count matches this one if it put the six
trials with no scenario under inconclusive and ran before one more failure
was graded; this document uses the recount from the records.

**Escalation fired 12 times, all in v9-escalate, and 2 of those trials
passed.** Both passes are `html-js-filter`, where the host compared the
candidates and kept Opus's first one. In the 8 escalations where the host
kept Astra's candidate, 5 on `cargo-flight-dispatch` and 3 on
`production-planning`, every trial failed. The escalations cost $20.40
([escalation](2026-09-24-escalation-on-failed-check.md)).
Escalation hasn't turned a failure into a pass. On `production-planning`,
the executor's self-report was the one signal that sorted trials: the three
attempts without a self-reported failure passed, and the three with one
escalated and failed. The records can't show whether Opus's own candidate
would have passed in those three.

**Effort routing misrouted one task.** v9 scored `gsea-proteomics` at
0.168, 0.177, and 0.183, below its 0.4 threshold, ran it at medium, and
passed 0 of 3. Always-xhigh v3 passed 2 of 3 and medium v2 passed 1 of 3.

## Measurement lessons

- **Unmatched comparisons told the wrong story.** The 26-task comparison
  credited Coder One with 45% lower cost; the matched pilot and matched
  test showed that most of it came from effort, tools, prompt, and cache,
  which plain Claude Code can use too ([pilot](2026-09-23-matched-opus-controller.md)).
  v3 to v6 ran on selected tasks, so their pass rates can't be compared with
  v2's.
- **One attempt per task can't carry an accuracy claim.** Pass-rate
  intervals over 26 to 49 tasks are about ±15 points, wider than every
  accuracy difference observed ([learned](2026-09-23-what-we-have-learned.md#9-the-evidence-is-thin-for-accuracy-claims)).
  v4, v5, and v6 have 6, 3, and 4 graded trials.
- **Lost trials distorted the counts.** 21 trials hit the usage limit and
  were graded anyway; a revoked login token ended v7's live test; a full
  disk, a missing CA bundle, an unreadable instruction file, and a runner
  that ignored policies each cost trials
  ([learned](2026-09-23-what-we-have-learned.md#8-infrastructure-failures-cost-more-trials-than-the-agents-did),
  [data quality](data-quality.md)). Each has a guard now.
- **An experiment needs a real control arm.** v8's persistence run and
  v9-escalate used `nop` as the second arm, because an experiment needs two
  arms and `nop` draws no quota. Their pass counts have no baseline.
- **Offline recall isn't live discrimination.** v7's checks met the recall
  bar on the trials they were written from, and don't discriminate on new
  trials.
- **The move that works:** interleaved experiments with three attempts per
  task per arm, a task rule committed before the first trial, Wilson
  intervals, an exact McNemar test on the pairs, and early stopping when
  the question is answered ([template](targeted-experiment-template.md)).
  When this document was written, `effort-9569` and `escalate-9571c` were
  stopping early, at 44 of 54 and 20 of 24 scheduled trials.

### Where the sources disagree

- **v2 and Claude Code on TB4.** The [status page](README.md) reports 19/45
  at $1.42 and 10/27 at $2.28; the later
  [lessons](2026-09-23-what-we-have-learned.md#where-things-stand) report
  24/49 at $2.00 and 10/28 at $2.22. This document uses the later figures,
  because a recount of the retained job records gives 24/49 at $2.04.
- **v4 and v5 cost.** The lessons report $5.34 and $8.90 a trial. The
  records price only 4 of v4's 6 and 2 of v5's 3 trials, for $7.41 and
  $13.34 over the priced trials; the lessons figure divides by every graded
  trial. This document uses the published figures and notes that they
  understate the cost of a trial that ran.
- **v6.** The lessons call v6 "v5 with cheaper escalation." The policy diff
  shows two changes: two persistence rounds instead of three, and a second
  executor only on a failed check. This document uses the diff.
- **v9's cost against v3.** The mean cost per graded trial puts v9 at 85%
  of v3's ($2.54 against $2.99). The [effort report](2026-09-24-effort-routing.md)
  sums per-task means and gets 76%, because v9 has three graded attempts on
  the most expensive task, `vba-userform-port`, and v3 has two. This document uses 76% for the verdict, since that
  is the metric the experiment's acceptance bar names.
- **Escalation cost.** Summing the 12 records' `cost_usd` gives $20.41;
  the [escalation report](2026-09-24-escalation-on-failed-check.md) gives
  $20.40. This document uses the report's figure.
- **Policy digests.** The manifest files hash differently from the digests
  the runs recorded (for example, v2's file is `e8842585fb51` and its runs
  are `85153db960ec`), because fields were added to the files after the
  runs. The diffs in this document compare the current files; the runs'
  own manifests are in `gym coder policy list`.

## Recommendations

The standing goal is that Coder is the cheapest and the best on every task,
by choosing a configuration per task. Every recommendation below moves
toward a per-task table of measured outcomes, rather than a bigger fixed
composition.

### Stop

- **Stop running persistence on every attempt.** It added $6.51 on tasks
  both arms always passed. Keep it only behind a trigger that predicts an
  unfinished task, such as a first session that stops early.
- **Stop escalating to Astra from the original state.** 12 escalations,
  $20.40, and no recovered failure. Turn off `verify.second` in the default
  policy until a trigger discriminates.
- **Stop building on the check verdicts as a pass signal.** They don't
  discriminate on live trials. Keep the checks as evidence for a repair
  brief, not as a gate.
- **Stop tuning the packer's parameters.** The study found no held-out
  gain.
- **Stop reporting unmatched arms as verdicts.** A new policy runs beside a
  real baseline, or its result is labeled a smoke run.

### Double down

- **The Jev briefing and the lean executor.** Both have matched savings.
  Make them the default for every arm, including the baseline you compare
  against.
- **Effort as a per-task choice from observed outcomes.** It's the strongest
  lever measured. Choose it from the outcome matrix and the leaderboard's
  per-task effort sensitivity, not from a text score alone.
- **Matched, interleaved, three-attempt experiments that stop early.** The
  matched test settled the controller question for $72 of Claude quota, and
  `effort-9569` measured the effort lever for $93.

### Next experiments, ranked by expected value

Each is small, runs three interleaved attempts per task per arm against a
real baseline, and stops early when the paired result can't change the
decision. Rank reflects the expected gain toward the per-task goal for the
quota it needs.

1. **Effort from observed outcomes.** A v2 arm whose effort comes from a
   per-task table: xhigh where the TB4 leaderboard's Opus 5 rows gain at
   least 0.4 from medium to xhigh, or where our own matrix shows it, medium
   elsewhere. Run it against v3 on the six `effort-9569` tasks plus four
   tasks medium already passes. It should match v3's passes at v2's cost on
   the insensitive tasks. Stop when the arm is two discordant pairs behind
   v3 on sensitive tasks, or after two attempts if it ties v3 and costs
   less.
2. **Offline trigger audit, no runs.** Over the 81 in-flight trials and the
   60 matched-test trials, measure for each candidate trigger (self-report
   by task family, first-session turns and duration, own-test results,
   support contradictions) its pass-and-fail split. Promote only a trigger
   with a measured precision to gate repair, persistence, or escalation.
   The `production-planning` self-report split and the `mvcc-lsm-compaction`
   early stop are the two candidates the records already show.
3. **Persistence behind an early-stop trigger.** The matched v8 controller
   with persistence only when the first session ended under a threshold of
   turns or minutes, against plain Claude Code, on `mvcc-lsm-compaction`,
   `heat-pump-warranty`, `sound-change-cascade`, and the three tasks both
   arms always passed. It should keep the `mvcc-lsm-compaction` win and
   drop most of the 68% cost gap. Stop when it loses a pass the full v8
   controller won, or when it matches v8's passes at under half v8's extra
   cost.
4. **Finish `v10-persist-9570` on `mvcc-lsm-compaction` only.** It's the
   task where persistence showed its one clear win, and v10 runs round 1 on
   Sol. If Sol's round keeps the 3 of 3 that Opus's rounds got, cheap
   persistence is proven where it matters; if not, v10's cost saving is a
   saving on rounds that didn't help anyway. Stop the other three tasks,
   which match v8's earlier result.
5. **Luna first on TB4 tasks medium Opus already passes.** Luna with the
   coverage packer and an Opus fallback on no answer, against v2, on four
   to six TB4 tasks v2 passes every time. If Luna holds on even some of
   them, the per-task table gains a cell that costs a tenth of Opus. Stop a
   task at its first Luna failure, and record the cell either way. The
   [Luna TB4 protocol](2026-09-24-luna-tb4-baseline.md) for issue
   [#9583](https://github.com/OpenAgentsInc/openagents/issues/9583), with
   `tunable-luna-pack-solo.json`, measures Luna without the Opus fallback
   first; run the fallback arm on the tasks where it fails.

## Evidence

- Policy manifests: [`crates/coder-one/policies/`](../../crates/coder-one/policies/),
  compared with `gym coder policy diff` and listed with
  `gym coder policy list`.
- Published reports: [development results](development-results.md),
  [tunable results](2026-09-23-tunable-results.md),
  [what the TB4 runs show](2026-09-23-what-we-have-learned.md),
  [Coder One against Claude Code](2026-09-23-coder-one-vs-claude-code-tb4.md),
  [matched pilot](2026-09-23-matched-opus-controller.md),
  [matched test](2026-09-23-matched-controller-targeted.md),
  [v8 persistence](2026-09-23-persist-v8.md),
  [check recall](2026-09-23-check-recall.md),
  [task wins](2026-09-23-task-win-analysis.md),
  [failure analysis](2026-09-23-tb4-failure-analysis.md),
  [effort routing](2026-09-24-effort-routing.md),
  [escalation on a failed check](2026-09-24-escalation-on-failed-check.md),
  [routing](2026-09-22-routing.md), [pack study](2026-09-22-pack-study.md),
  [TB4 results](tb4-results.md), and the
  [components design](../optimization/coder-components.md).
- The delegate door outside Terminal-Bench:
  [delegate against Gemini](../coder/measurements/2026-09-23-delegate-vs-gemini.md)
  answered 8 of 8 repository questions against Gemini's 6, at $0.054 a turn
  against $0.009.
- The founding strategy: [episode 287](../transcripts/287.md) and
  [episode 288](../transcripts/288.md).
- In-flight experiments: `~/.openagents/terminal-bench/experiments/{effort-9569,escalate-9571b,escalate-9571c,v10-persist-9570}/`,
  their job directories under `~/.openagents/terminal-bench/jobs/`, and the
  standing outcome matrix from `gym coder matrix --profile tb4`. Issues
  [#9569](https://github.com/OpenAgentsInc/openagents/issues/9569),
  [#9570](https://github.com/OpenAgentsInc/openagents/issues/9570), and
  [#9571](https://github.com/OpenAgentsInc/openagents/issues/9571).
