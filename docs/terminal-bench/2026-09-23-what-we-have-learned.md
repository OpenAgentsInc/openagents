# Terminal-Bench 4.0: what the runs so far show

Status: analysis of the TB4 data collected through the afternoon of
2026-09-23 (UTC), one attempt per task unless stated. Scoreboard:
`bench/terminal-bench/tools/tb4_scoreboard.py`, which leaves out trials
that hit a provider usage limit. Related: [the 26-task comparison](2026-09-23-coder-one-vs-claude-code-tb4.md),
[the matched pilot](2026-09-23-matched-opus-controller.md), [the task-win
audit](2026-09-23-task-win-analysis.md), and [the failure analysis](2026-09-23-tb4-failure-analysis.md).

## Where things stand

| Arm | What it is | Passed / graded | Mean cost per trial |
| --- | --- | ---: | ---: |
| Coder One v2 | Opus 5.5, medium effort, lean Claude Code, Jev briefing, checks, one repair | 24/49 (49%) | $2.00 |
| Claude Code on Opus 5.5 | Harbor's agent, high effort, defaults | 10/28 (36%) | $2.22 |
| Coder One v3 | v2 with xhigh effort on long tasks | 5/7 | $5.79 |
| Coder One v4 | v3 with behavioral checks, family routing, GPT-6 Astra second executor | 3/6 | $5.34 |
| Coder One v5 | v4 with persistence rounds | 1/3 | $8.90 |
| Coder One v6 | v5 with cheaper escalation, on all 66 tasks | 2/4 (running) | $8.26 |

v3 to v5 ran on selected tasks (effort-sensitive tasks, or tasks v2 failed),
so their pass rates aren't comparable with v2's.

The leaderboard's best rows pass 58% of all 330 trials. On v2's own 45
tasks, GPT-6 Astra at max would be expected to pass about 24 and costs
$6.99 a trial on those tasks.

## What we've learned

### 1. The savings come from configuration, not the controller

On 26 shared tasks Coder One v2 passed 11 against Claude Code's 9, for 45%
less money. But the two arms differed in effort (medium against high),
tools, system prompt, and cache lifetime, as well as in the controller. The
[matched pilot](2026-09-23-matched-opus-controller.md) held those settings
fixed: plain Claude passed 6 of 6 for $6.64, Coder passed 5 of 6 for $6.14.
With the same lean, medium-effort settings, plain Claude is about as cheap,
and the controller added nothing measurable on those two tasks.

**Lesson:** the cheap configuration is the product decision that produced
the headline so far. The controller has to earn its place by passing
tasks the configuration alone fails.

**Follow-up:** the [matched targeted test](2026-09-23-matched-controller-targeted.md)
ran the v8 controller against the same executor on 10 tasks, three
attempts each. It passed 18 of 30 against 15 of 30 (exact McNemar
p = 0.51) for 68% more cost. Persistence rounds account for five of the six
Coder-only passes, three of them on `mvcc-lsm-compaction`; checks, repair,
and escalation changed no outcome.

### 2. Checks rarely catch the failures that matter

Across v2's 49 graded trials, checks ran on 34 but reported a failure on
only 6, while 25 trials failed. That's a recall of about a quarter at
best, and the audit found one of the six was a check reading the wrong
path (`/app` instead of `/results` on `roy-polymorph-cn`). Repair ran 6
times and changed the candidate each time; 3 of those trials passed, but
none is a clean case of a repair turning a failure into a pass. Escalation
never fired: 0 of 49.

**Lesson:** on TB4 the post-execution half of Coder, checks, support,
repair, and escalation, is mostly idle or blind. It's the part of the
design with the most promise and the least effect so far.

### 3. Effort is the strongest lever measured

v3 raised long-task effort from medium to xhigh and passed 5 of 7 tasks
selected because the leaderboard's Opus 5 rows gain on them with effort,
including `cad-model`, `gsea-proteomics`, and `vba-userform-port`, all of
which v2 failed. No controller feature has shown a comparable effect.
The cost is about three times v2's per trial.

### 4. The headroom is in tasks other agents already solve

v2's 25 failures split into:

| Group | Tasks | Examples |
| --- | ---: | --- |
| Never solved by any leaderboard row | 7 | `bun-sourcemap-leak`, `cargo-flight-dispatch`, `glycan-ms2-elucidation` |
| Hard: the best row passes under 80% | 4 | `vba-userform-port`, `music-harmony`, `vllm-deepseek-streaming` |
| **Solved by some row at least 80% of the time** | **14** | `cad-model`, `mvcc-lsm-compaction`, `production-planning`, `atrx-vep-crispr`, `wal-recovery-ordering` |

The later versions recovered 4 of those 14 (`cad-model`, `gsea-proteomics`,
`mvcc-lsm-compaction`, `atrx-vep-crispr`). The other 10 are the main
opportunity: another agent reliably solves them, so the capability exists.

### 5. Persistence makes progress but not passes, at a high price

v5 took `cargo-flight-dispatch`, which no row has ever solved, from 8
failing tests of 27 to 2, over seven sessions and about $12.75. It hasn't
turned a failure into a pass yet. Most persistence rounds change little;
the stop rules let rounds run that don't move any test.

**Follow-up:** in the [matched targeted test](2026-09-23-matched-controller-targeted.md),
v8's persistence turned `mvcc-lsm-compaction` from 0 of 3 into 3 of 3,
where the executor alone stopped after a minute. It also ran on every
attempt, cost 52% of the arm's Claude usage, and missed the same hidden
failures as the baseline on `wal-recovery-ordering` and `fin-saccr-rwa`.

### 6. Jev is almost free; the executor is the whole bill

Jev cost $0.22 of v2's $98, about 0.2%. Every question worth asking about
routing, evidence, or progress is affordable. The spend that matters is
executor tokens, driven by effort and session count.

### 7. Routing hasn't routed

Every TB4 task went to Opus: the difficulty signal scores almost every
task high, and the long-deadline rule sent everything to the strong tier.
The family-routing table in v4 is fitted on the same 66 tasks it would be
scored on. There's no evidence yet that Coder picks configurations per
task better than a fixed choice.

### 8. Infrastructure failures cost more trials than the agents did

In one day, trials were lost or corrupted by:

- a full disk from TB4's ML images;
- Claude login refreshes revoking the token a running trial held;
- the subscription usage limit, which 21 trials hit while being graded;
- Harbor masking a credential path on resume;
- an unreadable instruction file;
- a runner script that silently ran an artifact that ignores policies.

Each now has a guard: a disk watchdog and floor, a preferred long-lived
token, usage-limit requeue and host-wide Claude slots, unmasked resume,
readable uploads, and a doctor check for the policy. **The binding
constraint on throughput is now the Claude quota**, at three to five
concurrent Opus trials.

### 9. The evidence is thin for accuracy claims

One attempt per task. Pass-rate intervals for 26 to 49 tasks are about
±15 points, wider than every accuracy difference observed. Cost and time
differences are consistent enough to trust; accuracy differences aren't
yet.

## What to improve, in order

1. **Establish attribution at scale.** Run the matched design, plain
   Claude Code and Coder One with identical executor settings, on about 20
   TB4 tasks with three attempts each. Until then, don't attribute gains to
   the controller. Also add a lean, medium-effort Claude Code arm as the
   fair baseline for cost claims.
2. **Make checks see failures.** Target the 14 failures other agents solve:
   derive checks from each task's stated outputs and behaviors, fix
   wrong-path checks by resolving outputs from the requirement map, and
   measure check recall against verifier outcomes on retained trials,
   aiming for half of failures flagged before any repair.
3. **Route effort, not just model.** Use xhigh where the leaderboard shows
   effort sensitivity, and medium elsewhere; fit that choice on the task
   pool outside the 66 scored tasks so it can be validated.
4. **Tighten persistence.** Stop a round that changes no test outcome;
   run later rounds on a cheaper executor; cap spend per task relative to
   its value.
5. **Make escalation real.** Escalate on a failed or self-reported check
   to GPT-6 Astra, which v4 showed can recover tasks Opus failed, and
   measure its conditional success.
6. **Run the full suite with repeats.** v6 on all 66 tasks, then three
   attempts per task for Coder and the fair baseline, with the long-lived
   Claude token so no run dies on a login refresh.
7. **Keep the operator's view readable.** The human-friendly Runs pane
   (#9565) is in progress, so a person can follow what each run did.

## Bottom line

Coder One on Opus 5.5 completes as many or more TB4 tasks than Claude Code
for about half the money, but the matched pilot shows most of that comes
from running Claude Code lean and at medium effort. The next gains have to
come from the parts of Coder that decide and verify: checks that notice
failures, routing that picks effort and executor per task, and escalation
that fires. Those parts are built and mostly idle on TB4 today; making them
catch the 10 remaining tasks other agents solve is the clearest path to a
result that beats the base harnesses on accuracy as well as cost.
