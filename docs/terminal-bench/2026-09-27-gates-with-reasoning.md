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
  (`crates/microcoder/src/claude.rs`). The provider runs the `claude`
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
  tokens. Opus costs more per step at list price than Luna, so the cap may
  end runs earlier than in Round 4; the cap stands, and a run it ends is
  reported as ended by cost.
- **Runs, arms, tasks, caps, knowledge, records, reporting:** as declared.
  The records go under `~/gates-reasoning-runs/runs/` on coderos-4080 next
  to the kept first-attempt faults.
- **Reading the result:** a step that fails because Claude Code refuses or
  rate-limits the login is a provider fault, reported as such, not a result.
