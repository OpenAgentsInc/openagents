# The first study: `evidence.pack` parameters

The first hill-climbing study over Coder One's policy manifests (#9557)
tuned the coverage packer's budget, slice, span, and task-text reserve on
every retained briefing. It finished, recorded all 221 candidates and
their spend, and its winner **does not beat the hand-authored baseline
beyond the noise floor on held-out evidence**.

The study ran tiers 0 (replay) and 1 (mini-tasks). No Terminal-Bench
screen, measurement, or confirmation ran: those tiers are defined and
runnable, but gated behind `--allow-terminal-bench`, and the result lists
the harness commands each would run.

```sh
coder-one study run evidence.pack --through mini --retain
gym coder study evidence-pack-9e6f68505e58 --all
gym-terminal --terminal-bench   # press s
```

The retained records are in
[`bench/terminal-bench/studies/evidence-pack-9e6f68505e58/`](../../bench/terminal-bench/studies/evidence-pack-9e6f68505e58/):
the frozen plan, every proposal and candidate with its manifest, the
selection committed before confirmation, the promoted candidates'
manifests, and the result. The per-briefing trials (12 MB) stay in
`~/.openagents/coder-one/studies/`. [Run a study](../coder/guides/coder-one-components.md#run-a-study)
explains the runner.

## The plan

- **Baseline**: `policies/pack-luna.json` with the coverage packer at its
  defaults: cap 12,000, slice 1,200, item max 8,000, and task-text share
  0.5. Replay has no recorded coverage judgments for most briefings, so
  the baseline names `coverage`, not `coverage-jev`.
- **Cases**: 233 retained briefings; one early smoke briefing can't be
  read back and is named in the result.
- **Split, by task**: `cancel-async-tasks`, `fix-git`, and
  `git-leak-recovery` are held out (80 briefings), chosen by the SHA-256 of
  seed 9557 and each task name before anything ran. The five development
  tasks' briefings alternate between search (79) and selection (74).
- **Objective**: J = ½·Jev-selected items delivered + ½·hand-labeled
  requirements covered by delivered evidence − duplicate listing bytes /
  12,000 − 0.1·briefing characters / 12,000, with tasks weighted equally.
- **Acceptance**: on the held-out briefings, a mean paired ΔJ of at least
  0.01 and a 2.5th percentile above zero in a task-clustered bootstrap
  with 2,000 resamples.

## The candidates

| Operator | Algorithm | Proposals | Candidates | Best search J | Best selection J |
| --- | --- | --- | --- | --- | --- |
| baseline | Hand-authored defaults | 1 | 1 | 0.5242 | 0.5309 |
| swap | The first packer in place of the coverage packer | 1 | 1 | 0.3779 | — |
| grid | Grid search: 4 caps × 4 slices × 4 item maxima × 3 shares | 192 | 192 | 0.5461 | 0.5531 |
| climb | Coordinate ascent from the baseline | 32 | 27 (5 duplicates) | 0.5315 | 0.5383 |

Successive halving kept 74 of 221 candidates after search and 25 of 75
after selection. All 25, and the baseline, passed every mini-task with the
scripted executor and stayed within their caps. The selected candidate is
the grid's `cap=8000 slice=800 item_max=2000 instruction_share=0.3`
(`c46671cf47e6`). The climb moved the cap from 12,000 to 10,000 and the
slice to 1,000, then stopped at a local optimum below the grid's best.

The component swap confirms the coverage packer's gain over the first
packer on the search partition: 80% of Jev-selected items delivered
against 100%, 43,147 duplicate listing bytes against none, and 68 items
omitted against 14.

| Search partition | Baseline | Selected |
| --- | --- | --- |
| J | 0.5242 | 0.5461 |
| Jev-selected items delivered | 100% | 100% |
| Labels covered by delivered evidence | 21.6% | 21.6% |
| Labels any item informs (the ceiling) | 21.6% | 21.6% |
| Duplicate listing bytes | 0 | 0 |
| Mean briefing characters | 7,692 | 5,294 |
| Items omitted | 14 | 25 |

## Held-out confirmation

| Held-out partition | Baseline | Selected |
| --- | --- | --- |
| J | 0.4733 | 0.4733 |
| Mean briefing characters | 3,331 | 3,320 |
| Labels any item informs | 0% | 0% |

ΔJ is +0.0001, with a 95% interval of 0.0000 to 0.0002; per task, 0.0000
for `cancel-async-tasks` and `git-leak-recovery` and +0.0002 for
`fix-git`. The rule needs +0.01, so the winner doesn't beat the baseline.

## What the study shows

- **No coverage headroom at these parameters.** The baseline already
  delivers every Jev-selected item and every labeled requirement that any
  item informs. The whole search-partition gain is size: a smaller cap and
  a 2,000-character item maximum cut the development briefings by 31%.
- **The held-out tasks' briefings are already small.** They average 3,331
  characters, mostly under every candidate's cap, and no retained item informs
  their labels through an exact path or constant. Nothing a packing
  parameter changes reaches them, so the confirmation is a null.
- **The objective has blind spots.** It counts a trimmed item as delivered,
  so it can't see that a 2,000-character item maximum trims selected
  items; and it doesn't price unselected, unlabeled evidence, of which the
  winner omits more. Replay also scores packing, not task outcomes.

Coverage headroom needs a different lever than these four parameters: Jev
coverage judgments (`coverage-jev`) to tie evidence to requirements
without an exact key, or an objective that counts how much of each
selected item arrived. Either is a new study with a new plan, not a change
to this one.

## Spend

The study made no model or Jev call and spent $0.00. It took 57 seconds
of local work: 60 ms to read the briefings, 266 ms to propose (including
the climb's evaluations), 1.6 s for the search rung, 0.6 s for selection,
54 s for 104 mini-task episodes, and 13 ms for confirmation. Each
candidate's own replay time is in the result.
