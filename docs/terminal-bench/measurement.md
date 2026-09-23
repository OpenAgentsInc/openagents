# Terminal-Bench measurement and pricing

[Current status](README.md) · [Data quality](data-quality.md)

These definitions and historical rates explain the results recorded on
2026-09-22 and 2026-09-23. They are not a current price quote. Keep the
cost source, rate date, and any missing charges with every new result.

## Populations and comparisons

A table can describe one trial, repeated trials of one task, or a suite.
For the eight-task development summaries, cost and agent time are **sums
of per-task means**. With three trials per task, total spend across all 24
trials is three times that cost before rounding; elapsed suite wall time
is not the sum of agent times when trials run concurrently.

For a matched leaderboard comparison on a task set S, sum each task's
successes divided by trials to obtain expected passes. For matched mean
cost, sum each task's total cost divided by its trial count, then divide
by the number of tasks in S. Do not use the full leaderboard's mean cost
against a selected subset. See the [corrected TB4 comparison](tb4-results.md#matched-task-comparison).

Keep unknown costs unknown. A recorded lower bound excludes missing
charges and does not establish a savings percentage. A displayed zero
from a report that skips unknown charges is not evidence of a free trial.
List-price estimates from subscription sessions are not cash bills.

A verifier reward alone does not establish a valid agent trial. Report
quota, authentication, setup, and wrong-artifact attempts separately,
retain their costs and evidence, and link any replacement attempt. A
best-of-several-versions result measures a selected portfolio, not one
policy. Development screens and routing fitted on the benchmark need a
held-out evaluation before generalizing.

## How to read the columns

- **Reward** is the task's own verifier result. *No result* means the
  verifier produced no reward, so the attempt is unverifiable, not a zero.
- **Cost** is in US dollars. The sources differ by arm, so the **Cost
  source** column names each one:
  - *CLI list price*: Claude Code's own `total_cost_usd`, which it marks
    `costBasis: "list"`: the run's tokens priced at Anthropic's published API
    rates. These runs signed in with a Claude subscription, so nothing was
    billed per token; the figure is what the same tokens cost at API list
    price. See [Claude pricing](#claude-pricing).
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

## Claude pricing

Claude Code prices every run itself from its token counts at Anthropic's
API list prices ([Anthropic pricing](https://platform.claude.com/docs/en/about-claude/pricing),
retrieved 2026-09-22). The rates for the models these trials use, per
million tokens:

| Model | Input | 5-minute cache write | 1-hour cache write | Cache read | Output |
| --- | --- | --- | --- | --- | --- |
| Opus 5.5 | $4.00 | $5.00 | $8.00 | $0.20 | $20.00 |
| Sonnet 5 | $2.00 | $2.50 | $4.00 | $0.20 | $10.00 |
| Haiku 4.5 | $1.00 | $1.25 | $2.00 | $0.10 | $5.00 |
| Fable 5.1 | $10.00 | $12.50 | $20.00 | $0.25 | $50.00 |
| Sonnet 4.5 | $3.00 | $3.75 | $6.00 | $0.30 | $15.00 |

Cache writes cost 1.25× the input rate for a 5-minute cache and 2× for a
1-hour cache. A cache read costs 0.1× the input rate, except 0.05× on Opus
5.5 and 0.025× on Fable 5.1. Thinking tokens bill as output.

Cost = input × input rate + 5-minute writes × its rate + 1-hour writes × its
rate + cache reads × the read rate + output × the output rate. These early Claude Code trials
use a one-hour cache; later arms explicitly test five-minute caching. For example, Opus 5.5 on `headless-terminal`:

```text
       10 input          × $4.00/M  = $0.00004
    7,707 1-hour writes  × $8.00/M  = $0.06166
   79,040 cache reads    × $0.20/M  = $0.01581
    3,860 output         × $20.00/M = $0.07720
                                    = $0.15470
```

That equals Claude Code's reported `total_cost_usd` exactly. The same formula
reproduces the reported cost of all 12 Claude Code on Opus 5.5 trials to
within rounding.

Because these runs used a subscription, the costs are comparable list-price
figures, not charges. The same holds for Codex (a ChatGPT subscription, with
GPT-6 priced by hand below) and for Coder One's generation on the
openagents.com `free` lane. Jev is the exception: its key is pay-as-you-go
at the published $0.042 per million input tokens.

## GPT-6 pricing

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
