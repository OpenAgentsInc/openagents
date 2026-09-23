# Coder One run analyses, 2026-09-22

[Current status](README.md) · [Development results](development-results.md) · [Publication procedure](runbook.md#after-each-run)

This is the analysis of the original loop and delegate experiments on
2026-09-22. The loop described below predates the
[tunable composition](../coder/guides/coder-one-tunable.md).

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
| **Lean Jev-brief → Opus 5.5** | Jev-brief with Claude Code limited to six tools (`CODER_ONE_DELEGATE_TOOLS`) | **12 of 12 over three repetitions, $0.4887 and 164.3 s** (sums of per-task means): 25% cheaper and 16% faster than Opus 5.5 alone. The first Opus call's input fell from about 16,400–20,100 tokens to 6,900–9,800. |

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
