# Terminal-Bench results

This page is the one place for OpenAgents' Terminal-Bench results: every
retained trial of every arm, with reward, cost, Jev cost, duration, and call
counts, plus an analysis of each Coder One run. Update it after every run.

- Harness: `bench/terminal-bench/` (Harbor 0.22.0). How to run it:
  [the runbook](../coder/terminal-bench.md). The artifact contract:
  [`openagents.coder.episode.v1`](../coder/terminal-bench-contract.md).
- Coder One's delegate arms: [the delegate runbook](coder-one-delegate-runbook.md).
- Tasks: the upstream Terminal-Bench repository at `3b5caaa4863d`.
- Evidence: each row links its retained trajectory under
  [`bench/terminal-bench/traces/`](../../bench/terminal-bench/traces/).
- Tracking issues: [#9530](https://github.com/OpenAgentsInc/openagents/issues/9530)
  (harness) and [#9531](https://github.com/OpenAgentsInc/openagents/issues/9531)
  (Coder One).

Every row is a single trial. Treat the tables as behavioral evidence for
development, not a pass-rate estimate or a significance claim.

## Headline: Coder One against Opus 5.5 and GPT-6

The comparison we care about most: Coder One, alone and delegating to Opus
5.5, against Claude Code on Opus 5.5 and Codex on the three GPT-6 models.
Every row ran on the same task pins, on the same x86_64 Linux host, with
Harbor's phase timings. GPT-6 costs use the operator's list prices (‡, see
[GPT-6 pricing](#gpt-6-pricing)).

| Task | Arm | Reward | Total cost | Agent time | Model calls |
| --- | --- | --- | --- | --- | --- |
| `fix-git` | Claude Code 2.1.280 / Opus 5.5 | 1.0 | $0.1420 | 23.8 s | 7 steps |
| `fix-git` | Coder One / Gemini 3.8 Flash + Jev | 1.0 | $0.0604 | 73.8 s | 17 generations, 17 Jev |
| `fix-git` | Coder One delegating to Opus 5.5 (`always`) | 1.0 | $0.1055 | 47.4 s | 1 generation, 3 Jev, 4 Opus turns |
| `fix-git` | Coder One delegating to Opus 5.5 (`auto`) | 1.0 | $0.1408 | 70.3 s | 5 generations, 7 Jev, 4 Opus turns |
| `fix-git` | Codex 0.155.1 / GPT-6 Astra | 1.0 | $1.1275‡ | 47.8 s | 6 steps |
| `fix-git` | Codex 0.155.1 / GPT-6 Sol | 1.0 | $0.4464‡ | 64.8 s | 11 steps |
| `fix-git` | Codex 0.155.1 / GPT-6 Luna | 1.0 | $0.0180‡ | 65.3 s | 9 steps |
| `build-cython-ext` | Claude Code 2.1.280 / Opus 5.5 | 1.0 | $0.4173 | 114.9 s | 17 steps |
| `build-cython-ext` | Coder One / Gemini 3.8 Flash + Jev | 1.0 | $0.3748 | 338.4 s | 49 generations, 48 Jev |
| `build-cython-ext` | Coder One delegating to Opus 5.5 (`always`) | 1.0 | $0.3905 | 128.9 s | 8 generations, 8 Jev, 16 Opus turns |
| `build-cython-ext` | Coder One delegating to Opus 5.5 (`auto`) | 1.0 | $0.4145 | 153.5 s | 8 generations, 8 Jev, 13 Opus turns |
| `build-cython-ext` | Codex 0.155.1 / GPT-6 Astra | 1.0 | $6.7650‡ | 223.6 s | 18 steps |
| `build-cython-ext` | Codex 0.155.1 / GPT-6 Sol | 1.0 | $1.9323‡ | 357.7 s | 28 steps |
| `build-cython-ext` | Codex 0.155.1 / GPT-6 Luna | 1.0 | $0.0988‡ | 204.7 s | 31 steps |

Every arm solved both tasks. What separates them is cost and time:

- **Opus 5.5 is the fastest** on both tasks, and about 94% of its input
  was cached.
- **Coder One alone is cheap but slow.** It cost 43% of Opus 5.5 on
  `fix-git` and 90% on `build-cython-ext`, and took about three times as
  long. Its prompts are small and uncached, and it takes many more steps.
- **Delegating to Opus 5.5 cut Opus's turns but not its cost much.** The
  `always` arm finished `fix-git` in 4 Opus turns against 7, for 26% less,
  and `build-cython-ext` in 16 turns against 17, for 6% less. Each Opus
  call still carries Claude Code's own prompt of about 16,000 to 20,000
  tokens, which a briefing cannot shrink. Delegation was slower than Opus
  alone: the explore phase added 30 to 40 seconds, and on `fix-git` the
  free lane's rate limit cost another 30 seconds (see
  [Data problems](#data-problems)).
- **GPT-6 Luna is the cheapest arm by far** at the stated rates, and it
  solved both tasks. GPT-6 Astra is the most expensive: its upper-bound
  cost on `build-cython-ext` is $6.77.

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
  - *Manual list price* ‡: GPT-6 costs computed from list prices the
    operator supplied on 2026-09-22, not from Harbor, which has no price
    for these models under Codex 0.155.1. See [GPT-6 pricing](#gpt-6-pricing).
  - *Door + Jev + CLI list price*: a Coder One delegate arm's generation,
    Jev, and Opus 5.5 costs added together.
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
| **Coder One → Opus 5.5** (delegate, `always`) | Gemini 3.8 Flash + `jev-1.13.0`, then Opus 5.5 | **1.0** | **$0.1055** | Door + Jev + CLI list price | $0.0002955 | 47.4 | 1 generation, 3 Jev, 4 Opus turns | 1 shell | Gemini 900 / — / 240; Opus 71,496 / 62,267 / 870 | [trace](../../bench/terminal-bench/traces/smoke--coder-one-delegate-opus--fix-git/) |
| **Coder One → Opus 5.5** (delegate, `auto`) | Gemini 3.8 Flash + `jev-1.13.0`, then Opus 5.5 | **1.0** | **$0.1408** | Door + Jev + CLI list price | $0.0008844 | 70.3 | 5 generations, 7 Jev, 4 Opus turns | 5 shell | Gemini 12,660 / — / 1,307; Opus 83,036 / 72,060 / 1,166 | [trace](../../bench/terminal-bench/traces/smoke--coder-one-delegate-auto--fix-git/) |
| Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.1420 | CLI list price | — | 23.8 | 7 | 6 | 126,361 / 116,005 / 1,799 | [trace](../../bench/terminal-bench/traces/smoke--claude-code-opus--fix-git/) |
| Codex 0.155.1 | GPT-6 Astra | 1.0 | $1.1275‡ | Manual list price | — | 47.8 | 6 | 5 | 107,937 / 98,944 / 963 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-astra--fix-git/) |
| Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.4464‡ | Manual list price | — | 64.8 | 11 | 10 | 209,964 / 189,312 / 2,646 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-sol--fix-git/) |
| Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0180‡ | Manual list price | — | 65.3 | 9 | 8 | 166,488 / 142,080 / 2,724 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-luna--fix-git/) |
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
| **Coder One** | Gemini 3.8 Flash (`free` lane) + `jev-1.13.0` | **1.0** | **$0.3748** | Door-reported + Jev list price | $0.0100300 | 338.4 | 49 generations, 48 Jev | 48 shell | 403,209 / 0 / 16,631 | [trace](../../bench/terminal-bench/traces/smoke--coder-one--build-cython-ext/) |
| **Coder One → Opus 5.5** (delegate, `always`) | Gemini 3.8 Flash + `jev-1.13.0`, then Opus 5.5 | **1.0** | **$0.3905** | Door + Jev + CLI list price | $0.0010810 | 128.9 | 8 generations, 8 Jev, 16 Opus turns | 7 shell | Gemini 17,873 / — / 2,660; Opus 404,204 / 380,950 / 5,199 | [trace](../../bench/terminal-bench/traces/smoke--coder-one-delegate-opus--build-cython-ext/) |
| **Coder One → Opus 5.5** (delegate, `auto`) | Gemini 3.8 Flash + `jev-1.13.0`, then Opus 5.5 | **1.0** | **$0.4145** | Door + Jev + CLI list price | $0.0013304 | 153.5 | 8 generations, 8 Jev, 13 Opus turns | 8 shell | Gemini 33,773 / — / 2,500; Opus 396,041 / 370,767 / 5,112 | [trace](../../bench/terminal-bench/traces/smoke--coder-one-delegate-auto--build-cython-ext/) |
| Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.4173 | CLI list price | — | 114.9 | 17 | 16 | 456,887 / 431,183 / 6,277 | [trace](../../bench/terminal-bench/traces/smoke--claude-code-opus--build-cython-ext/) |
| Codex 0.155.1 | GPT-6 Astra | 1.0 | $6.7650‡ | Manual list price | — | 223.6 | 18 | 17 | 652,905 / 613,376 / 4,720 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-astra--build-cython-ext/) |
| Codex 0.155.1 | GPT-6 Sol | 1.0 | $1.9323‡ | Manual list price | — | 357.7 | 28 | 27 | 936,445 / 896,000 / 5,943 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-sol--build-cython-ext/) |
| Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0988‡ | Manual list price | — | 204.7 | 31 | 30 | 952,216 / 904,448 / 7,151 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-luna--build-cython-ext/) |
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

### GPT-6 pricing

The GPT-6 costs are **manual**: they come from list prices the operator
supplied on 2026-09-22, not from Harbor. Harbor records no cost for these
models under Codex 0.155.1.

| Model | Input, per million tokens | Output, per million tokens |
| --- | --- | --- |
| GPT-6 Astra | $10.00 | $50.00 |
| GPT-6 Sol | $2.00 | $10.00 |
| GPT-6 Luna | $0.10 | $0.50 |

No cached-input rate was supplied, so the tables charge cached input at the
full input rate. That is an upper bound: about 90% of each run's input was
cached. The lower bound charges only uncached input:

| Trial | Upper bound (cached at the input rate) | Lower bound (cached input free) |
| --- | --- | --- |
| GPT-6 Astra, `fix-git` | $1.1275 | $0.1381 |
| GPT-6 Astra, `build-cython-ext` | $6.7650 | $0.6313 |
| GPT-6 Sol, `fix-git` | $0.4464 | $0.0678 |
| GPT-6 Sol, `build-cython-ext` | $1.9323 | $0.1403 |
| GPT-6 Luna, `fix-git` | $0.0180 | $0.0038 |
| GPT-6 Luna, `build-cython-ext` | $0.0988 | $0.0084 |

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

### `build-cython-ext`, 2026-09-22

Same artifact and bounds. Reward 1.0: all 11 verifier tests passed,
including `test_ccomplexity`, the test Claude Code on Sonnet 4.5 failed
because it never searched the `.pyx` sources. Harbor phases: environment
53.9 s, agent setup 1.2 s, agent 338.4 s, verifier 4.2 s, 408.7 s in all.

| | Coder One | Claude Code / Fable 5.1 | Codex / gpt-6-astra | Claude Code / Sonnet 4.5 |
| --- | --- | --- | --- | --- |
| Reward | 1.0 | 1.0 | 1.0 | 0.0 |
| Total cost | $0.3748 | $1.4420 | $1.4277 | $0.9172 |
| Generation calls | 49 | 20 | 21 | 46 |
| Jev calls | 48 | — | — | — |
| Shell or tool calls | 48 | 19 | 20 | 55 |
| Generation input tokens | 403,209 | 702,690 | 801,816 | 1,800,515 |
| Of which cached | 0 | 666,599 | 758,528 | 1,760,664 |
| Generation output tokens | 16,631 | 11,193 | 4,726 | 10,070 |
| Mean input tokens per generation | 8,229 (980 to 14,065) | 35,135 | 38,182 | 39,142 |
| Generation cost | $0.3648 | $1.4420 | $1.4277 | $0.9172 |
| Jev input tokens | 238,809 | — | — | — |
| Jev cost | $0.0100300 | — | — | — |
| Cost per 1,000 tokens moved (in + out) | $0.0009 | $0.0020 | $0.0018 | $0.0005 |
| Agent time | 338.4 s | 201.8 s† | 222.2 s† | 263.7 s† |
| Time in generation | 219.1 s (65%) | | | |
| Time in shell commands | 103.9 s (31%) | | | |
| Time in Jev | 13.9 s over 48 calls (4%) | | | |

**Why it was cheap:** the same reasons as `fix-git`. The model is cheaper
per token, and each prompt is rebuilt small (8,229 tokens on average,
against 35,000 to 39,000 for the external agents). Coder One made more
than twice as many generation calls as Fable 5.1 or Codex (49 against 20
and 21) and still cost about a quarter as much. Jev was 2.7% of the bill.

**Why it succeeded where Sonnet 4.5 failed:** after the first round of
fixes, Coder One listed every `.c`, `.pyx`, and `.so` file (step 21) and
searched the `.pyx` sources for NumPy aliases (step 22). Sonnet 4.5
limited every search to `*.py` and never reached `ccomplexity.pyx`.

**Why it was slower:** 49 sequential generations took 219 seconds, and
the builds, installs, and test runs the task needs took 104 seconds. Jev
took 14 seconds. Coder One also repeated some work: it read `setup.py`
five times in slices and re-ran the same alias search three times.

### Delegate mode, 2026-09-22

Artifact `coder-one 0.1.0 (9d7f081dc051)`, sha256 `b1107be4…15c85`, from
[#9532](https://github.com/OpenAgentsInc/openagents/issues/9532). The
explorer runs on the `free` lane for up to 8 steps. Code builds a briefing
of up to 12,000 characters from Jev's answers, and Claude Code 2.1.280 on
Opus 5.5 finishes the task. `always` delegates after exploring; `auto`
delegates when the explorer stalls. All four trials scored 1.0.

| | Opus 5.5 alone | `always`, `fix-git` | `auto`, `fix-git` | Opus 5.5 alone | `always`, `build-cython-ext` | `auto`, `build-cython-ext` |
| --- | --- | --- | --- | --- | --- | --- |
| Task | `fix-git` | | | `build-cython-ext` | | |
| Total cost | $0.1420 | $0.1055 | $0.1408 | $0.4173 | $0.3905 | $0.4145 |
| Gemini cost | — | $0.0016 | $0.0144 | — | $0.0234 | $0.0347 |
| Jev cost | — | $0.0002955 | $0.0008844 | — | $0.0010810 | $0.0013304 |
| Opus cost | $0.1420 | $0.1037 | $0.1255 | $0.4173 | $0.3661 | $0.3785 |
| Opus turns | 7 steps | 4 | 4 | 17 steps | 16 | 13 |
| Opus input tokens | 126,361 | 71,496 | 83,036 | 456,887 | 404,204 | 396,041 |
| Explore steps | — | 1 | 5 | — | 7 | 8 |
| Why it delegated | — | always | the explorer's generation failed (rate limit) | — | always | explore bound reached |
| Briefing | — | 3,180 chars | 4,708 chars | — | — | 11,934 chars |
| Agent time | 23.8 s | 47.4 s | 70.3 s | 114.9 s | 128.9 s | 153.5 s |
| Time exploring (generation + shell + Jev) | — | 33.4 s, 30.4 s of it a rate-limited wait | 51.8 s, 30.5 s of it a rate-limited wait | — | 32.8 s | 72.4 s |
| Time in Opus | 23.8 s | 12.3 s | 17.9 s | 114.9 s | 95.3 s | 80.6 s |

**What the briefing bought:** Opus reached the answer in fewer turns and
less time once it started. On `fix-git` it needed 4 turns and 12 to 18
seconds, against 7 steps and 24 seconds from scratch. On
`build-cython-ext` it spent 80 to 95 seconds, against 115.

**Why the total barely moved:** every Opus call carries Claude Code's own
system prompt and tool definitions, 16,000 to 20,000 tokens at the first
call, and the conversation grows from there. The briefing removes
exploration turns, not that fixed cost. On `build-cython-ext` the saved
turns were worth 6% in the `always` arm and 0.7% in the `auto` arm.

**Why it was slower:** the explore phase runs before Opus starts, and on
this host the free lane allows 20 generations a minute per account. Both
`fix-git` runs lost about 30 seconds to that limit, and the `auto` run
delegated because of it rather than because the task stalled. Without the
waits, the `always` arm would have finished `fix-git` in about 17 seconds.

**What to try next:** a shorter explore phase (2 to 3 steps) that exists
only to build the briefing; a lean executor configuration that drops Claude
Code's unused tools from the prompt; and reruns without the rate limit.

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

- Four Coder One trials on 2026-09-22 are **invalid**, not losses: `coder-one-v2`
  (prompt reordering) and `coder-one-deep` (reordering plus a Jev survey),
  each on both tasks, artifact `coder-one 0.1.0 (99e647a7a974)`, tagged
  `coder-one-speed-99e647a7a974`. Eight Coder One explorers ran at once,
  the account exceeded the `free` lane's 20 generations a minute, and each
  episode ended as `generation_failed` after 3 to 7 steps (reward 0.0).
  Coder One now waits out a rate limit (commit `9f31630ac6`). The trials
  will be rerun one at a time.

- The first `claude-code-opus` attempts on both tasks ended in about a
  second with no inference: the API refuses Opus 5.5 to Claude Code
  2.1.278 (`claude_code_version_too_old`). The arm now pins 2.1.280. The
  failed job directories are kept outside the results, under
  `~/.openagents/terminal-bench/failed/` on the Linux host.

- Three Codex `fix-git` trajectories (`smoke--codex--fix-git`, `-2`, and
  `-3`) are not valid JSON: Harbor's credential scrubber rewrote literal
  values in them. The first two were also model-rejection failures. Only
  `smoke--codex--fix-git-4` is usable.
- Devin trajectories carry one timestamp for every step, so no duration can
  be read from them.
- The external arms' trials ran on an arm64 Mac, and Coder One's on an
  x86_64 Linux host. Agent times are not strictly comparable across
  machines.
