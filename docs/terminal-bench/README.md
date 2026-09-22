# Terminal-Bench results

This page is the one place for OpenAgents' Terminal-Bench results: every
retained trial of every arm, with reward, cost, Jev cost, duration, and call
counts, plus an analysis of each Coder One run. Update it after every run.

- Harness: `bench/terminal-bench/` (Harbor 0.22.0). How to run it:
  [the runbook](../coder/terminal-bench.md). The artifact contract:
  [`openagents.coder.episode.v1`](../coder/terminal-bench-contract.md).
- Tasks: the upstream Terminal-Bench repository at `3b5caaa4863d`.
- Evidence: each row links its retained trajectory under
  [`bench/terminal-bench/traces/`](../../bench/terminal-bench/traces/).
- Tracking issues: [#9530](https://github.com/OpenAgentsInc/openagents/issues/9530)
  (harness) and [#9531](https://github.com/OpenAgentsInc/openagents/issues/9531)
  (Coder One).

Every row is a single trial. Treat the tables as behavioral evidence for
development, not a pass-rate estimate or a significance claim.

## How to read the columns

- **Reward** is the task's own verifier result. *No result* means the
  verifier produced no reward, so the attempt is unverifiable, not a zero.
- **Cost** is in US dollars. The sources differ by arm, so the **Cost
  source** column names each one:
  - *CLI list price*: Claude Code's own `total_cost_usd`. These runs
    authenticated with a subscription, so it is a list-price figure, not a
    bill.
  - *Harbor estimate*: Harbor's price estimate from Codex's token counts.
    Codex ran on a ChatGPT subscription, so this is not a bill either.
  - *Door-reported + Jev list price*: Coder One's generation cost as
    openagents.com reports it per call (`cost_microusd`), plus the Jev cost.
  - Devin shows `—`. Devin does not report what the run would cost to buy,
    so no number is shown.
- **Jev cost** is exact: the Jev input tokens TypeSafe reported, times the
  published `jev-1.13.0` rate of $0.042 per million input tokens. Output
  tokens are free. It is `—` for arms that make no Jev calls.
- **Agent time** is how long the agent ran, in seconds. A plain number is
  Harbor's `agent_execution` phase. A number marked † is the span from the
  first to the last step of the retained trajectory, which is a lower bound:
  those trials' Harbor phase timings stayed in the job directories of the
  machine that ran them. `—` means the trajectory carries no usable
  timestamps (Devin writes one timestamp for every step).
- **Steps** counts agent steps in the trajectory. **Tool calls** counts the
  tool calls the agent made. For Coder One, generations, Jev requests, and
  shell commands are counted separately.
- **Tokens** are model input / cached input / output. For Coder One they
  are generation tokens only; Jev tokens appear in the Jev cost.

## Results

### `fix-git`

| Arm | Model | Reward | Cost | Cost source | Jev cost | Agent time | Steps | Tool calls | Tokens in / cached / out | Trace |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Coder One** | Gemini 3.8 Flash (`free` lane) + `jev-1.13.0` | **1.0** | **$0.0604** | Door-reported + Jev list price | $0.0022524 | 73.8 | 17 generations, 17 Jev | 16 shell | 50,613 / 0 / 5,369 | [trace](../../bench/terminal-bench/traces/smoke--coder-one--fix-git/) |
| Claude Code 2.1.278 | Fable 5.1 | 1.0 | $0.3630 | CLI list price | — | 30.5† | 6 | 5 | 118,946 / 106,631 / 1,833 | [trace](../../bench/terminal-bench/traces/smoke--claude-code--fix-git-fable/) |
| Claude Code 2.1.278 | Sonnet 4.5 | 1.0 | $0.1463 | CLI list price | — | 42.6† | 8 | 10 | 179,343 / 168,918 / 2,216 | [trace](../../bench/terminal-bench/traces/smoke--claude-code--fix-git-2/) |
| Codex 0.153.3 | gpt-6-astra | 1.0 | $0.2607 | Harbor estimate | — | 53.3 | 6 | 5 | 103,959 / 91,776 / 941 | [trace](../../bench/terminal-bench/traces/smoke--codex--fix-git-4/) |
| Devin 3000.11.1 | swe-2-high | 1.0 | — | — | — | — | 11 | 12 | 171,977 / 163,652 / 3,843 | [trace](../../bench/terminal-bench/traces/smoke--devin--fix-git-swe2/) |
| Devin 3000.11.1 | claude-sonnet-5-high | 0.0 (provider refusal: weekly quota exhausted) | — | — | — | — | 0 | 0 | — | [trace](../../bench/terminal-bench/traces/smoke--devin--fix-git/) |
| Oracle / nop controls | — | 1.0 / 0.0 | — | — | — | — | — | — | — | — |

Codex's 53.3-second agent time is Harbor's phase timing from the #9530 run
log; its trajectory spans 46.1 seconds.

### `build-cython-ext`

| Arm | Model | Reward | Cost | Cost source | Jev cost | Agent time | Steps | Tool calls | Tokens in / cached / out | Trace |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Coder One** | Gemini 3.8 Flash (`free` lane) + `jev-1.13.0` | Running | | | | | | | | |
| Claude Code 2.1.278 | Fable 5.1 | 1.0 | $1.4420 | CLI list price | — | 201.8† | 20 | 19 | 702,690 / 666,599 / 11,193 | [trace](../../bench/terminal-bench/traces/smoke--claude-code--build-cython-ext-fable/) |
| Claude Code 2.1.278 | Sonnet 4.5 | 0.0 | $0.9172 | CLI list price | — | 263.7† | 46 | 55 | 1,800,515 / 1,760,664 / 10,070 | [trace](../../bench/terminal-bench/traces/smoke--claude-code--build-cython-ext/) |
| Codex 0.153.3 | gpt-6-astra | 1.0 | $1.4277 | Harbor estimate | — | 222.2† | 21 | 20 | 801,816 / 758,528 / 4,726 | [trace](../../bench/terminal-bench/traces/smoke--codex--build-cython-ext/) |
| Devin 3000.11.1 | swe-2-high | 1.0 | — | — | — | — | 42 | 95 | 2,361,410 / 2,287,028 / 25,586 | [trace](../../bench/terminal-bench/traces/smoke--devin--build-cython-ext/) |
| Oracle / nop controls | — | 1.0 / 0.0 | — | — | — | — | — | — | — | — |

### `fix-code-vulnerability`

| Arm | Model | Reward | Cost | Cost source | Jev cost | Agent time | Steps | Tool calls | Tokens in / cached / out | Trace |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Claude Code 2.1.278 | Fable 5.1 | 1.0 | $0.2333 | CLI list price | — | 24.7† | 4 | 3 | 77,986 / 70,662 / 1,403 | [trace](../../bench/terminal-bench/traces/panel--claude-code--fix-code-vulnerability/) |
| Codex 0.153.3 | gpt-6-astra | 1.0 | $0.5094 | Harbor estimate | — | 44.2† | 6 | 5 | 167,355 / 134,528 / 933 | [trace](../../bench/terminal-bench/traces/panel--codex--fix-code-vulnerability/) |
| Devin 3000.11.1 | swe-2-high | 1.0 | — | — | — | — | 15 | 17 | 305,662 / 282,281 / 4,659 | [trace](../../bench/terminal-bench/traces/panel--devin--fix-code-vulnerability/) |

### `cancel-async-tasks`

| Arm | Model | Reward | Cost | Cost source | Jev cost | Agent time | Steps | Tool calls | Tokens in / cached / out | Trace |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Claude Code 2.1.278 | Fable 5.1 | 1.0 | $0.5988 | CLI list price | — | 92.8† | 8 | 7 | 172,919 / 162,262 / 6,947 | [trace](../../bench/terminal-bench/traces/panel--claude-code--cancel-async-tasks/) |
| Codex 0.153.3 | gpt-6-astra | 1.0 | $0.1781 | Harbor estimate | — | 54.2† | 4 | 3 | 59,581 / 55,168 / 1,577 | [trace](../../bench/terminal-bench/traces/panel--codex--cancel-async-tasks-2/) |
| Devin 3000.11.1 | swe-2-high | No result | — | — | — | — | 20 | 21 | 523,855 / 492,318 / 24,152 | [trace](../../bench/terminal-bench/traces/panel--devin--cancel-async-tasks/) |

### `headless-terminal`

| Arm | Model | Reward | Cost | Cost source | Jev cost | Agent time | Steps | Tool calls | Tokens in / cached / out | Trace |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Claude Code 2.1.278 | Fable 5.1 | 1.0 | $1.2035 | CLI list price | — | 166.4† | 8 | 7 | 211,630 / 187,603 / 13,567 | [trace](../../bench/terminal-bench/traces/panel--claude-code--headless-terminal/) |
| Codex 0.153.3 | gpt-6-astra | 0.0 | $0.4597 | Harbor estimate | — | 172.7† | 7 | 6 | 112,356 / 100,608 / 4,833 | [trace](../../bench/terminal-bench/traces/panel--codex--headless-terminal/) |
| Devin 3000.11.1 | swe-2-high | No result | — | — | — | — | — | — | — | No trajectory retained |

### Not yet run

`vllm-deepseek-streaming` and `batched-eval-parity` are in the CPU panel and
have no retained trials. `math-eval-grader` needs an H100 and stays excluded
locally.

## Coder One run analyses

Coder One (`crates/coder-one`) is a minimal loop: each step sends one Jev
request whose typed answers become hints in the prompt, generates one tool
call through openagents.com, and runs it. Every step rebuilds the prompt
from explicit state, so no model context carries over between steps.

### `fix-git`, 2026-09-22

Artifact `coder-one 0.1.0 (cb6968b9faa6)`, sha256 `dbfe9879…9577c`,
`free` lane, 50-step limit, 300-second command deadline. Reward 1.0: the
verifier's `test_about_file` and `test_layout_file` both passed. Harbor
phases: environment 10.9 s, agent setup 1.1 s, agent 73.8 s, verifier
8.9 s, 105.7 s in all.

| | Coder One | Claude Code / Fable 5.1 | Codex / gpt-6-astra |
| --- | --- | --- | --- |
| Reward | 1.0 | 1.0 | 1.0 |
| Total cost | $0.0604 | $0.3630 | $0.2607 |
| Generation calls | 17 | 6 | 6 |
| Jev calls | 17 | — | — |
| Shell or tool calls | 16 | 5 | 5 |
| Generation input tokens | 50,613 | 118,946 | 103,959 |
| Of which cached | 0 | 106,631 | 91,776 |
| Generation output tokens | 5,369 | 1,833 | 941 |
| Mean input tokens per generation | 2,977 (769 to 5,249) | 19,824 | 17,327 |
| Generation cost | $0.0581 | $0.3630 | $0.2607 |
| Jev input tokens | 53,628 | — | — |
| Jev cost | $0.0022524 | — | — |
| Cost per 1,000 tokens moved (in + out) | $0.0011 | $0.0030 | $0.0025 |
| Agent time | 73.8 s | 30.5 s† | 53.3 s |
| Time in generation | 69.0 s (93%) | | |
| Time in Jev | 3.7 s over 17 calls (5%) | | |
| Time in shell commands | 0.1 s | | |

**Why it was cheap:**

- **A small, cheap model.** Gemini 3.8 Flash costs less per token than
  Fable 5.1 or gpt-6-astra. Across all tokens moved, Coder One paid about a
  third of Claude Code's rate and under half of Codex's.
- **Small prompts.** Each prompt is rebuilt from the issue, a compact
  history, and Jev's hints. Recent command outputs are capped at 6,000
  characters and older ones at 300. The mean prompt was 2,977 tokens,
  against about 18,000 to 20,000 for the external agents, which re-send
  their whole conversation each turn.
- **Not fewer calls.** Coder One made nearly three times as many generation
  calls (17 against 6) and three times as many shell calls (16 against 5).
  Many were repeated `git status` and `git show` checks. It was cheap in
  spite of its call count, not because of it.
- **Jev added 3.7% to the bill.** Jev read more input tokens than
  generation did (53,628 against 50,613), because each request carries the
  candidate files and command history. At $0.042 per million it cost
  $0.0022524.
- **No prompt caching.** The external agents' input was about 90% cached.
  Coder One's prompts change every step, so nothing was cached. Caching
  would matter more for a larger model.

**Why it was slower:** 17 sequential generations averaged about 4 seconds
each, and they took 93% of the agent time. Jev took 5%. Fewer, better
steps would help more than faster Jev calls.

**What to try next:** fewer redundant inspection steps (the model re-ran
`git status` five times), and a stronger lane to see whether it finishes
in fewer steps at a comparable cost.

**Cost caveat:** openagents.com describes the `free` lane as "metered at
zero" but still reports a `cost_microusd` for each call. The table uses the
reported figure until we confirm whether it is a bill or a market-rate
estimate.

## After every Coder One run

1. Check the result: `uv run tbench inspect <job>`.
2. Copy the evidence into
   `bench/terminal-bench/traces/<job>/`: Harbor's `agent/trajectory.json`
   as `<trial>.json`, plus `manifest.json`, `evaluation/usage.json`, and a
   trimmed `result.json` (phase timings, reward, agent usage) under
   `<trial>.episode/`. Check that no credential appears in any of them.
3. Add a row to the task's table. Take generation cost from
   `usage.json`, and compute Jev cost as Jev input tokens × $0.042 ÷
   1,000,000. Take agent time from Harbor's `agent_execution` phase.
4. Add an analysis under **Coder One run analyses**: reward, total and
   per-component cost, calls by kind, tokens per call, where the time went,
   and what explains the difference from the other arms on the same task.
5. Commit and push.

## Data problems

- Three Codex `fix-git` trajectories (`smoke--codex--fix-git`, `-2`, and
  `-3`) are not valid JSON: Harbor's credential scrubber rewrote literal
  values in them. The first two were also model-rejection failures. Only
  `smoke--codex--fix-git-4` is usable.
- Devin trajectories carry one timestamp for every step, so no duration can
  be read from them.
- The external arms' trials ran on an arm64 Mac, and Coder One's on an
  x86_64 Linux host. Agent times are not strictly comparable across
  machines.
