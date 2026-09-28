# End-of-run checks with reasoning on: declaration and results

2026-09-27. Issue [#9717](https://github.com/OpenAgentsInc/openagents/issues/9717),
epic [#9680](https://github.com/OpenAgentsInc/openagents/issues/9680).

## Why

The [Round 2 loop report](2026-09-26-round2-loop.md) found that no
end-of-run check raised Microcoder's pass rate on the development tasks:
base 0 of 8, every mechanism 0 of 8. Every GPT-6 Luna run in it used the
Codex login before `ac3ee05901`, so Luna did no reasoning on any step
([route diff](2026-09-26-route-diff.md)). This experiment repeats that
comparison with reasoning on. The failure the checks target is a green
ending on a wrong answer: the run's own tests pass while the grader fails.

## Declaration

Written and pushed before any run of this experiment started.

- **Binary:** `microcoder-study-r4` on coderos-4080, built from
  `ac3ee05901`: the Round 4 build, with reasoning on the Codex route.
- **Model:** GPT-6 Luna through the Codex login (`--provider codex`,
  `list_price`), `--effort xhigh`, as in Round 4.
- **Tasks:** the Round 2 loop's four excluded development tasks, and only
  those: `sound-change-cascade`, `batched-eval-parity`, `fin-saccr-rwa`, and
  `hof-topology-interpenetration`. No held-out or Fable-fails task, file,
  transcript, or record is opened.
- **Arms:** `base`, with none of the new flags, and `full`, with
  `--oracle --gate-requirements --gate-credible --gate-target --adversarial 1`
  (the Round 2 loop's "full" arm, `--doubt-threshold` at its 0.9 default).
- **Runs:** 2 per arm per task, 16 in all, at most 4 at a time, the two arms
  of a task started together.
- **Caps, both arms:** 200 steps, $2.00, 90 minutes, as in Round 4. The
  Round 2 loop's 60 steps and 60 minutes are not used, because its full arm
  ended at the step limit in 7 of 8 runs and xhigh steps are slower.
- **Knowledge:** `--kb candidates`, `OPENAGENTS_KNOWLEDGE` set to a new
  empty local folder, so only entries already synced from
  `wss://relay.openagents.com` appear. Each run's `summary.json` records the
  entries it was shown, with their digests, and the retrieval mode.
- **Records:** each run gets its own `--run-dir` under
  `~/gates-reasoning-runs/` on coderos-4080, outside
  `~/.openagents/microcoder/runs`, so the study's scans don't pick them up.
- **Reported per arm:** passes, grader tests passed, mean and total cost
  (unknown cost stays unknown), mean time, how each run ended, and which
  checks sent a run back.
- **Reading the result:** with 8 runs per arm, this can show a large effect
  only. A difference of one or two passes is not evidence either way. A
  task that passes in both arms does not show the checks helped.

## Results

**Not yet run: the Codex login is out of quota until 2026-10-03 18:07 UTC.**

The first attempt started at 2026-09-27 09:33 CDT with the declared
configuration. All 16 runs ended within 2 minutes as provider faults:
every GPT-6 Luna call returned HTTP 429 `usage_limit_reached` (plan `pro`,
10,080-minute window, resetting at Unix time 1791050823). No model step
succeeded, so no run is a result. Each spent only about $0.0005, for Jev and
embeddings. The records are kept in
`~/gates-reasoning-runs/attempt1-faults/` on coderos-4080.

TB4 Round 4 at `xhigh` most likely used the week's allowance. The
declaration above stands unchanged. Run it after the reset, or earlier if
another route to GPT-6 Luna gets credit. A different route is a
configuration change and must be recorded here before it runs.

## Configuration change: Claude in place of GPT-6 Luna

Recorded and pushed before any run of the second attempt started. The
Codex login stays out of quota until 2026-10-03, and no other route to
GPT-6 Luna has credit, so the second attempt changes the model. It
therefore answers a different question from the declaration: whether the
end-of-run checks raise Claude's pass rate on these four tasks, with
reasoning on. It says nothing about Luna. Everything not listed here stands
as declared.

- **Binary:** `microcoder-study-claude` on coderos-4080, built from
  `b7238446f4`: `main` at `4865afd687` plus the `claude` provider
  (`crates/microcoder-loop/src/claude.rs`). The provider runs the `claude`
  binary once per step in print mode with every tool off, one turn, no
  settings files, and no saved session, and asks for the same `next_action`
  JSON schema the Codex route sends as its output format.
- **Model:** Claude through the operator's Claude Code login on
  coderos-4080 (`--provider claude --model opus`, alias for the canonical
  `claude-opus-5-5` the binary reported in a smoke test), `--effort xhigh`,
  which Claude Code accepts as an effort level.
- **Cost:** Claude Code's `total_cost_usd`, its list-price figure for the
  call, recorded as `list_price`. The $2.00 cap applies to that figure plus
  Jev and embeddings, as before. Because each step is a new process, no
  prompt cache carries between steps; cache-creation tokens count as input
  tokens.
- **Spend cap:** `--max-usd 6.00` per run in place of $2.00, so the 16 runs
  stay under about $100 of list-price figures a day, the ceiling the
  operator set for the Claude subscription. Opus costs more per step at
  list price than Luna, and $2.00 would have ended most runs early. The
  step and time caps stand at 200 steps and 90 minutes. A run the cap ends
  is reported as ended by cost.
- **Pilot before the 16:** the operator asked for a small sanity check
  first. The second attempt starts with one pilot pair, `sound-change-cascade`
  base and full, one run each, and the remaining 14 runs start only after
  the pilot shows the Claude route works end to end (steps produce
  `next_action` output, cost and model are recorded, the run ends for a
  task reason and not a provider fault). The pilot runs count toward the
  declared two runs per arm for that task; they are not extra runs.
- **Runs, arms, tasks, knowledge, records, reporting:** as declared.
  The records go under `~/gates-reasoning-runs/runs/` on coderos-4080 next
  to the kept first-attempt faults.
- **Reading the result:** a step that fails because Claude Code refuses or
  rate-limits the login is a provider fault, reported as such, not a result.

## Second attempt results: Claude, base arm complete, full arm held

Nine runs finished on coderos-4080 between 11:26 and 13:35 CDT, all served
by `claude-opus-5-5`, with no provider faults. Records are under
`~/gates-reasoning-runs/runs/<task>.<arm>.<n>/` there; `outcomes.txt` in
the parent directory is the run ledger, read through
`openagents study outcomes coderos ~/gates-reasoning-runs`.

| Run | Reward | Steps | Time | List price | Ending |
|---|---:|---:|---:|---:|---|
| batched-eval-parity.base.1 | 0 | 21 | 14:37 | $6.64 | spend limit |
| batched-eval-parity.base.2 | 0 | 11 | 22:48 | $6.23 | spend limit |
| fin-saccr-rwa.base.1 | 1 | 11 | 32:13 | $6.37 | spend limit |
| fin-saccr-rwa.base.2 | 0 | 10 | 30:36 | $6.13 | spend limit |
| hof-topology-interpenetration.base.1 | 1 | 12 | 20:57 | $5.37 | finished |
| hof-topology-interpenetration.base.2 | 1 | 15 | 24:36 | $6.12 | spend limit |
| sound-change-cascade.base.1 | 1 | 23 | 22:22 | $5.84 | finished |
| sound-change-cascade.base.2 | 1 | 21 | 19:10 | $5.14 | finished |
| sound-change-cascade.full.1 | 0 | 13 | 35:10 | $6.83 | spend limit |

- **Base arm: 5 of 8 passed** (batched-eval-parity 0/2, fin-saccr-rwa 1/2,
  hof-topology-interpenetration 2/2, sound-change-cascade 2/2). Six of the
  eight ended at the $6.00 cap; the reward is the verifier's grade of the
  workspace as the run left it, and a cap ending is reported as ended by
  cost.
- **Full arm: held after one run.** The pilot's full run reached only step
  13 before the cap, at about twice the base arm's list price per step,
  and passed nothing. Launching the other seven full runs at the same cap
  would most likely end each at the cap before the checks could act, so
  the operator held the arm. The declared comparison (does the full arm
  raise the pass rate?) is therefore **unanswered** for Claude: 0 of 1 is
  not a full-arm result.
- **What this does say:** Opus 5.5 with reasoning, on the base loop, passes
  three of the four development tasks at least once at a $6.00 list-price
  cap. It says nothing about GPT-6 Luna, whose declared re-test waits on the
  Codex quota reset (2026-10-03 18:07 UTC), and nothing about the checks.
- **Spend:** $54.67 of list-price figures across the nine runs.
