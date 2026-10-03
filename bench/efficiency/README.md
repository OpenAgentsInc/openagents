# Standing efficiency study

The standing Gym study for
[#10162](https://github.com/OpenAgentsInc/openagents/issues/10162): the
same pinned tasks run through raw Claude Code, raw Codex, and OpenAgents'
routed paths, with an independent check of every run's work. It grew from
the [#10209 shadow-baseline harness](../../docs/cost/2026-10-02-shadow-baseline/study.py)
and is the source of `openagents efficiency`, the terminal's `/efficiency`,
and [openagents.com/efficiency](https://openagents.com/efficiency)
([#10210](https://github.com/OpenAgentsInc/openagents/issues/10210)).

## What it runs

- **Task set `efficiency-v1`** (pinned in [`study.py`](study.py)): four
  Terminal-Bench 2.1 tasks moved onto the host (`fix-git`,
  `fix-code-vulnerability`, `headless-terminal`, `build-cython-ext`) and
  three real repository fixes merged after the models' training cutoff
  (`mi-seekable`, `mi-one`, `bottle-etag`), each asked as an issue. The
  prompts, preparation, and checks are the #10209 study's; its
  [Method section](../../docs/cost/2026-10-02-shadow-baseline-measurement.md#method)
  describes each. Change a task, prompt, or check and change `TASKSET`.
- **Arms** (`STANDING`):
  - `raw-claude`: `claude -p` on Claude Code's own defaults.
  - `raw-codex`: `codex exec`, pinned to the routed default's model and
    effort (`gpt-6.1-sol`, medium) so that it differs from the routed arm only
    in routing. The owner's own `~/.codex/config.toml` default on
    coderos-4080 is `gpt-6-sol` at max effort, which is not what the router
    runs.
  - `routed-default`: `openagents chat send` with the shipped settings
    (every agent on, so Codex first, delegate recipe on).
  - `routed-lean`: routed to Claude Code as one lean session
    ([#10246](https://github.com/OpenAgentsInc/openagents/issues/10246),
    `coder.claude session`).
  - Also available: `routed-claude` (the Claude Microcoder loop) and
    `routed-codex` (Codex only).
- **Checks**: the task's own tests or the fix commit's own test file, run
  on the work the run left. Each fails on the untouched task and passes on
  the reference fix.
- **Rows**: one JSON line per run with the study, task set, arm, engine,
  class, binary commit, host, engine versions, cost (list price, engine plus
  Jev), wall time, tokens, and the check's result.

## Run it

On coderos-4080 (Claude Code and Codex are signed in there; check
`df -h ~` shows at least 60 GB free):

```sh
scp bench/efficiency/study.py coderos-4080:gym-efficiency/study.py
ssh coderos-4080
cd ~/gym-efficiency
# Binaries: a folder with `openagents`, `microcoder`, and a COMMIT file.
EFFICIENCY_BIN=~/gym-efficiency/bin nohup python3 study.py standing RUN_ID 3 4 \
  > standing-RUN_ID.log 2>&1 &
```

`standing RUN_ID TRIALS PARALLEL` prepares the sources and templates (the
first time: it clones bottle and more-itertools, and copies `fix-git`'s
checkout out of `alexgshaw/fix-git:20260403` with docker or podman, or
from `EFFICIENCY_SRC`), runs every standing arm on every task for each
trial in a shuffled order, and writes `~/gym-efficiency/RUN_ID.jsonl`.
A finished run is never repeated: rerunning the command resumes. `recheck
RUN_ID TASK` reruns one task's checks on the kept work, never the agents.

To publish a run:

1. Copy `~/gym-efficiency/RUN_ID.jsonl` to `bench/efficiency/results/`.
2. Add it to `PUBLISHED` in
   [`crates/coder/src/efficiency.rs`](../../crates/coder/src/efficiency.rs).
   The latest entry is the one the terminal and the page lead with.
3. Check it: `openagents efficiency` (or `--study FILE` before committing).
4. Commit, and deploy the site
   ([docs/deployment/openagents-web.md](../../docs/deployment/openagents-web.md)).

On GCE pool hosts instead (`openagents cloud up --hosts 1`, then ssh as
`coder` per [docs/cloud/gce-pool.md](../../docs/cloud/gce-pool.md)): the
host builds origin/main's `openagents` and `microcoder`, but neither engine
is signed in there, and it has no Terminal-Bench checkout. Sign in to
Claude Code and Codex on the host, set `EFFICIENCY_TB` to a Terminal-Bench
2.1 `tasks` folder, and run the same command. The host costs about $0.17 an
hour on spot.

## Side-by-side demo

[`demo.py`](demo.py) runs three of these tasks (`bottle-etag`,
`mi-seekable`, `mi-one`) at once in OpenAgents Terminal (the lean session)
and raw Claude Code from the same commit, in a tmux split, and prints the
measured difference from the run records
([#10211](https://github.com/OpenAgentsInc/openagents/issues/10211); runbook
and dry-run numbers:
[docs/cost/2026-10-02-terminal-vs-claude-code-demo.md](../../docs/cost/2026-10-02-terminal-vs-claude-code-demo.md)).

## What it costs

The first standing run, `2026-10-03` (84 runs: 7 tasks, 4 arms, 3 trials,
binaries at `6b4dbe827d`, coderos-4080, 4 runs at a time), cost **$13.15**
at list price: raw Claude Code $5.66, lean session $3.55, raw Codex $2.15,
routed default $1.79. It took 1.7 hours of agent time and about half an
hour of wall time. Claude usage comes from the owner's Claude Code
subscription and Codex usage from the Codex login, so list price is not
what is billed.

## Results, 2026-10-03

Rows: [`results/2026-10-03.jsonl`](results/2026-10-03.jsonl). Every run of
every arm passed its check (21/21 each; Wilson 95% interval 85%–100%).
Medians and ratios with 95% bootstrap intervals, from `openagents
efficiency`:

| Arm | Cost per checked result | Time to a checked result (median) | Cost against raw Claude Code | Time against raw Claude Code |
| --- | ---: | ---: | --- | --- |
| Raw Claude Code | $0.270 | 42 s | 1 | 1 |
| Raw Codex | $0.102 | 38 s | 0.38 (0.36–0.40) | 1.17 (1.09–1.26) |
| Routed default (Codex, recipe on) | **$0.085** | 79 s | **0.32 (0.30–0.33)** | 1.90 (1.78–2.04) |
| Lean Claude Code session | $0.169 | 44 s | 0.63 (0.56–0.71) | 1.34 (1.19–1.53) |

Against raw Codex on the same model and effort, the routed default cost
0.83 (0.79–0.88) and took 1.62 (1.52–1.73) as long. **Routing wins on cost
and loses on time**. In the #10209 study the gap was the Microcoder loop's
separate model calls, plus 3 to 4 seconds of routing before the engine
starts; this run did not break the time down.
Most of the cost win against raw Claude Code is Codex's lower price; the
part routing adds on the same engine is 17%.

## Follow-up: the lean session's speed (#10254)

The lean session was cheaper than raw Claude Code but slower (1.34× above,
1.46–1.61× in the [demo](../../docs/cost/2026-10-02-terminal-vs-claude-code-demo.md)).
The trajectories showed why:

- **The headless core asked for more work than raw Claude Code does.** Its
  `verify` section says "Test your change even when nobody asked you to".
  On `mi-one` the session fixed the bug in 3 calls, as raw did, then wrote
  regression tests, reverted the fix to watch them fail, and reapplied it:
  8 turns against raw's 3. On `build-cython-ext` it cross-checked Cython
  against pure Python and cleaned up build folders, 22–33 turns against
  17–22, and in one run waited 131 s on a `python3 -` that read standard
  input. That task alone was the whole time gap in the sum of means.
- **The survey cost 4–7 s before the session started** (6–9 sequential
  Jev requests), as long as raw Claude Code took for a whole small fix,
  and the session read the files again anyway.
- **The harness polled the route record every 5 s**, adding up to 5 s to
  each routed run's wall time that raw runs never paid.
- Effort was not the main cause: per-turn API time was close on most
  tasks (3–6 s a turn on both arms). The six tools were not either: both
  arms mostly used Bash.

The changes: the session's system prompt is now the `lean-session` preset
(`crates/coder-delegate/src/system.rs`). It is the core with `verify`
replaced by `finish` (run the named checks, check requirements as a fresh
shell sees them, then stop without adding tests or checks) plus `pace` (few
steps, parallel tool calls, act when ready, no commands that wait for
input). The recipe skips the workspace survey when the first route is the
lean Claude session (`Input::survey`): the briefing is the request with
Jev's class, knowledge, and frozen checks. The Codex session and every
other route keep the core and the survey. `study.py` polls every 0.5 s.
Effort stays medium and the tools stay six.

Measured on coderos-4080 against raw Claude Code in the same run, 7 tasks ×
3 trials, 4 at a time (rows in
[`docs/cost/2026-10-02-lean-session-speed/`](../../docs/cost/2026-10-02-lean-session-speed/);
ratios from `openagents efficiency --study`, 95% bootstrap intervals):

| Lean session | n per arm | Passed (lean / raw) | Cost against raw | Time against raw | Mean turns (lean / raw) |
| --- | ---: | --- | --- | --- | --- |
| Before (`2026-10-03`, above) | 21 | 21/21 / 21/21 | 0.63 (0.56–0.71) | 1.34 (1.19–1.53) | 8.9 / 8.9 |
| Prompt only (`a-prompt`) | 21 | 21/21 / 21/21 | 0.53 (0.46–0.61) | 1.12 (0.87–1.45) | 7.6 / 9.7 |
| Prompt and no survey, first wording (`b-prompt-nosurvey`) | 21 | 20/21 / 21/21 | 0.45 (0.42–0.50) | 0.91 (0.72–1.12) | 6.7 / 9.8 |
| **Shipped (`final`)** | 21 | **21/21 / 21/21** | **0.42 (0.38–0.47)** | **0.99 (0.76–1.27)** | **6.6 / 9.9** |

The `b` failure was a `build-cython-ext` run that made the snippet work
only with an `LD_LIBRARY_PATH` set in its own commands, which the checker
does not have; `finish` now says to check as a fresh shell would. Per
task in the shipped run, the session was faster on `bottle-etag` (0.72),
`build-cython-ext` (0.87), and `mi-seekable` (0.95), and slower on the
small ones: `fix-git` 1.36, `fix-code-vulnerability` 1.41, `headless-terminal`
1.26, `mi-one` 1.61, with as many turns as raw or fewer. What is left on a
small fix is the route's own start: the chat router (an offer the harness
accepts, 3–10 s), Jev's class and checks (2–4 s), and the session's
startup, against a 15–20 s raw run.

Caveats: coderos-4080 was loaded by another job during these runs (load
average 40–180, disk briefly full; run `d` was discarded for that). Raw and
routed runs interleave in one run, so both arms shared it. An extension
to 5 trials was stopped when the disk fell under 12 GB; its 17 rows
(`final-extension-aborted.jsonl`, both arms failed one `headless-terminal`
trial) are kept and not counted. The standing run `2026-10-03b` below
publishes these settings with the start-up cut.

## Follow-up: the routed start-up (#10279)

After #10254 the lean session itself was as fast as raw Claude Code, but
a routed small fix still spent 5 to 10 s (median 7.9 s) outside the
Claude Code session, three Jev round trips in a row before it started:
the chat router's judgment, then Jev's issue judgment (does the message
ask to work a GitHub issue?), then the recipe's class, knowledge search
and keep, and checks, one after another. On a 15 to 20 s raw run that was
the whole gap. The changes:

- The chat starts Jev's issue judgment when the message is sent, beside
  the router's judgment, and a start uses it for the same message.
- When a run would begin on the lean Claude Code session, the chat also
  prepares the recipe's groundwork for the exact handoff prompt beside the
  router (`coder_delegate::recipe::Ahead`, kept in the task store). The
  run takes it, copies its Jev steps into its own record so their cost is
  the run's, and asks Jev itself when the prompt differs.
- Without the survey, the class, knowledge, and checks go to Jev at once.
- A whole agent's frozen checks run after its turn only when no
  independent check (#10232) runs them on the candidate right after; they
  ran twice.

Time outside the session (wall time less the session's own) on the four
small fixes fell from a median of 7.9 s (mean 9.8 s, 12 runs, `leanspeed-e`
trials 1 to 3) to 2.2 s (mean 2.5 s, 20 runs, `b-shipped`): the router's
own judgment (about 1.4 s) and the route settling. Measured on coderos-4080
against raw Claude Code in the same run, 5 trials, 2 at a time (rows in
[`docs/cost/2026-10-03-routed-startup/`](../../docs/cost/2026-10-03-routed-startup/),
95% bootstrap intervals from its `ci.py`):

| Small fix | Before (`leanspeed-e`, n=3 per arm): time / cost | Shipped (`b-shipped`, n=5 per arm): time / cost | Passed (lean / raw) |
| --- | --- | --- | --- |
| `fix-git` | 1.36 (1.22–1.47) / 0.46 (0.40–0.53) | 1.39 (1.23–1.58) / 0.47 (0.44–0.50) | 5/5 / 5/5 |
| `fix-code-vulnerability` | 1.41 (0.94–2.07) / 0.45 (0.43–0.47) | 1.34 (1.17–1.54) / 0.47 (0.44–0.51) | 5/5 / 5/5 |
| `mi-one` | 1.61 (1.35–1.99) / 0.35 (0.31–0.38) | 0.83 (0.63–1.08) / 0.30 (0.26–0.33) | 5/5 / 5/5 |
| `headless-terminal` | 1.26 (1.04–1.67) / 0.56 (0.51–0.63) | 0.72 (0.54–0.99) / 0.46 (0.36–0.58) | 5/5 / 5/5 |
| Sum of means | 1.36 (1.19–1.59) / 0.47 (0.45–0.49) | **0.91 (0.78–1.07) / 0.43 (0.39–0.48)** | 20/20 / 20/20 |

An earlier run with the concurrency but not the single check run
(`a-concurrent`, n=5) gave 1.06, 1.62, 1.04, and 1.06 on the same four,
1.13 summed: per-task means of five runs move by 0.3 between runs. Pooled
over both runs (n=10 per arm): `fix-git` 1.20 (0.96–1.48),
`fix-code-vulnerability` 1.49 (1.19–1.87), `mi-one` 0.94 (0.69–1.33),
`headless-terminal` 0.87 (0.71–1.05), 1.02 (0.90–1.15) summed, cost 0.45.
`mi-one` and `headless-terminal` meet the 1.1 target; `fix-git` and
`fix-code-vulnerability` do not, and what is left there is inside the
session, not before it: on `fix-git` the session alone took 19–27 s
against raw's 16–19 s for the same 6 turns, and on `fix-code-vulnerability`
the session runs the frozen `pytest` itself (5 turns against raw's 3 to 5)
and the host runs it before the session and in the independent check
(about 1.5 s more outside it). Start-up is no longer the cause.

## Results, 2026-10-03b

The standing study after the start-up cut, at `0565629714` (84 runs: 7
tasks, 4 arms, 3 trials, coderos-4080, 4 at a time). Rows:
[`results/2026-10-03b.jsonl`](results/2026-10-03b.jsonl). Every run of every
arm passed its check (21/21 each). From `openagents efficiency`:

| Arm | Cost per checked result | Time to a checked result (median) | Cost against raw Claude Code | Time against raw Claude Code |
| --- | ---: | ---: | --- | --- |
| Raw Claude Code | $0.297 | 41 s | 1 | 1 |
| Raw Codex | $0.110 | 37 s | 0.37 (0.33–0.41) | 1.11 (0.97–1.26) |
| Routed default (Codex, recipe on) | **$0.094** | 75 s | **0.32 (0.28–0.35)** | 1.83 (1.61–2.07) |
| Lean Claude Code session | $0.133 | **34 s** | 0.45 (0.39–0.52) | **0.86 (0.74–0.98)** |

The lean session is now faster than raw Claude Code at under half its
cost. The routed default still loses on time (1.65× raw Codex on the same
model): its start-up is shorter by the issue judgment only, since the
groundwork prepared ahead is the lean session's (the Codex route keeps the
survey), and most of its gap is the Microcoder loop's own model calls.
The run cost $13.33 at list price.

## Schedule

Manual, not weekly. A run is cheap at list price (about $13), but it spends
the owner's own Claude Code subscription and Codex login, which exist only
on coderos-4080, the owner's working machine; a timer there would compete
with real work and the subscription's limits, and the GCE pool has no
engine logins. Run it after a change to routing, the
delegate recipe, or an engine's defaults, and publish the rows.
