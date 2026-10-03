# Results: Codex and System One on new tasks (#10356)

Run `validate-10356` under the [frozen protocol](protocol.md) (committed in
65175b1e6c before any agent ran): six Terminal-Bench 2.1 tasks never used in
tuning, four arms, three trials, 72 runs, coderos-4080, binaries at
65175b1e6c, Claude Code 2.1.288, codex-cli 0.159.2. Rows:
[`validate-10356.jsonl`](validate-10356.jsonl); analysis:
[`analyze.py`](analyze.py) → [`results.json`](results.json). Every run is
kept; none was repeated.

## Headline

On new tasks, **routing still cuts cost at equal quality, and still loses
time on Codex**:

- **Lean Claude session vs bare Claude Code: 0.58× cost (0.48–0.66), 18/18
  vs 18/18, time 1.14× (0.74–2.04, not distinguishable from equal).** The
  cost win validates on unseen tasks; the familiar-task time win (0.86×)
  does not reproduce here, but neither does a loss.
- **Routed Codex vs bare Codex: 0.75× cost (0.59–1.08, not significant),
  2.29× time (1.89–2.55), 17/18 vs 18/18.** Routing Codex through the
  Microcoder loop is clearly slower and not clearly cheaper on these tasks.
  The time is inside the engine (median 73 s engine vs 2.0 s outside it),
  not the router or Jev.
- **Cheapest checked result: routed Codex, $0.091** (bare Codex $0.115,
  lean Claude $0.210, bare Claude Code $0.363). **Fastest: bare Claude
  Code**, median 22 s.

## By arm

| Arm | Passed (Wilson 95%) | Total cost | Cost per checked result | Median wall | Median outside engine |
|---|---|---|---|---|---|
| raw-codex (gpt-6.1-sol, medium) | 18/18 (82%–100%) | $2.06 | $0.115 | 42.8 s | – |
| routed-codex (System One, Codex only) | 17/18 (74%–99%) | $1.55 | $0.091 | 75.2 s | 2.0 s |
| raw-claude (Claude Code defaults) | 18/18 (82%–100%) | $6.53 | $0.363 | 21.9 s | – |
| routed-lean (lean Claude session) | 18/18 (82%–100%) | $3.79 | $0.210 | 27.1 s | 2.0 s |

## Ratios (sums over matched task × trial pairs, 95% bootstrap)

| Ratio | Cost | Wall time |
|---|---|---|
| routed-codex / raw-codex | 0.75 (0.59–1.08) | 2.29 (1.89–2.55) |
| routed-lean / raw-claude | **0.58 (0.48–0.66)** | 1.14 (0.74–2.04) |
| raw-codex / raw-claude | 0.32 (0.21–0.49) | 0.60 (0.41–1.66) |
| routed-codex / raw-claude | 0.24 (0.19–0.30) | 1.37 (0.94–3.16) |
| routed-lean / raw-codex | 1.84 (1.23–2.92) | 1.90 (0.98–3.29) |

By the protocol's decision rule: **validated** — lean session cost against
bare Claude Code. **Loss** — routed Codex time against bare Codex. **No
detectable difference** — routed Codex cost against bare Codex, lean
session time against bare Claude Code.

## Per task (passed / median wall)

| Task | raw-codex | routed-codex | raw-claude | routed-lean |
|---|---|---|---|---|
| constraints-scheduling | 3/3, 32 s | 3/3, 76 s | 3/3, 19 s | 3/3, 26 s |
| distribution-search | 3/3, 45 s | 3/3, 68 s | 3/3, 43 s | 3/3, 37 s |
| log-summary-date-ranges | 3/3, 32 s | 3/3, 56 s | 3/3, 18 s | 3/3, 23 s |
| polyglot-c-py | 3/3, 64 s | 3/3, 112 s | 3/3, 30 s | 3/3, 28 s |
| regex-log | 3/3, 32 s | 3/3, 60 s | 3/3, 18 s | 3/3, 19 s |
| schemelike-metacircular-eval | 3/3, 471 s | **2/3**, 1300 s | 3/3, 1139 s | 3/3, 893 s |

The one failure: routed Codex, `schemelike-metacircular-eval` trial 3,
finished normally after 13 steps; its evaluator passed 62 of the task's 63
Scheme programs. The long task dominates the time sums, which is why the
time intervals are wide.

## Phase timing

Routed arms spend a median 2.0 s outside the engine (router, Jev class and
checks, startup) — the #10279 start-up cut holds on new tasks. Jev's own
cost is small: $0.029 across 18 routed-Codex runs, $0.001 across 18 lean
runs. Routed Codex's extra time is the Microcoder loop's step-by-step
requests to Codex (engine median 73 s against bare `codex exec`'s 43 s
whole run), consistent with #10250's finding that the loop, not a session,
is the routed Codex default.

## Limitations and unmeasured costs

- Small n (18 runs per arm, 6 tasks); all tasks are Terminal-Bench medium,
  so this says little about hard tasks (see `efficiency-hard-v1`).
- Costs are list-price estimates; Claude Code and Codex ran on the owner's
  subscriptions, so this is not what is billed. Host time and power, the
  orchestrating agent's own model use, and engineering time are unmeasured.
- coderos-4080 also ran other work during the run; runs of all arms
  interleaved in the same shuffled order, so load affected them alike.
- `largest-eigenval` was replaced before any agent ran because its reference
  solution failed a wall-clock test on this host (protocol.md).

## What this means

Ship as is for Claude: the lean session's cost win is real on unseen tasks.
For Codex, the routed loop buys at best a modest, uncertain cost cut for
more than double the time; the next experiment is the Codex session route
(`coder.codex session`, #10250) or bare `codex exec` behind the router on
this same frozen panel, under a new set name.
