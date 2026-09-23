# Tunable Coder One development results, 2026-09-23

[Current status](README.md) · [Earlier development results](development-results.md) · [Measurement and pricing](measurement.md)

These are small development screens on eight tasks, not the 66-task TB4
suite. The whole-composition tables report the **sum of eight per-task
means**, equivalent to one average attempt at each task. They do not report
the total spend or time for all 24 trials. Direct Luna has unequal repeats
and is not a matched 24-trial baseline.

The four tunable arms' pass counts, costs, and times were rechecked from
retained `evaluation/usage.json` and `harbor-result.json` files through
`4b6c619770`. Each population contains three trials of each of the eight
tasks under `panel--<arm>--*` and `extended--<arm>--*` in the
[trace store](../../bench/terminal-bench/traces/). The separate
`smoke--coder-one-tunable--fix-git` warmup is excluded. Round after summing
task means: this corrects the earlier coverage-arm cost from $0.1505 to
$0.1504, tunable Opus time from 253.4 s to 253.3 s, and Luna-v2 time from
456.9 s to 457.0 s. Reference arms retain their earlier published figures.

## Coverage packer on `log-summary-date-ranges`, 2026-09-23

The first live screen of the tunable components ([#9547](https://github.com/OpenAgentsInc/openagents/issues/9547)).
`coder-one-pack-luna` is Jev-probe v3 → Luna with one change: the briefing
packer ranks probes and files together, removes duplicate listings, and
keeps representative data records. Artifact `coder-one 0.1.0 (f5d525b600de)`,
policy `crates/coder-one/policies/pack-luna.json`.

| Arm | Passed | Mean cost | Mean agent time |
| --- | --- | ---: | ---: |
| Jev-probe v3 → Luna (old packer) | 0/3 | $0.0019 | 25.4 s |
| **Coverage packer → Luna** | **3/3** | $0.0031 | 24.0 s |
| Codex / GPT-6 Luna direct | 3/3 | $0.0016 | 26.3 s |

v3's briefings spent 76% of their space on overlapping directory listings
and dropped every log excerpt Jev selected, so Luna never saw a record's
severity field. The new briefing delivers the records, and Luna counts the
field instead of the word. The fix holds the executor, directions, and Jev
questions fixed, which supports the packing explanation in this
three-trial screen. It does not establish a general pass-rate improvement.
Traces:
[`extended--coder-one-pack-luna--log-summary-date-ranges`](../../bench/terminal-bench/traces/extended--coder-one-pack-luna--log-summary-date-ranges/)
and `-2`, `-3`.

## Tunable Coder One on the eight development tasks, 2026-09-23

`coder-one-tunable` runs the whole tunable composition in Terminal-Bench
episodes ([#9560](https://github.com/OpenAgentsInc/openagents/issues/9560)):
routing from `task.profile`, the briefing, a delegate, `verify.checks`,
`verify.support`, one `verify.repair`, and escalation from Luna to lean
Opus 5.5. Artifact `coder-one 0.1.0 (1a035cebe6e6)`, tagged
`coder-one-tunable-1a035cebe6`; policy `crates/coder-one/policies/tunable.json`.
Three trials per task.

| Arm | Passed | Sum of task mean costs | Sum of task mean agent times |
| --- | --- | ---: | ---: |
| Claude Code on Opus 5.5 alone | 24/24 | $1.0863 | 306.4 s |
| **Coder One tunable** | **24/24** | **$0.5219** | **253.3 s** |
| Jev-probe v2 → lean Opus, five-minute cache | 24/24 | $0.4282 | 191.6 s |

The tunable arm passed every trial at 52% lower cost and 17% less agent
time than Claude Code on Opus alone. It didn't beat the best hand-tuned arm,
and the traces say why:

- **The router sent all 24 trials to Opus.** Jev scored every task's
  difficulty at 0.5 or more (0.59 to 0.90), so the difficulty rule never
  chose Luna. The arm ran as Opus plus checks and repair. The difficulty
  Score doesn't discriminate these tasks; routing needs the outcome matrix,
  not a single threshold.
- **A check saved one trial.** On `cancel-async-tasks`, one scenario failed
  after the first answer; the repair changed the candidate and the trial
  passed.
- **The support judge triggered three needless repairs.** On
  `git-leak-recovery`, every check passed, Jev's support judgments
  contradicted two requirements, and a repair ran and changed nothing, at
  $0.038 against $0.026 for the answer. A new `checked` repair trigger
  (commit `6d1c2f7a72`) repairs only on an observed scenario failure.
- **The tunable policies used the old briefing packer.** They left
  `brief.packer` unset. `coder-one-tunable-luna-pack` adds the coverage
  packer.

## Luna first, escalating to Opus, 2026-09-23

Two Luna-first tunable arms on the same eight tasks, three trials each:
Codex on GPT-6 Luna starts; a failed check, a contradicted requirement, no
answer, or a monitor stall escalates once to lean Opus 5.5 with a
code-built handoff brief. Artifact `1a035cebe6e6`.

| Arm | Passed | Sum of task mean costs | Sum of task mean agent times |
| --- | --- | ---: | ---: |
| Claude Code on Opus 5.5 alone | 24/24 | $1.0863 | 306.4 s |
| Codex on GPT-6 Luna alone (13 of 16 across unequal trials) | 13/16 | $0.0432 | 577.9 s |
| Luna first, section packer (`coder-one-tunable-luna`) | 22/24 | $0.0859‡ | 406.6 s |
| **Luna first, coverage packer (`coder-one-tunable-luna-pack`)** | **24/24** | **$0.1504‡** | **385.0 s** |

‡ Luna costs are manual list prices. Four trials of the coverage-packer arm
have one Luna dispatch whose charge is unknown; the table counts the
recorded lower bound. The missing charges have no established upper bound,
so the complete cost and savings are unknown.

The missing charges are recorded in these retained episodes:

- [`build-cython-ext`](../../bench/terminal-bench/traces/panel--coder-one-tunable-luna-pack--build-cython-ext/).
- [`fix-code-vulnerability`, first trial](../../bench/terminal-bench/traces/panel--coder-one-tunable-luna-pack--fix-code-vulnerability/).
- [`fix-code-vulnerability`, second trial](../../bench/terminal-bench/traces/panel--coder-one-tunable-luna-pack--fix-code-vulnerability-2/).
- [`sqlite-db-truncate`, second trial](../../bench/terminal-bench/traces/extended--coder-one-tunable-luna-pack--sqlite-db-truncate-2/).

**Every trial passed, with incomplete cost accounting.** The coverage-packer
arm matched Claude Code on Opus 5.5's 24 of 24 with 26% more agent time.
Its recorded cost is 86% below the Opus figure, but missing charges prevent
claiming that percentage as the complete savings. The section packer's two
misses were both `log-summary-date-ranges`, the failure the coverage packer
addressed.

**Monitor false alarms caused expensive escalations.** The policy
escalated on the monitor's stall flag after three judgments. In four
trials (`build-cython-ext`, two `fix-code-vulnerability`,
`sqlite-db-truncate`) it stopped a working Luna session after 12 to 30
seconds, and Opus finished the task for $0.05 to $0.14, 10 to 40 times the
Luna cost. The replay measurements had already shown Jev's stall flag
right 0 times in 22. `coder-one-tunable-luna-v2` keeps the monitor in
shadow mode and escalates only on a failed check, a contradicted
requirement, or no answer. Escalations on `cancel-async-tasks` were real:
Luna's candidate failed a cancellation check, and Opus fixed it.

**Turning those stops off made the arm worse.** `coder-one-tunable-luna-v2`
(artifact `753a17ed975f`; monitor in shadow, repair only on an observed
failure) passed 22 of 24 for $0.1907 and 457.0 s. Without the monitor's
early handoffs, Luna ran alone to the end and failed one `fix-git` and one
`fix-code-vulnerability` trial, and no check caught either: no scenario
applies to a git recovery, and the vulnerability checks passed a wrong fix.
The false stops had been a crude hedge, trading Opus's cost for its
reliability. The fix is check coverage for these task families, not the
monitor.
