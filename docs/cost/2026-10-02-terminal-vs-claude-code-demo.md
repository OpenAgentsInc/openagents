# Demo: the same task in OpenAgents Terminal and in Claude Code

[#10211](https://github.com/OpenAgentsInc/openagents/issues/10211), part of
[#10204](https://github.com/OpenAgentsInc/openagents/issues/10204). A
reproducible side-by-side: three real repository fixes, each run at the same
time in OpenAgents Terminal (routed) and in raw Claude Code, from the same
commit in two scratch checkouts, with the measured cost, time, and pass
printed from the run records at the end.

**Result of the dry run (two runs, 2026-10-02, coderos-4080): OpenAgents
Terminal passed 3 of 3 both times, as did Claude Code, at 0.68× and 0.63× of
Claude Code's cost, and took 1.61× and 1.46× as long.** Cheaper, not faster.

## What runs

| Side | Command | Configuration |
| --- | --- | --- |
| Left: OpenAgents Terminal | `openagents chat send` | Routed to Claude Code as one lean session, the shipped Claude route ([#10246](https://github.com/OpenAgentsInc/openagents/issues/10246), `coder.claude session`): Jev's briefing, six tools, the headless core system prompt, Opus 5.5 at medium effort. |
| Right: Claude Code | `claude -p` | Claude Code's own defaults (Opus 5.5, 1M context, its own system prompt and tools). |

Same engine and model family on both sides, so the difference is what
OpenAgents does around Claude Code, not a cheaper model. The routed default
with every agent on picks Codex first, which costs less still (0.32× raw
Claude Code in the standing study) but takes 1.90× as long; the lean Codex
session ([#10250](https://github.com/OpenAgentsInc/openagents/issues/10250))
is built but not yet the default Codex route and has no standing-study rows,
so the demo stays Claude against Claude.

**Tasks**: the three real repository fixes in the standing study's task set
(`efficiency-v1`; merged after the models' training cutoff, asked as an
issue, checked with the fix commit's own tests):

| Task | Repository | Standing study medians (3 trials each): lean session against raw Claude Code |
| --- | --- | --- |
| `bottle-etag` | bottle, quoted ETags and a multi-value If-None-Match | cost 0.52×, time 0.74× |
| `mi-seekable` | more-itertools, `seekable(maxlen=0)` drops items | cost 0.53×, time 1.06× |
| `mi-one` | more-itertools, `one()`/`only()` with a falsy custom exception | cost 0.48×, time 1.67× |

All three passed 3/3 on both sides in the study, and the lean session cost
about half on each. They are the study's real repository tasks; the other
four are Terminal-Bench tasks moved onto the host (the lean session also won
on cost there, from 0.39× on `headless-terminal` to 0.76× on
`build-cython-ext`, but they read as benchmark puzzles on screen).

## Run it

On coderos-4080, from an `openagents` checkout at or after this commit, with
`openagents` and `microcoder` built (`cargo +1.97.1 build -q -p
openagents-cli --bin openagents -p microcoder --bin microcoder`):

```sh
EFFICIENCY_BIN=~/openagents/target/debug python3 bench/efficiency/demo.py start
```

It opens a tmux session (`oa-demo`, on its own server: `tmux -L oa-demo
attach -t oa-demo` to come back) with OpenAgents Terminal on the left,
Claude Code on the right, and the driver underneath. For each task both
sides start together; each pane shows the run's steps as they happen and,
when it ends, the independent check, cost, time, and tokens. After the
third task the driver prints the table and the same rows through
`openagents efficiency --study`. It takes about four minutes and about $1 at
list price (Claude Code subscription usage; not billed per run).

`demo.py drive RUN_ID` does the same without tmux (progress as lines),
`demo.py summary RUN_ID` reprints the table, and a finished run is never
repeated. The records land in `~/gym-efficiency/runs/RUN_ID/` and
`~/gym-efficiency/RUN_ID.jsonl`, the summary in `RUN_ID-summary.txt`.

Both sides go through [`study.py`](../../bench/efficiency/study.py)'s own
arms and checks, so the numbers are recorded exactly as the standing study
records them. One difference: raw Claude Code runs with `--output-format
stream-json --verbose` so its pane can show its work; cost, usage, and turns
come from the same final result event.

## Dry run, 2026-10-02

coderos-4080, `openagents` and `microcoder` at `682d958c7e` (debug build, as
the standing study's), Claude Code 2.1.286, through `demo.py start` in tmux.
Output: [run 1](2026-10-02-terminal-vs-claude-code-demo/demo-dryrun-1-drive.txt),
[run 2](2026-10-02-terminal-vs-claude-code-demo/demo-dryrun-2-drive.txt);
rows: [run 1](2026-10-02-terminal-vs-claude-code-demo/demo-dryrun-1.jsonl),
[run 2](2026-10-02-terminal-vs-claude-code-demo/demo-dryrun-2.jsonl).

| Run | Task | OpenAgents Terminal | Claude Code | Cost ratio | Time ratio |
| --- | --- | --- | --- | ---: | ---: |
| 1 | `bottle-etag` | pass, $0.188, 59 s | pass, $0.282, 46 s | 0.66× | 1.27× |
| 1 | `mi-seekable` | pass, $0.107, 40 s | pass, $0.213, 35 s | 0.50× | 1.13× |
| 1 | `mi-one` | pass, $0.135, 57 s | pass, $0.139, 15 s | 0.98× | 3.74× |
| 1 | **total** | **3/3, $0.430, 156 s** | **3/3, $0.634, 97 s** | **0.68×** | **1.61×** |
| 2 | `bottle-etag` | pass, $0.187, 69 s | pass, $0.284, 55 s | 0.66× | 1.25× |
| 2 | `mi-seekable` | pass, $0.137, 62 s | pass, $0.212, 36 s | 0.64× | 1.74× |
| 2 | `mi-one` | pass, $0.084, 31 s | pass, $0.150, 20 s | 0.56× | 1.53× |
| 2 | **total** | **3/3, $0.408, 162 s** | **3/3, $0.647, 111 s** | **0.63×** | **1.46×** |

The run-2 summary as the driver prints it:

```
task           OpenAgents Terminal          Claude Code                     cost    time
               check   cost      time       check   cost      time         ratio   ratio
bottle-etag    pass    $0.187       69 s    pass    $0.284       55 s      0.66x   1.25x
mi-seekable    pass    $0.137       62 s    pass    $0.212       36 s      0.64x   1.74x
mi-one         pass    $0.084       31 s    pass    $0.150       20 s      0.56x   1.53x
total          3/3     $0.408      162 s    3/3     $0.647      111 s      0.63x   1.46x

OpenAgents Terminal cost 37% less than Claude Code (3/3 against 3/3 passing) and took 1.46x as long.
```

## What it shows, and what it does not

- **Cost: a consistent win.** 0.63–0.68× across the two runs, in line with
  the standing study's 0.63× over all seven tasks and about 0.5× on these
  three. The routed side read fewer input tokens in 5 of the 6 pairs (46–151 k
  per run against 65–238 k for raw), from Jev's briefing, six tools, and a
  shorter system prompt. Jev itself cost under $0.001 a run.
- **Time: a loss, said plainly.** OpenAgents Terminal took 1.46–1.61× as
  long in the demo. About 2–3 s of each routed run is routing (the router,
  Jev's probes, starting the session); up to 5 s more is the harness waiting
  for the route record to settle (it polls every 5 s, as in the study). The
  rest is the session taking more turns than raw Claude Code on small fixes:
  on `mi-one` in run 1 raw Claude Code finished in 3 turns and 15 s, the
  lean session in 8 turns. In the standing study the lean session was faster on
  `bottle-etag` (0.74×) and slower on `mi-one` (1.67×); one demo run is one
  draw.
- **One trial per task per run.** The demo is for showing, not measuring:
  the bootstrap intervals `openagents efficiency` prints for it are
  degenerate (one row per task). The measurement is the standing study
  (`bench/efficiency/README.md`, openagents.com/efficiency).
- **Both sides run at once on the same machine**, so they share its load.
- **Cost is list price** from each engine's reported usage. On the owner's
  Claude Code subscription neither run is billed per run.

## For the recording

The owner's step (`NEEDS_OWNER.md` in the workspace): run the command above
in a full-width terminal on coderos-4080 (or over ssh from a Mac), record
the screen from the tmux session opening until the summary table and the
`openagents efficiency` lines are on screen, and keep the run's
`RUN_ID-summary.txt` with the video.
