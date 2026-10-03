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

## Schedule

Manual, not weekly. A run is cheap at list price (about $13), but it spends
the owner's own Claude Code subscription and Codex login, which exist only
on coderos-4080, the owner's working machine; a timer there would compete
with real work and the subscription's limits, and the GCE pool has no
engine logins. Run it after a change to routing, the
delegate recipe, or an engine's defaults, and publish the rows.
