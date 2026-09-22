# Terminal-Bench results

This page is the one place for OpenAgents' Terminal-Bench results: every
retained trial of every arm, with reward, cost, Jev cost, duration, and call
counts, plus an analysis of each Coder One run. Update it after every run.

To inspect the underlying attempts, comparisons, and retained files in the
terminal, use the [Gym TUI](../gym/terminal-bench-tui.md). The TUI reads
local Harbor jobs and the evidence linked below without running inference.

- Harness: `bench/terminal-bench/` (Harbor 0.22.0). How to run it:
  [the runbook](../coder/terminal-bench.md). The artifact contract:
  [`openagents.coder.episode.v1`](../coder/terminal-bench-contract.md).
- Operating notes for this host, credentials, rate limits, and pricing:
  [the Terminal-Bench runbook](runbook.md).
- Coder One's delegate arms: [the delegate runbook](coder-one-delegate-runbook.md).
- Tasks: the upstream Terminal-Bench repository at `3b5caaa4863d`.
- Evidence: each row links its retained trajectory under
  [`bench/terminal-bench/traces/`](../../bench/terminal-bench/traces/).
- Tracking issues: [#9530](https://github.com/OpenAgentsInc/openagents/issues/9530)
  (harness) and [#9531](https://github.com/OpenAgentsInc/openagents/issues/9531)
  (Coder One).

Every row is a single trial. Treat the tables as behavioral evidence for
development, not a pass-rate estimate or a significance claim.

## Headline: four tasks

Every arm below ran on the same task pins on the x86_64 Linux host, with Harbor's phase timings. Each cell is **reward · total cost · agent time** for one trial. ‡ marks GPT-6 costs computed by hand from OpenAI's standard pricing; § marks a Coder One cost with one generation call the door left unpriced, estimated at that run's own rate. The totals column sums all four tasks.

| Arm | `fix-git` | `build-cython-ext` | `headless-terminal` | `fix-code-vulnerability` | Passed | Total cost | Total agent time |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Claude Code 2.1.280 / Opus 5.5 | 1.0 · $0.1420 · 23.8 s | 1.0 · $0.4173 · 114.9 s | 1.0 · $0.1547 · 44.8 s | 1.0 · $0.0693 · 12.2 s | 4/4 | $0.7833 | 195.7 s |
| Codex 0.155.1 / GPT-6 Astra | 1.0 · $0.2370‡ · 47.8 s | 1.0 · $1.2447‡ · 223.6 s | 1.0 · $0.4843‡ · 185.6 s | 1.0 · $0.5293‡ · 67.0 s | 4/4 | $2.4952 | 524.0 s |
| Codex 0.155.1 / GPT-6 Sol | 1.0 · $0.1056‡ · 64.8 s | 1.0 · $0.3195‡ · 357.7 s | 1.0 · $0.2093‡ · 216.2 s | 1.0 · $0.0877‡ · 51.6 s | 4/4 | $0.7222 | 690.3 s |
| Codex 0.155.1 / GPT-6 Luna | 1.0 · $0.0052‡ · 65.3 s | 1.0 · $0.0174‡ · 204.7 s | 1.0 · $0.0032‡ · 67.7 s | 1.0 · $0.0079‡ · 49.7 s | 4/4 | $0.0336 | 387.4 s |
| Coder One alone (Gemini 3.8 Flash + Jev) | 1.0 · $0.0604 · 73.8 s | 1.0 · $0.3748 · 338.4 s | 1.0 · $0.3149 · 598.1 s | timeout · $0.2511 · 900.0 s | 3/4 | $1.0012 | 1910.3 s |
| Coder One alone, no Jev | 1.0 · $0.0557 · 71.9 s | 1.0 · $0.2069 · 245.5 s | 1.0 · $0.2950 · 361.8 s | 1.0 · $0.4119§ · 797.8 s | 4/4 | $0.9695 | 1477.0 s |
| Coder One → Opus 5.5 (explore 8) | 1.0 · $0.1055 · 47.4 s | 1.0 · $0.3905 · 128.9 s | 1.0 · $0.1610 · 68.4 s | 1.0 · $0.1136 · 73.4 s | 4/4 | $0.7706 | 318.1 s |
| Coder One → Opus 5.5 (explore 2) | 1.0 · $0.1023 · 26.0 s | 1.0 · $0.4681 · 138.3 s | 1.0 · $0.1564 · 50.2 s | 1.0 · $0.1246 · 50.6 s | 4/4 | $0.8514 | 265.1 s |
| Coder One → GPT-6 Luna (explore 8) | 1.0 · $0.0227 · 63.6 s | 1.0 · $0.0477 · 221.7 s | 1.0 · $0.0202 · 95.5 s | 1.0 · $0.0420 · 127.0 s | 4/4 | $0.1326 | 507.8 s |
| **Jev-brief → Opus 5.5** (survey only, explore 0) | 1.0 · $0.1077 · 20.5 s | 1.0 · $0.2724 · 87.1 s | 1.0 · $0.1323 · 35.6 s | 1.0 · $0.0851 · 12.6 s | 4/4 | $0.5974 | 155.8 s |
| **Jev-brief → GPT-6 Luna** (survey only, explore 0) | 0.0 · $0.0066 · 76.7 s | 1.0 · $0.0199 · 209.6 s | 1.0 · $0.0036 · 73.6 s | 1.0 · $0.0052 · 36.6 s | 3/4 | $0.0353 | 396.5 s |

**What the four tasks show:**

- **Jev-brief → Opus 5.5 is the best Coder One configuration so far.** It
  passed all four tasks for $0.5974 and 155.8 seconds of agent time in
  total, against $0.7833 and 195.7 seconds for Opus 5.5 alone: 24% cheaper
  and 20% faster. It skips the Gemini explorer: a parallel Jev survey ranks
  the files, and Opus starts from a briefing. It was faster than Opus alone
  on three of four tasks and cheaper on three of four.
- **GPT-6 Luna is the cheapest arm by far** at $0.0336 for all four tasks,
  and passed all four. Jev-brief → Luna matched its cost ($0.0353) but
  failed `fix-git`, so on Luna the briefing did not help.
- **Coder One alone is slow on the harder tasks.** It took 598 seconds on
  `headless-terminal` and timed out on `fix-code-vulnerability`. Its
  one-command-per-step loop on Gemini 3.8 Flash takes many steps, and each
  generation averaged 5 to 17 seconds. Jev per-step hints did not rescue
  it: the no-Jev ablation passed all four tasks and cost slightly less.
- **The Gemini explore phase is the expensive part of every delegate arm.**
  It was 80% to 90% of the Luna delegate's cost, and it added 20 to 80
  seconds before the delegate started. Shortening it to two steps cut time;
  removing it and letting Jev's survey build the briefing cut both.

These are single trials, and the gaps between some arms are within the
run-to-run spread we have seen (Coder One alone took 237 to 338 seconds on
`build-cython-ext` across three runs). Repetitions of Jev-brief → Opus and
Opus alone are running to test the headline result.

## Cheapest first

Every retained trial, one table per task, ordered from the cheapest run to
the most expensive. Rows with no cost (`—`) come last. The costs come from
different sources, labeled in [How to read the columns](#how-to-read-the-columns);
‡ marks GPT-6 costs computed by hand from OpenAI's standard pricing, and †
marks an agent time read from the trajectory rather than Harbor's phase
timing. A failed run's cost is still listed where it ranks, with its reward.

### `fix-git`, cheapest first

| Rank | Arm | Model | Reward | Cost | Agent time |
| --- | --- | --- | --- | --- | --- |
| 1 | Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0052‡ | 65.3 s |
| 2 | **Jev-brief → GPT-6 Luna** | Jev survey, then GPT-6 Luna | 0.0 | $0.0066 | 76.7 s |
| 3 | **Coder One → GPT-6 Luna** (explore 8) | Gemini + Jev, then GPT-6 Luna | 1.0 | $0.0227 | 63.6 s |
| 4 | **Coder One**, no Jev | Gemini 3.8 Flash | 1.0 | $0.0557 | 71.9 s |
| 5 | **Coder One** | Gemini 3.8 Flash + Jev | 1.0 | $0.0604 | 73.8 s |
| 6 | **Coder One deep** (Jev survey) | Gemini 3.8 Flash + Jev | 1.0 | $0.0625 | 75.5 s |
| 7 | **Coder One v2** (cache-stable prompt) | Gemini 3.8 Flash + Jev | 1.0 | $0.0668 | 77.0 s |
| 8 | **Coder One → Opus 5.5** (explore 2) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1023 | 26.0 s |
| 9 | **Coder One → Opus 5.5** (explore 8) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1055 | 47.4 s |
| 10 | Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.1056‡ | 64.8 s |
| 11 | **Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 | 1.0 | $0.1077 | 20.5 s |
| 12 | **Coder One → Opus 5.5** (`auto`) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1408 | 70.3 s |
| 13 | Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.1420 | 23.8 s |
| 14 | Claude Code 2.1.278 | Sonnet 4.5 | 1.0 | $0.1463 | 42.6 s† |
| 15 | Codex 0.155.1 | GPT-6 Astra | 1.0 | $0.2370‡ | 47.8 s |
| 16 | Codex 0.153.3 | GPT-6 Astra | 1.0 | $0.2607 | 53.3 s |
| 17 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $0.3630 | 30.5 s† |
| — | Devin 3000.11.1 | swe-2-high | 1.0 | — | — |
| — | Devin 3000.11.1 | claude-sonnet-5-high | 0.0 (provider refusal) | — | — |

### `build-cython-ext`, cheapest first

| Rank | Arm | Model | Reward | Cost | Agent time |
| --- | --- | --- | --- | --- | --- |
| 1 | Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0174‡ | 204.7 s |
| 2 | **Jev-brief → GPT-6 Luna** | Jev survey, then GPT-6 Luna | 1.0 | $0.0199 | 209.6 s |
| 3 | **Coder One → GPT-6 Luna** (explore 8) | Gemini + Jev, then GPT-6 Luna | 1.0 | $0.0477 | 221.7 s |
| 4 | **Coder One v2** (cache-stable prompt) | Gemini 3.8 Flash + Jev | 1.0 | $0.1597§ | 237.2 s |
| 5 | **Coder One**, no Jev | Gemini 3.8 Flash | 1.0 | $0.2069 | 245.5 s |
| 6 | **Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 | 1.0 | $0.2724 | 87.1 s |
| 7 | **Coder One deep** (Jev survey) | Gemini 3.8 Flash + Jev | 1.0 | $0.3155 | 292.3 s |
| 8 | Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.3195‡ | 357.7 s |
| 9 | **Coder One** | Gemini 3.8 Flash + Jev | 1.0 | $0.3748 | 338.4 s |
| 10 | **Coder One → Opus 5.5** (explore 8) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.3905 | 128.9 s |
| 11 | **Coder One → Opus 5.5** (`auto`) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.4145 | 153.5 s |
| 12 | Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.4173 | 114.9 s |
| 13 | **Coder One → Opus 5.5** (explore 2) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.4681 | 138.3 s |
| 14 | Claude Code 2.1.278 | Sonnet 4.5 | 0.0 | $0.9172 | 263.7 s† |
| 15 | Codex 0.155.1 | GPT-6 Astra | 1.0 | $1.2447‡ | 223.6 s |
| 16 | Codex 0.153.3 | GPT-6 Astra | 1.0 | $1.4277 | 222.2 s† |
| 17 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $1.4420 | 201.8 s† |
| — | Devin 3000.11.1 | swe-2-high | 1.0 | — | — |

### `headless-terminal`, cheapest first

| Rank | Arm | Model | Reward | Cost | Agent time |
| --- | --- | --- | --- | --- | --- |
| 1 | Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0032‡ | 67.7 s |
| 2 | **Jev-brief → GPT-6 Luna** | Jev survey, then GPT-6 Luna | 1.0 | $0.0036 | 73.6 s |
| 3 | **Coder One → GPT-6 Luna** (explore 8) | Gemini + Jev, then GPT-6 Luna | 1.0 | $0.0202 | 95.5 s |
| 4 | **Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 | 1.0 | $0.1323 | 35.6 s |
| 5 | Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.1547 | 44.8 s |
| 6 | **Coder One → Opus 5.5** (explore 2) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1564 | 50.2 s |
| 7 | **Coder One → Opus 5.5** (explore 8) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1610 | 68.4 s |
| 8 | Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.2093‡ | 216.2 s |
| 9 | **Coder One**, no Jev | Gemini 3.8 Flash | 1.0 | $0.2950 | 361.8 s |
| 10 | **Coder One** | Gemini 3.8 Flash + Jev | 1.0 | $0.3149 | 598.1 s |
| 11 | **Coder One deep** (Jev survey) | Gemini 3.8 Flash + Jev | 1.0 | $0.3247 | 471.7 s |
| 12 | Codex 0.153.3 | GPT-6 Astra | 0.0 | $0.4597 | 172.7 s† |
| 13 | Codex 0.155.1 | GPT-6 Astra | 1.0 | $0.4843‡ | 185.6 s |
| 14 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $1.2035 | 166.4 s† |
| — | Devin 3000.11.1 | swe-2-high | No result | — | — |

### `fix-code-vulnerability`, cheapest first

| Rank | Arm | Model | Reward | Cost | Agent time |
| --- | --- | --- | --- | --- | --- |
| 1 | **Jev-brief → GPT-6 Luna** | Jev survey, then GPT-6 Luna | 1.0 | $0.0052 | 36.6 s |
| 2 | Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0079‡ | 49.7 s |
| 3 | **Coder One → GPT-6 Luna** (explore 8) | Gemini + Jev, then GPT-6 Luna | 1.0 | $0.0420 | 127.0 s |
| 4 | Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.0693 | 12.2 s |
| 5 | **Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 | 1.0 | $0.0851 | 12.6 s |
| 6 | Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.0877‡ | 51.6 s |
| 7 | **Coder One → Opus 5.5** (explore 8) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1136 | 73.4 s |
| 8 | **Coder One → Opus 5.5** (explore 2) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1246 | 50.6 s |
| 9 | **Coder One deep** (Jev survey) | Gemini 3.8 Flash + Jev | 0.0 | $0.2281§ | 419.7 s |
| 10 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $0.2333 | 24.7 s† |
| 11 | **Coder One** | Gemini 3.8 Flash + Jev | timeout | $0.2511 | 900.0 s |
| 12 | **Coder One**, no Jev | Gemini 3.8 Flash | 1.0 | $0.4119§ | 797.8 s |
| 13 | Codex 0.153.3 | GPT-6 Astra | 1.0 | $0.5094 | 44.2 s† |
| 14 | Codex 0.155.1 | GPT-6 Astra | 1.0 | $0.5293‡ | 67.0 s |
| — | Devin 3000.11.1 | swe-2-high | 1.0 | — | — |

### `cancel-async-tasks`, cheapest first

| Rank | Arm | Model | Reward | Cost | Agent time |
| --- | --- | --- | --- | --- | --- |
| 1 | Codex 0.153.3 | GPT-6 Astra | 1.0 | $0.1781 | 54.2 s† |
| 2 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $0.5988 | 92.8 s† |
| — | Devin 3000.11.1 | swe-2-high | No result | — | — |

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
  - *Manual list price* ‡: GPT-6 costs computed by hand from OpenAI's
    standard pricing, which the operator supplied on 2026-09-22, not from
    Harbor, which has no price for these models under Codex 0.155.1. See
    [GPT-6 pricing](#gpt-6-pricing).
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
| Codex 0.155.1 | GPT-6 Astra | 1.0 | $0.2370‡ | Manual list price | — | 47.8 | 6 | 5 | 107,937 / 98,944 / 963 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-astra--fix-git/) |
| Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.1056‡ | Manual list price | — | 64.8 | 11 | 10 | 209,964 / 189,312 / 2,646 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-sol--fix-git/) |
| Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0052‡ | Manual list price | — | 65.3 | 9 | 8 | 166,488 / 142,080 / 2,724 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-luna--fix-git/) |
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
| Codex 0.155.1 | GPT-6 Astra | 1.0 | $1.2447‡ | Manual list price | — | 223.6 | 18 | 17 | 652,905 / 613,376 / 4,720 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-astra--build-cython-ext/) |
| Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.3195‡ | Manual list price | — | 357.7 | 28 | 27 | 936,445 / 896,000 / 5,943 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-sol--build-cython-ext/) |
| Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0174‡ | Manual list price | — | 204.7 | 31 | 30 | 952,216 / 904,448 / 7,151 | [trace](../../bench/terminal-bench/traces/smoke--codex-gpt-6-luna--build-cython-ext/) |
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

The GPT-6 costs are **manual**: computed by hand from OpenAI's standard
pricing, which the operator supplied on 2026-09-22, not reported by Harbor.
Harbor records no cost for these models under Codex 0.155.1. Prices are per
million tokens, standard tier, short context; every request here was well
under the long-context threshold.

| Model | Input | Cached input | Cache writes | Output |
| --- | --- | --- | --- | --- |
| GPT-6 Astra | $10.00 | $1.00 | $12.50 | $50.00 |
| GPT-6 Sol | $2.00 | $0.20 | $2.50 | $10.00 |
| GPT-6 Luna | $0.10 | $0.01 | $0.125 | $0.50 |

Cost = uncached input × input rate + cached input × cached rate + output
× output rate. Codex reports cached input but not cache writes, so uncached
input is priced at the ordinary input rate; if some of it was billed as cache
writes, the true cost is up to 25% higher on that part. The method matches
Harbor's: repricing the two earlier Codex 0.153.3 GPT-6 Astra trials this
way gives $0.2607 and $1.4277, exactly Harbor's figures.

| Trial | Uncached input | Cached input | Output | Cost |
| --- | --- | --- | --- | --- |
| GPT-6 Astra, `fix-git` | 8,993 | 98,944 | 963 | $0.2370 |
| GPT-6 Astra, `build-cython-ext` | 39,529 | 613,376 | 4,720 | $1.2447 |
| GPT-6 Sol, `fix-git` | 20,652 | 189,312 | 2,646 | $0.1056 |
| GPT-6 Sol, `build-cython-ext` | 40,445 | 896,000 | 5,943 | $0.3195 |
| GPT-6 Luna, `fix-git` | 24,408 | 142,080 | 2,724 | $0.0052 |
| GPT-6 Luna, `build-cython-ext` | 47,768 | 904,448 | 7,151 | $0.0174 |

### Not yet run

`vllm-deepseek-streaming` and `batched-eval-parity` are in the CPU panel and
have no retained trials. `math-eval-grader` needs an H100 and stays excluded
locally. The four tasks above carry the current comparison; their per-arm
detail for the newer arms is in the headline and cheapest-first tables, and
every trial's evidence is under `bench/terminal-bench/traces/`.

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

### Jev and delegate experiments, 2026-09-22

Each variant changes one thing about how Coder One uses Jev or a delegate.
Artifacts: `9f31630ac600` (v2 and deep reruns), `1a9c712f158b` (no-Jev,
deep on the new tasks, explore 2, Luna delegate), and `d6626285b583`
(Jev-brief).

| Variant | What changes | Result across the four tasks |
| --- | --- | --- |
| **v2**: cache-stable prompt | Unchanging parts first, a per-run `prompt_cache_key` | `fix-git` $0.0668, 77.0 s; `build-cython-ext` about $0.16, 237.2 s. Gemini cached almost nothing: 4,031 tokens in one call. Vertex's implicit cache is best-effort, and a door change now forwards `x-session-affinity`. |
| **deep**: Jev survey before step 1 | Up to 100 files judged in parallel; the top files' contents enter the prompt | Found nothing to survey on `build-cython-ext` (the task clones its repository first) and little on `fix-git`. On `headless-terminal` 1.0 in 471.7 s; on `fix-code-vulnerability` 0.0. No gain over Coder One alone. |
| **no Jev**: the ablation | No Jev calls at all | 4 of 4, $0.9695 and 1,477 s in total, against 3 of 4, $1.0012 and 1,910 s with Jev. Jev's per-step hints did not help the generate-and-run loop on these tasks. |
| **explore 2** → Opus 5.5 | Delegate after two explore steps instead of eight | 4 of 4, $0.8514, 265.1 s. Faster than explore 8 (318.1 s) but not cheaper. |
| **Luna delegate**, explore 8 | Codex on GPT-6 Luna finishes | 4 of 4 for $0.1326; 80% to 90% of that was the Gemini explorer. |
| **Jev-brief → Opus 5.5** | No explorer: Jev's survey builds the briefing, Opus finishes | **4 of 4, $0.5974, 155.8 s**: 24% cheaper and 20% faster than Opus 5.5 alone. |
| **Jev-brief → Luna** | Same, with GPT-6 Luna | 3 of 4, $0.0353, 396.5 s. Failed `fix-git`; no better than Luna alone. |

**Why Jev-brief → Opus beat Opus alone.** The clearest gain is on
`build-cython-ext`, where Opus needed 11 turns from the briefing against 17
steps from scratch; on the other three tasks the counts were within one (7,
6, and 3 turns against 7, 5, and 3 steps). The whole Jev survey took under
four seconds. On
`headless-terminal` and `fix-code-vulnerability`, the survey put the file
to change in front of Opus (`base_terminal.py`, `bottle.py`). On `fix-git`
and `build-cython-ext` it found nothing (the briefing was 801 and 2,257
characters, mostly the instruction), so the gain there came from the
briefing's framing or from run-to-run variation, not from Jev's file
selection. The repetitions running now separate the two.

**Why Coder One alone stays slow.** Every step is one sequential Gemini
generation. On the harder tasks it took 30 to 50 steps and 5 to 17 seconds
per generation. Neither caching nor Jev hints changed the step count.
Cutting time means fewer model turns, which the delegate arms deliver.

## After every Coder One run

1. Check the result: `uv run tbench inspect <job>`, and print every
   number a row needs with `python3 tools/trial_metrics.py <job>` from
   `bench/terminal-bench`.
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

- Costs marked § include one generation call the door left unpriced (in
  each case the call that read cached tokens); it is estimated at that
  run's own cost per token.
- The first Jev-brief trials recorded a null total because an episode with
  no generation counted its generation cost as unknown instead of zero
  (fixed in `1f883564aa`). Their totals here are the Jev and delegate costs
  added by hand.

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
