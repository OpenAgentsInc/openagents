# Frozen protocol: Codex and System One on new tasks (#10356)

Frozen and committed before the first agent run. Nothing below changes after
the run starts; any change means a new task set name and a new run.

## Question

On tasks never used to tune routing, prompts, briefings, or thresholds, does
OpenAgents' routed path (System One / Jev recipe in front of the engine)
change pass rate, cost, or time against the same engine run bare? The
standing study (`efficiency-v1`) answered this on familiar tasks; this panel
validates it on new ones and adds the Codex comparison #10356 left open.

## Tasks: `efficiency-validate-v1`

Six Terminal-Bench 2.1 tasks, none used in any earlier study, tuning run,
demo, or golden in this repository:

| Task | TB difficulty | What it asks |
| --- | --- | --- |
| `regex-log` | medium | write a regex for dates on lines with an IPv4 address |
| `log-summary-date-ranges` | medium | summarize generated logs by date range and severity into a CSV |
| `distribution-search` | medium | find a probability distribution with given KL divergences |
| `polyglot-c-py` | medium | one file that is both valid C and valid Python with the same output |
| `schemelike-metacircular-eval` | medium | a metacircular evaluator in the task's Scheme dialect |
| `constraints-scheduling` | medium | schedule a meeting from three calendars under constraints |

`largest-eigenval` was chosen first and dropped before any agent ran: its
reference solution failed a wall-clock speedup test on the loaded host, so
its check was not reliable.

Each task runs in a fresh git repository holding what the task's image puts
in `/app`. The prompt is the task's own instruction, with `/app` read as the
work tree and the Python environment named (`bench/efficiency/study.py`,
`VALIDATE`, `app_prompt`).

## Independent checks

The task's own `tests/test_outputs.py`, with `/app` mapped to the work tree
and `/tests` to a copy of the task's tests folder, run with the run's own
Python environment after the agent finishes (`check_validate`). Calibrated
before freezing (`study.py calibrate-validate`, evidence in
[`calibration.json`](calibration.json)): every check fails on the untouched
task and passes on the task's reference solution (`solution/solve.sh`).

## Arms (fixed)

| Arm | What runs |
| --- | --- |
| `raw-codex` | `codex exec`, gpt-6.1-sol, medium effort, bypass approvals |
| `routed-codex` | `openagents chat send --local`, Codex only, delegate recipe (System One) on |
| `raw-claude` | `claude -p`, Claude Code's own defaults |
| `routed-lean` | `openagents chat send --local`, Claude Code as one lean session, recipe on |

Binaries: `openagents` and `microcoder` built from the commit that freezes
this protocol, in `~/gym-efficiency/bin-10356` on coderos-4080, with a
`COMMIT` file recorded in every row.

## Trials and order

Three trials per arm per task: 6 × 4 × 3 = **72 runs**, 4 at a time, in the
harness's fixed shuffled order (seed 10162). One hour timeout per run. Every
run is kept, including timeouts, errors, and failed checks; no run is
repeated or dropped after it starts.

## Metrics

- **Pass rate** per arm, with Wilson 95% intervals.
- **Cost per checked result**: total list-price cost of all runs of the arm
  (failures included) divided by passing runs. Engine cost from the engine's
  own usage report; routed arms add Jev and embedding cost from the trace.
- **Wall time**: median per arm, and median time to a checked result.
- **Ratios** routed/raw on the same engine and each arm against raw Claude
  Code, as sums over matched (task, trial) pairs with 95% bootstrap intervals
  (2,000 resamples over tasks × trials).
- **Phase timing** for routed arms where the trace records it (routing,
  recipe, engine).

## Unmeasured costs (listed, not estimated)

Subscription billing (Claude Code and Codex run on the owner's logins; list
price is not what is billed), host time and power on coderos-4080, the
orchestrating agent's own model usage, and engineering time.

## Decision rule

Routing "validates" on a dimension if the routed/raw ratio's 95% interval
excludes 1 in its favor with no drop in pass rate beyond one run per arm;
otherwise the result is reported as no detectable difference or a loss.
