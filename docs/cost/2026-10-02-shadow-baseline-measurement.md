# Shadow baseline, first measurement: routed OpenAgents against raw Claude Code

2026-10-02, for [#10209](https://github.com/OpenAgentsInc/openagents/issues/10209)
(part of the umbrella [#10204](https://github.com/OpenAgentsInc/openagents/issues/10204)).
The plan was the cost audit's
[section 5b](2026-10-02-system-one-cost-efficiency-audit.md#5b-measuring-the-recipe).
This is the first test of the claim "routing work beats raw delegation"
on the path the terminal ships (`openagents chat send`, routed to a Coder
task), not on a bench arm.

## Result

**On these seven tasks, the shipped routed path did not save money or time
against raw Claude Code. On the Claude engine it cost 68% more and took 39%
longer at the same pass rate.** The Codex engine cost 13% less than raw
Claude Code, which is a cheaper model's list price, and took 2.6 times as long.
The delegate recipe (#10208) had no measurable effect on the Claude engine.
On Codex it raised cost by 61% and time by 43%, because Jev classed 18 of 21
tasks as "hard", which runs Codex at high effort.

The cause on Claude is caching, not tokens. The routed loop sent 3.7 times
fewer input tokens than raw Claude Code, but it wrote 85–88% of them to the
prompt cache at 1.25 times the input price and read only 12–15% back. Raw
Claude Code read 93% of its input from cache at 0.1 times the price. Each
Microcoder step is a fresh `claude -p` call, and the step's prompt changes near
its start, so the five-minute cache almost never hits.

The audit's 61% and 63% savings (sections 1 and 2c) came from a different
path: the Coder One CLI delegate (Jev probes in front of a lean Claude Code
session with six tools) on Harbor. That path was not run here, so this
measurement does not refute those numbers. It shows that the routed path the
terminal uses today does not reproduce them.

## Numbers

105 runs: 7 tasks × 5 arms × 3 trials, all on coderos-4080 on 2026-10-02.
Passes come from an independent check (below). Cost is list price for the
engine plus Jev. Wall time is end to end, from the command's start to the
route record settling (routed) or to the process exiting (raw).

| Arm | n | Passed (Wilson 95%) | Total cost | Median cost per run | Total wall time | Median wall time per run | Median input tokens |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| Raw Claude Code (`claude -p`, defaults: Opus 5.5 1M) | 21 | 21/21 (85%–100%) | **$6.24** | $0.238 | **24.3 min** | **45 s** | 194,547 |
| Routed, Claude engine, recipe on | 21 | 21/21 (85%–100%) | $10.52 | $0.285 | 33.9 min | 64 s | 52,452 |
| Routed, Claude engine, recipe off | 21 | 20/21 (77%–99%) | $10.00 | $0.321 | 35.7 min | 84 s | 51,390 |
| Routed, Codex engine (GPT-6.1 Sol), recipe on | 21 | 21/21 (85%–100%) | $5.45 | $0.119 | 63.6 min | 124 s | 58,564 |
| Routed, Codex engine, recipe off | 21 | 21/21 (85%–100%) | **$3.38** | $0.104 | 44.6 min | 99 s | 43,621 |

Each arm compared with raw Claude Code: the sum over tasks of per-task means,
with a 95% bootstrap interval that resamples trials within each task.

| Arm | Cost ratio (95% CI) | Cost | Wall-time ratio (95% CI) | Time |
| --- | --- | ---: | --- | ---: |
| Claude, recipe on | 1.68 (1.46–1.95) | 68% more | 1.39 (1.02–1.89) | 39% longer |
| Claude, recipe off | 1.60 (1.44–1.75) | 60% more | 1.47 (1.15–1.87) | 47% longer |
| Codex, recipe on | 0.87 (0.79–0.96) | 13% less | 2.61 (2.05–3.31) | 161% longer |
| Codex, recipe off | 0.54 (0.51–0.57) | 46% less | 1.83 (1.41–2.37) | 83% longer |

The recipe on against off, same engine:

| Engine | Cost on/off (95% CI) | Wall time on/off (95% CI) |
| --- | --- | --- |
| Claude Code | 1.05 (0.88–1.25): no measurable difference | 0.95 (0.77–1.18): no measurable difference |
| Codex | 1.61 (1.45–1.79): 61% more | 1.43 (1.23–1.66): 43% longer |

Each cell is passes/n, then median cost and median wall time.

| Task | Raw Claude Code | Claude on | Claude off | Codex on | Codex off |
| --- | --- | --- | --- | --- | --- |
| `fix-git` | 3/3 · $0.167 · 17 s | 3/3 · $0.259 · 46 s | 3/3 · $0.222 · 81 s | 3/3 · $0.086 · 81 s | 3/3 · $0.048 · 68 s |
| `fix-code-vulnerability` | 3/3 · $0.186 · 21 s | 3/3 · $0.193 · 42 s | 3/3 · $0.214 · 32 s | 3/3 · $0.158 · 103 s | 3/3 · $0.104 · 114 s |
| `headless-terminal` | 3/3 · $0.303 · 78 s | 3/3 · $0.477 · 107 s | 2/3 · $0.568 · 168 s | 3/3 · $0.097 · 194 s | 3/3 · $0.110 · 146 s |
| `build-cython-ext` | 3/3 · $0.661 · 172 s | 3/3 · $1.353 · 227 s | 3/3 · $1.595 · 218 s | 3/3 · $0.971 · 475 s | 3/3 · $0.575 · 239 s |
| `mi-seekable` (repository) | 3/3 · $0.228 · 40 s | 3/3 · $0.265 · 55 s | 3/3 · $0.222 · 64 s | 3/3 · $0.119 · 152 s | 3/3 · $0.067 · 90 s |
| `mi-one` (repository) | 3/3 · $0.213 · 37 s | 3/3 · $0.171 · 34 s | 3/3 · $0.123 · 41 s | 3/3 · $0.098 · 114 s | 3/3 · $0.066 · 85 s |
| `bottle-etag` (repository) | 3/3 · $0.357 · 57 s | 3/3 · $0.618 · 99 s | 3/3 · $0.484 · 92 s | 3/3 · $0.124 · 112 s | 3/3 · $0.140 · 92 s |

Where the money went, summed over each arm's 21 runs:

| Arm | Input tokens | Read from cache | Written to cache | Output tokens | Engine | Jev |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Raw Claude Code | 5.82 M | 93% | 7% | 87 k | $6.24 | – |
| Claude, recipe on | 1.81 M | 12% | 88% | 121 k | $10.47 | $0.048 |
| Claude, recipe off | 1.74 M | 15% | 85% | 127 k | $9.97 | $0.027 |
| Codex, recipe on | 2.51 M | 9% | – | 75 k | $5.39 | $0.060 |
| Codex, recipe off | 1.52 M | 6% | – | 47 k | $3.35 | $0.032 |

Jev was 0.45% of the Claude arm's cost and 1.1% of the Codex arm's. The router
and the start add a median of 3.2–3.6 s per run before the engine starts. The
rest of the time gap is the loop: a median of 5–6 steps, each a separate
model call, against raw Claude Code's single session (a median of 9 turns).

## What the recipe did (recipe-on arms, 42 runs)

- **Class:** Jev judged 36 of 42 tasks "hard" and 6 "change". A "hard" task
  runs Claude at medium effort instead of low, and Codex at high instead of
  medium. On these small tasks that judgment looks miscalibrated, and it
  accounts for the Codex arm's extra cost.
- **Frozen checks:** 10 of 42 runs kept checks. 7 runs ended `checks_passed`
  (3 on Claude, 4 on Codex).
- **Knowledge:** 17 of 42 runs kept at least one knowledge entry.
- **Endings:** On Claude, 14 `finished`, 3 `checks_passed`, and 4 `bad_replies`
  (the loop gave up after three unusable replies). All four `bad_replies` runs
  still passed the check. On Codex, 17 `finished` and 4 `checks_passed`.

## Task-class recalibration (#10245)

The shipped class question is now `openagents.delegate.recipe.class.v2`.
It asks about work to reach passing checks, not specialized terminology or
security implications. A substantial task is expected to need more than
20 agent steps or 10 minutes of active work; routine builds, localized fixes,
and questions do not qualify by subject alone. Escalation requires
`hard >= 0.8` (previously 0.5). The unchanged information-only judgment takes
precedence at `asks_only >= 0.6`.

Calibration uses all **105 retained runs**, grouped by their seven tasks and
five arms, with passing-run medians rather than failed-run duration. The
largest routed passing median step count is **15 steps**.
All seven tasks have the label `change`. Raw Claude's native tool
turns are retained separately because they are not loop steps. Recorded wall
time includes build waits, so it is a conservative proxy for active work, not
an independently measured active-time field.

The class-only remeasurement reused the 42 recipe-on runs' original framed
requests with Jev, plus three authored information-only and three authored
substantial-work controls. The production state builder, questions, and
classifier are shared with the evaluation runner:

| Rows | Expected | v1 hard labels | v2 result |
| --- | --- | ---: | --- |
| 42 measured requests (seven distinct tasks) | change | 36 | 42 change, 0 hard |
| 3 authored information-only controls | question | Not measured | 3 question |
| 3 authored substantial-work controls | hard | Not measured | 3 hard |

The small-task hard probabilities range from **0.10 to 0.67**, while the
substantial controls range from **0.97 to 0.98**. The 0.8 cutoff lies between
these groups. All **48/48** rows match their labels. These are calibration
results, not a held-out generalization claim: the measured corpus has only
seven distinct small tasks and no measured hard tasks. The authored hard
controls test that escalation remains possible; they do not supply measured
time-to-pass for hard work.

The 48 decision calls took **34.78 seconds** in total (median **0.69 seconds**)
and cost **$0.002084** at the recorded Jev input-token rate of $0.042 per
million tokens. No engine ran. The effort mapping now keeps these small
requests at admitted Codex medium or Claude low, while hard controls still
select Codex high or Claude medium. This does **not** remeasure end-to-end
engine cost, wall time, or pass rate; the original +61% cost and +43% time
comparison includes other recipe settings and cannot be attributed solely
to effort or claimed as savings from this change.

Retained evidence is under
[`2026-10-02-shadow-baseline/class-v2/`](2026-10-02-shadow-baseline/class-v2/):
`labels.json` binds labels to all 105 runs' outcomes, times, steps, and costs;
`rows.jsonl` contains exact framed requests and original v1 class records;
`results.jsonl` contains the remeasured answers, exact question bodies,
ATIF calls, wall times, and usage. Offline regression tests recompute labels,
verify the production question bodies, and exercise class and effort cutoffs.
To repeat the class-only measurement with the configured Jev door:

```sh
cargo run -p coder-delegate --example recipe_class_eval -- \
  docs/cost/2026-10-02-shadow-baseline/class-v2/rows.jsonl > /tmp/recipe-class-results.jsonl
```

## Method

- **Host and binaries:** coderos-4080, `~/coder-runner/openagents` and
  `microcoder` at `feb4bc8270`. Claude Code 2.1.286, signed in with the
  owner's subscription, default model `opus[1m]` (Opus 5.5, 1M context).
  Codex CLI 0.159.2, model `gpt-6.1-sol` at medium effort (the routed default).
- **Raw arm:** `claude -p PROMPT --output-format json
  --dangerously-skip-permissions`, run in a fresh copy of the task repository,
  with nothing else changed. The permission flag is the headless equivalent
  of the routed run's full access. Cost is Claude Code's own
  `total_cost_usd`.
- **Routed arms:** `openagents chat send --local --json PROMPT`, run in a fresh
  copy of the repository. Each arm had its own task store, chat home, and
  settings file. The settings enabled one agent (`coder.providers` set to
  `claude` or `codex`, every other agent disabled). The router sent every
  prompt to Coder. Cost and wall time come from the route record
  (`routes/<thread>.jsonl`) and the task's ATIF trajectory.
- **Recipe off:** the shipped launchers clear the environment, so
  `OPENAGENTS_DELEGATE_RECIPE=off` never reached the engine (fixed in this
  change). The off arms used a controller shim
  ([`microcoder-recipe-off.sh`](2026-10-02-shadow-baseline/microcoder-recipe-off.sh)),
  named by `OPENAGENTS_CODER_CONTROLLER`. It starts the same owner process the
  `--detach` launch starts, with the variable set. No off-arm trajectory has a
  `delegate_recipe` record. With the recipe off, the Microcoder loop keeps its
  own lean settings (no tools, one turn per step, five-minute cache). Only the
  briefing, knowledge, class effort, and frozen checks are removed.
- **Codex price:** `gpt-6.1-sol` had no list price in
  `codex_transport::price`, so every routed Codex run's cost was recorded as
  unknown. The study priced Codex's reported tokens at OpenAI's $2 input and
  $10 output per million (`docs/research/ppq.md`) and $0.20 per million cached
  input, the same 10% ratio as GPT-6 Sol. This change adds that price.
- **Not counted:** the hosted chat router's own model call (Gemini 2.5 Flash
  Lite on the chat worker) is not metered on this computer, so it is not in
  the routed cost. All costs are list price, not bills.

### Tasks and checks

The four development-panel tasks from Terminal-Bench 2.1 were moved from their
containers to scratch Git repositories on the host. Each was adapted only as
far as the host required:

- `fix-git`: the prepared `personal-site` checkout, copied out of the task's
  prebuilt image (`alexgshaw/fix-git:20260403`; the upstream repository no
  longer exists). The instruction is unchanged. The check is the task's MD5 of
  both files, in the working tree or on `master`. A routed run works in a
  detached worktree, and `master` lives in the checkout.
- `fix-code-vulnerability`: bottle at the task's commit with the task's two
  deletions. `/app` paths are now relative. The check is the repository's own
  `pytest` plus the task's tests, with the run's `bottle.py` imported and the
  report's path ending in `bottle.py`.
- `headless-terminal`: `base_terminal.py`, with "the system Python" pointing
  at a per-run virtualenv. The check is the task's tests with a fresh `HOME`
  (holding the `.profile` that sources `.bashrc`, as the image's does), a fresh
  tmux server, a free port, and a temporary folder for `/server`.
- `build-cython-ext`: an empty repository. The system Python is a per-run
  virtualenv with NumPy 2.3.0, plus a `.pth` file that preloads `libstdc++`,
  `libgcc_s`, and `zlib` so manylinux wheels load on NixOS. The check is the
  task's tests, pointed at `./pyknotid` and that environment.

Three ordinary repository tasks came from real fixes merged after the
models' training cutoff. The repository held only the fix's parent commit and
its history. The request was written as an issue, and the check is the fix
commit's own test file:

- `mi-seekable`: more-itertools `6b1907d` (`seekable(maxlen=0)` dropped peeked items).
- `mi-one`: more-itertools `def2dab` (`one()`/`only()` ignored a falsy custom exception).
- `bottle-etag`: bottle `457a8fa` (unquoted ETag; a multi-valued If-None-Match).
  `test_sendfile` leaks state between tests at the fix commit itself, so each
  test runs on its own. The hidden test file's mtime is set an hour back,
  because `test_ims` compares a whole-second If-Modified-Since against the
  file's mtime.

Every check was validated before the study: each fails on the untouched task
and passes on the reference fix (the oracle). The cython oracle was not run.

### Variance, failures, and corrections

- **Reruns.** A disk-full moment on the host (other work on the box filled the
  last space) killed 4 routed task owners mid-run and failed 2 setups. The
  harness then waited on those dead runs. All 6 were deleted and rerun; the
  table has no lost runs. Separately, `openagents chat send` kept following a
  task whose owner had died, with no error, until it was stopped.
- **Two check corrections, applied to every arm alike:**
  - The first `fix-git` check read only the run's own worktree. Every routed
    `fix-git` run, 12 of 12, ran commands in the person's checkout (`cd` into
    it) to merge into `master`. Coder's worktree is detached, and `master` is
    checked out in the checkout. That changed the 3 results recorded before the correction from fail to pass; later runs were checked the corrected way from the start.
  - `bottle-etag`'s `test_ims` failed 4 runs on timing alone (above). The
    agents' changes were correct, and those 4 results changed from fail to
    pass.

  The recheck reruns the check on the kept work and never reruns an agent.
- **Real failures:** one, `headless-terminal` on Claude with the recipe off
  (trial 2): the interactive vim test failed.
- **Intervals:** with 21 runs per arm, every pass-rate interval overlaps.
  Pass rates are indistinguishable. The cost and time ratios have bootstrap
  intervals that exclude 1 for every comparison except the recipe's effect on
  Claude.

## What it cost

The 105 runs cost **$35.60** at list price, $0.17 of it Jev, with 3.4 hours of
agent wall time. About a dozen probe runs, used to build the harness, added
under a dollar. Claude usage came from the owner's subscription and Codex
usage from the Codex login, so list price is not what was billed.

## What it means for the umbrella (#10204)

1. **The claim isn't shown yet on the shipped path.** The measured win so far
   is the Codex engine's lower price (46% cheaper than raw Claude Code with the
   recipe off), which costs about twice the wall time. That is a model choice,
   not routing.
2. **Fix the cache before anything else.** Keep the Microcoder step's prompt
   stable at the front (system text, task, briefing) and append each step's
   new state at the end, or keep one Claude session across steps instead of a
   fresh `claude -p` per step. This should turn most of the 85–88% of input
   now written to cache into reads, at a twelfth of the write price.
3. **Recalibrate the recipe's class.** 36 of 42 small tasks judged "hard"
   means "hard" doesn't separate tasks. Until it does, high effort on Codex
   is pure cost.
4. **Measure the audit's winning path too.** The Coder One CLI delegate
   (`jevprobe2-opus-lean-low-5m`) is the configuration behind the 61% and 63%
   savings. Running it as a sixth arm on this harness would show whether
   routing to it, rather than to the Microcoder loop, delivers the claim.
5. **Shadow real use from now on.** The shadow baseline below turns everyday
   runs into the same comparison.

## The shadow baseline (shipped with this measurement)

The shadow baseline is opt-in and off by default: `openagents settings set
coder.shadow 10` samples 10% of this computer's finished Coder runs. Sampling
is by task ID, so it is reproducible. Eligible runs are a first turn on Codex
or Claude Code that is not an issue flow. Each sampled run also runs once
through the raw engine on its own defaults (`claude -p`, or `codex exec` when
Codex ran the task) with the person's request as they wrote it. The baseline
runs in a scratch clone of the same commit with no remote, so it can't push,
and its changes are never applied. One baseline runs at a time.
`coder.shadow_budget_usd` is the person's own cap and is unset by default (no
cap). `openagents shadow report` finishes the baselines that have ended: it
runs the recipe's kept checks on both sides, writes
`~/.openagents/shadow/records.jsonl`, deletes the clone, and prints cost and
wall time (totals and medians) and checks passed, routed against raw. The code
is [`coder::task::shadow`](../../crates/coder/src/task/shadow.rs) and the
settings are documented in [docs/cli/settings.md](../cli/settings.md).

## Files

- [`2026-10-02-shadow-baseline/study.py`](2026-10-02-shadow-baseline/study.py):
  the harness (prepare, run, check, recheck, collect).
- [`2026-10-02-shadow-baseline/analyze.py`](2026-10-02-shadow-baseline/analyze.py)
  and [`extra.py`](2026-10-02-shadow-baseline/extra.py): the tables above.
- [`2026-10-02-shadow-baseline/collected.jsonl`](2026-10-02-shadow-baseline/collected.jsonl):
  one line per run. Each run's directory on coderos-4080
  (`~/shadow-10209/runs/<task>/<arm>/<trial>/`) holds `result.json` with the
  check's output, the CLI's event stream, the route record, and the ATIF
  trajectory.
