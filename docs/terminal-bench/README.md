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
- Why the two winning configurations were cheap and fast, and the upgrade
  plan: [the winning-runs analysis](winning-runs-analysis.md).
- How Coder becomes a per-task policy of tunable components, and the
  system that tunes it: [Coder as a tunable system](../optimization/coder-components.md).
- Full-day review and proposed Luna/Jev upgrade:
  [requirement coverage, evidence packing, and bounded repair](2026-09-22-luna-jevprobe-upgrade.md).
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
| **Lean Jev-brief → Opus 5.5** (six Claude Code tools) | 1.0 · $0.0701 · 15.5 s | 1.0 · $0.2856 · 98.1 s | 1.0 · $0.1176 · 45.3 s | 1.0 · $0.0576 · 13.3 s | 4/4 | $0.5309 | 172.2 s |
| **Jev-brief → GPT-6 Luna** (survey only, explore 0) | 0.0 · $0.0066 · 76.7 s | 1.0 · $0.0199 · 209.6 s | 1.0 · $0.0036 · 73.6 s | 1.0 · $0.0052 · 36.6 s | 3/4 | $0.0353 | 396.5 s |

### Repeated runs: Coder One's best configuration against Opus 5.5

Three trials per task for each arm. **Lean Jev-brief → Opus 5.5** runs no
Gemini explorer: a parallel Jev survey (under four seconds) builds a
briefing, and Claude Code on Opus 5.5 finishes with six tools (Bash, Read,
Edit, Write, Glob, Grep), which roughly halves its fixed prompt on every
call.

| Arm | Task | Passed | Cost, mean (min–max) | Agent time, mean (min–max) |
| --- | --- | --- | --- | --- |
| Claude Code 2.1.280 / Opus 5.5 | `fix-git` | 3/3 | $0.1216 ($0.0819–$0.1420) | 23.5 s (22.8–23.9) |
| Claude Code 2.1.280 / Opus 5.5 | `build-cython-ext` | 3/3 | $0.3481 ($0.3011–$0.4173) | 111.8 s (100.1–120.5) |
| Claude Code 2.1.280 / Opus 5.5 | `headless-terminal` | 3/3 | $0.1456 ($0.1283–$0.1547) | 49.1 s (44.8–51.9) |
| Claude Code 2.1.280 / Opus 5.5 | `fix-code-vulnerability` | 3/3 | $0.0400 ($0.0253–$0.0693) | 11.4 s (11.0–12.2) |
| Jev-brief → Opus 5.5 | `fix-git` | 3/3 | $0.0781 ($0.0614–$0.1077) | 18.8 s (16.2–20.5) |
| Jev-brief → Opus 5.5 | `build-cython-ext` | 3/3 | $0.2824 ($0.2712–$0.3038) | 96.3 s (87.1–105.0) |
| Jev-brief → Opus 5.5 | `headless-terminal` | 3/3 | $0.1272 ($0.1091–$0.1404) | 45.1 s (35.6–50.3) |
| Jev-brief → Opus 5.5 | `fix-code-vulnerability` | 3/3 | $0.0661 ($0.0291–$0.0851) | 12.1 s (11.8–12.6) |
| **Lean Jev-brief → Opus 5.5** | `fix-git` | 3/3 | $0.0578 ($0.0502–$0.0701) | 16.2 s (15.4–17.8) |
| **Lean Jev-brief → Opus 5.5** | `build-cython-ext` | 3/3 | $0.2834 ($0.1911–$0.3736) | 93.2 s (82.7–98.8) |
| **Lean Jev-brief → Opus 5.5** | `headless-terminal` | 3/3 | $0.1113 ($0.1048–$0.1176) | 42.6 s (38.7–45.3) |
| **Lean Jev-brief → Opus 5.5** | `fix-code-vulnerability` | 3/3 | $0.0362 ($0.0248–$0.0576) | 12.2 s (11.5–13.3) |

| Arm | Passed | Sum of per-task mean cost | Sum of per-task mean agent time |
| --- | --- | --- | --- |
| Claude Code 2.1.280 / Opus 5.5 | 12/12 | $0.6554 | 195.9 s |
| Jev-brief → Opus 5.5 | 12/12 | $0.5539 | 172.2 s |
| **Lean Jev-brief → Opus 5.5** | 12/12 | $0.4887 | 164.3 s |

Across 12 trials each, the lean arm passed every task for 25% less than
Opus 5.5 alone ($0.4887 against $0.6554, summing the per-task means) and in
16% less agent time (164.3 against 195.9 seconds). Its mean cost is lower on
all four tasks; its mean time is lower on three, and 0.8 seconds higher on
`fix-code-vulnerability`. The ranges overlap on most tasks, so treat the
per-task differences as a development result rather than a significance
claim; the totals are consistent in direction across tasks.

**What the four tasks show:**

- **Lean Jev-brief → Opus 5.5 is now the best configuration:** 12 of 12
  over three repetitions, 25% cheaper and 16% faster than Opus 5.5 alone.
  See the repeated-runs table above.
- **Jev-brief → Opus 5.5 (full Claude Code) was the first to beat Opus.** It
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
`build-cython-ext` across three runs). Three repetitions of the Opus arms
confirmed the direction of the headline result, with smaller margins
(15% cheaper and 12% faster for Jev-brief; 25% and 16% for the lean arm).

### Jev-probe arms, 2026-09-22

The Jev-probe arms run no Gemini at all. Before delegating, the host runs a
battery of read-only probes in parallel (listing, git state, README, tests,
versions, paths the task names), Jev keeps the outputs the task needs, and
those join Jev's file survey in the briefing. The host's part, Jev
included, takes 1 to 5 seconds. [The winning-runs analysis](winning-runs-analysis.md)
takes the two best arms apart.

**Jev-probe v2** ([#9535](https://github.com/OpenAgentsInc/openagents/issues/9535),
`CODER_ONE_PROBE_V2=on`, artifact `coder-one 0.1.0 (03401dad7483)`) adds a
Jev-gated setup pack that runs the clone and install steps a task names,
git probes in named repositories, whole files for likely edit targets, a
40-file survey pool, and directions to work in few, large steps. The
earlier probe arms ran artifact `14c95a…` (`774c2a9340a2`).

**Jev-probe v3** (`CODER_ONE_PROBE_V2=v3`, artifact `coder-one 0.1.0
(88f47d67b405)`) keeps v2 and replaces "run one final check" with
directions to run the checks the task names, exercise every changed code
path, and search a bulk find-and-replace for misses and doubles. Its Opus
arm also sets `CLAUDE_CODE_PROMPT_CACHE_TTL=5m`, so Claude Code writes the
five-minute cache instead of the subscription token's one-hour cache; the
retained streams show only `ephemeral_5m_input_tokens` writes. A third arm,
`coder-one-jevprobe2-opus-lean-low-5m`, runs v2's directions on the v2
artifact with the five-minute cache, to separate the two changes.

Three trials per task per arm, on the four panel tasks:

| Arm | Task | Passed | Cost, mean (min–max) | Agent time, mean (min–max) | Delegate turns, mean |
| --- | --- | --- | --- | --- | --- |
| **Jev-probe v3 → Luna**‡ | `fix-git` | 3/3 | $0.0029 ($0.0027–$0.0031) | 34.2 s (29.4–42.1) | 8.3 |
| **Jev-probe v3 → Luna**‡ | `build-cython-ext` | 3/3 | $0.0120 ($0.0099–$0.0159) | 163.8 s (146.9–182.6) | 18.7 |
| **Jev-probe v3 → Luna**‡ | `headless-terminal` | 2/3 | $0.0019 ($0.0015–$0.0026) | 47.7 s (41.4–59.2) | 3.7 |
| **Jev-probe v3 → Luna**‡ | `fix-code-vulnerability` | 3/3 | $0.0031 ($0.0029–$0.0032) | 29.4 s (29.1–29.8) | 5.7 |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | `fix-git` | 3/3 | $0.0439 ($0.0431–$0.0453) | 14.6 s (13.4–15.7) | 4.0 |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | `build-cython-ext` | 3/3 | $0.1409 ($0.0970–$0.1851) | 111.9 s (82.0–137.3) | 8.7 |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | `headless-terminal` | 3/3 | $0.0544 ($0.0522–$0.0577) | 23.7 s (21.9–25.5) | 2.0 |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | `fix-code-vulnerability` | 3/3 | $0.0462 ($0.0457–$0.0468) | 12.1 s (11.4–12.5) | 3.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | `fix-git` | 3/3 | $0.0418 ($0.0405–$0.0427) | 14.7 s (13.5–15.8) | 4.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | `build-cython-ext` | 3/3 | $0.1047 ($0.0895–$0.1128) | 87.7 s (83.2–92.6) | 6.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | `headless-terminal` | 3/3 | $0.0513 ($0.0499–$0.0534) | 20.3 s (19.8–21.2) | 2.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | `fix-code-vulnerability` | 3/3 | $0.0454 ($0.0451–$0.0456) | 11.1 s (10.7–11.5) | 3.0 |
| Jev-probe → Luna‡ | `fix-git` | 3/3 | $0.0032 ($0.0028–$0.0036) | 36.0 s (30.1–44.1) | 8.0 |
| Jev-probe → Luna‡ | `build-cython-ext` | 3/3 | $0.0119 ($0.0088–$0.0158) | 185.8 s (180.0–194.0) | 25.7 |
| Jev-probe → Luna‡ | `headless-terminal` | 3/3 | $0.0032 ($0.0024–$0.0043) | 65.2 s (51.9–83.9) | 7.7 |
| Jev-probe → Luna‡ | `fix-code-vulnerability` | 3/3 | $0.0035 ($0.0034–$0.0036) | 31.1 s (28.4–35.0) | 6.7 |
| **Jev-probe v2 → Luna**‡ | `fix-git` | 3/3 | $0.0028 ($0.0026–$0.0030) | 30.9 s (30.0–31.4) | 7.7 |
| **Jev-probe v2 → Luna**‡ | `build-cython-ext` | 0/3 | $0.0135 ($0.0098–$0.0156) | 158.3 s (136.1–169.9) | 19.0 |
| **Jev-probe v2 → Luna**‡ | `headless-terminal` | 3/3 | $0.0021 ($0.0014–$0.0033) | 51.0 s (37.9–76.6) | 4.7 |
| **Jev-probe v2 → Luna**‡ | `fix-code-vulnerability` | 3/3 | $0.0030 ($0.0028–$0.0032) | 25.1 s (23.5–27.0) | 5.3 |
| Jev-probe → lean Opus 5.5, low effort | `fix-git` | 3/3 | $0.0563 ($0.0548–$0.0572) | 14.9 s (13.6–16.0) | 4.0 |
| Jev-probe → lean Opus 5.5, low effort | `build-cython-ext` | 3/3 | $0.1270 ($0.0873–$0.1608) | 78.0 s (60.9–99.0) | 7.7 |
| Jev-probe → lean Opus 5.5, low effort | `headless-terminal` | 3/3 | $0.0599 ($0.0589–$0.0605) | 20.7 s (19.7–21.8) | 2.0 |
| Jev-probe → lean Opus 5.5, low effort | `fix-code-vulnerability` | 3/3 | $0.0642 ($0.0631–$0.0658) | 11.5 s (10.9–12.7) | 3.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort** | `fix-git` | 3/3 | $0.0549 ($0.0543–$0.0554) | 13.6 s (12.9–14.0) | 4.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort** | `build-cython-ext` | 3/3 | $0.1456 ($0.1219–$0.1759) | 74.4 s (66.9–82.9) | 6.3 |
| **Jev-probe v2 → lean Opus 5.5, low effort** | `headless-terminal` | 3/3 | $0.0613 ($0.0554–$0.0658) | 21.3 s (20.2–22.1) | 2.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort** | `fix-code-vulnerability` | 3/3 | $0.0631 ($0.0624–$0.0636) | 11.3 s (10.1–12.7) | 3.0 |
| Jev-probe → lean Opus 5.5 | `fix-git` | 3/3 | $0.0742 ($0.0724–$0.0761) | 18.2 s (18.0–18.4) | 5.0 |
| Jev-probe → lean Opus 5.5 | `build-cython-ext` | 3/3 | $0.2194 ($0.1931–$0.2371) | 90.3 s (85.1–98.9) | 11.3 |
| Jev-probe → lean Opus 5.5 | `headless-terminal` | 3/3 | $0.1078 ($0.0833–$0.1212) | 36.4 s (30.5–40.6) | 4.3 |
| Jev-probe → lean Opus 5.5 | `fix-code-vulnerability` | 3/3 | $0.0704 ($0.0639–$0.0771) | 12.9 s (10.7–14.5) | 3.3 |

| Arm | Passed | Sum of per-task mean cost | Sum of per-task mean agent time |
| --- | --- | --- | --- |
| **Jev-probe v3 → Luna**‡ | 11/12 | $0.0198 | 275.2 s |
| **Jev-probe v2 → Luna**‡ | 9/12 | $0.0214 | 265.3 s |
| Jev-probe → Luna‡ | 12/12 | $0.0219 | 318.1 s |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | 12/12 | $0.2433 | 133.8 s |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | 12/12 | $0.2853 | 162.4 s |
| Jev-probe → lean Opus 5.5, low effort | 12/12 | $0.3075 | 125.1 s |
| **Jev-probe v2 → lean Opus 5.5, low effort** | 12/12 | $0.3249 | 120.7 s |
| Jev-probe → lean Opus 5.5 | 12/12 | $0.4717 | 157.7 s |

On four tasks the panel doesn't cover (the `extended` profile), against
both direct baselines, three trials each:

| Arm | Task | Passed | Cost, mean (min–max) | Agent time, mean (min–max) | Delegate turns, mean |
| --- | --- | --- | --- | --- | --- |
| Claude Code 2.1.280 / Opus 5.5 | `cancel-async-tasks` | 3/3 | $0.1833 ($0.1497–$0.2385) | 51.5 s (45.3–63.5) | — |
| Claude Code 2.1.280 / Opus 5.5 | `git-leak-recovery` | 3/3 | $0.0749 ($0.0601–$0.0867) | 19.5 s (15.0–26.4) | — |
| Claude Code 2.1.280 / Opus 5.5 | `log-summary-date-ranges` | 3/3 | $0.0945 ($0.0826–$0.1055) | 19.4 s (18.6–21.1) | — |
| Claude Code 2.1.280 / Opus 5.5 | `sqlite-db-truncate` | 3/3 | $0.0781 ($0.0652–$0.1007) | 20.1 s (19.6–20.6) | — |
| Codex 0.155.1 / GPT-6 Luna‡ | `cancel-async-tasks` | 1/3 | $0.0016 ($0.0014–$0.0021) | 37.9 s (35.3–42.4) | — |
| Codex 0.155.1 / GPT-6 Luna‡ | `git-leak-recovery` | 2/3 | $0.0028 ($0.0026–$0.0029) | 58.4 s (56.0–61.3) | — |
| Codex 0.155.1 / GPT-6 Luna‡ | `log-summary-date-ranges` | 3/3 | $0.0016 ($0.0009–$0.0022) | 26.3 s (24.3–28.1) | — |
| Codex 0.155.1 / GPT-6 Luna‡ | `sqlite-db-truncate` | 3/3 | $0.0035 ($0.0030–$0.0040) | 67.9 s (58.3–78.7) | — |
| **Jev-probe v3 → Luna**‡ | `cancel-async-tasks` | 2/3 | $0.0015 ($0.0014–$0.0017) | 35.9 s (33.2–37.7) | 4.0 |
| **Jev-probe v3 → Luna**‡ | `git-leak-recovery` | 3/3 | $0.0015 ($0.0014–$0.0016) | 26.6 s (24.5–27.8) | 5.7 |
| **Jev-probe v3 → Luna**‡ | `log-summary-date-ranges` | 0/3 | $0.0019 ($0.0016–$0.0021) | 25.4 s (24.6–27.0) | 3.0 |
| **Jev-probe v3 → Luna**‡ | `sqlite-db-truncate` | 3/3 | $0.0039 ($0.0022–$0.0060) | 57.2 s (40.0–91.4) | 6.0 |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | `cancel-async-tasks` | 3/3 | $0.0489 ($0.0466–$0.0512) | 17.7 s (16.7–19.3) | 2.3 |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | `git-leak-recovery` | 3/3 | $0.0276 ($0.0274–$0.0279) | 10.3 s (9.5–11.0) | 3.0 |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | `log-summary-date-ranges` | 3/3 | $0.0710 ($0.0678–$0.0727) | 19.8 s (15.8–26.7) | 3.7 |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | `sqlite-db-truncate` | 3/3 | $0.0532 ($0.0494–$0.0552) | 19.5 s (18.2–20.3) | 4.3 |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | `cancel-async-tasks` | 3/3 | $0.0453 ($0.0443–$0.0467) | 16.6 s (15.9–17.8) | 2.3 |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | `git-leak-recovery` | 3/3 | $0.0277 ($0.0274–$0.0283) | 10.6 s (9.8–11.5) | 3.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | `log-summary-date-ranges` | 3/3 | $0.0672 ($0.0651–$0.0699) | 14.3 s (12.9–15.9) | 3.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | `sqlite-db-truncate` | 3/3 | $0.0447 ($0.0300–$0.0557) | 16.2 s (10.8–20.1) | 3.3 |
| Jev-probe → Luna‡ | `cancel-async-tasks` | 0/3 | $0.0012 ($0.0009–$0.0017) | 24.1 s (19.8–28.5) | 3.3 |
| Jev-probe → Luna‡ | `git-leak-recovery` | 3/3 | $0.0019 ($0.0017–$0.0020) | 27.8 s (24.8–31.3) | 6.7 |
| Jev-probe → Luna‡ | `log-summary-date-ranges` | 0/3 | $0.0027 ($0.0024–$0.0032) | 24.6 s (23.6–25.4) | 3.0 |
| Jev-probe → Luna‡ | `sqlite-db-truncate` | 3/3 | $0.0042 ($0.0040–$0.0045) | 43.5 s (40.3–47.4) | 6.0 |
| **Jev-probe v2 → Luna**‡ | `cancel-async-tasks` | 1/3 | $0.0016 ($0.0009–$0.0024) | 37.1 s (22.1–61.3) | 4.3 |
| **Jev-probe v2 → Luna**‡ | `git-leak-recovery` | 3/3 | $0.0016 ($0.0013–$0.0022) | 22.9 s (19.8–27.5) | 5.0 |
| **Jev-probe v2 → Luna**‡ | `log-summary-date-ranges` | 1/3 | $0.0022 ($0.0019–$0.0026) | 27.8 s (21.8–38.7) | 3.3 |
| **Jev-probe v2 → Luna**‡ | `sqlite-db-truncate` | 3/3 | $0.0039 ($0.0030–$0.0050) | 57.0 s (35.1–72.9) | 6.3 |
| Jev-probe → lean Opus 5.5, low effort | `cancel-async-tasks` | 3/3 | $0.0613 ($0.0496–$0.0760) | 20.9 s (15.6–24.8) | 3.7 |
| Jev-probe → lean Opus 5.5, low effort | `git-leak-recovery` | 3/3 | $0.0370 ($0.0332–$0.0402) | 11.6 s (10.9–12.5) | 3.7 |
| Jev-probe → lean Opus 5.5, low effort | `log-summary-date-ranges` | 3/3 | $0.0916 ($0.0858–$0.1010) | 14.8 s (12.1–16.4) | 3.3 |
| Jev-probe → lean Opus 5.5, low effort | `sqlite-db-truncate` | 3/3 | $0.0583 ($0.0561–$0.0610) | 17.8 s (17.1–18.3) | 4.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort** | `cancel-async-tasks` | 3/3 | $0.0487 ($0.0480–$0.0491) | 15.2 s (14.5–16.0) | 2.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort** | `git-leak-recovery` | 3/3 | $0.0348 ($0.0332–$0.0364) | 10.0 s (9.5–10.4) | 3.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort** | `log-summary-date-ranges` | 3/3 | $0.0882 ($0.0878–$0.0884) | 13.6 s (13.1–14.2) | 3.0 |
| **Jev-probe v2 → lean Opus 5.5, low effort** | `sqlite-db-truncate` | 3/3 | $0.0556 ($0.0533–$0.0585) | 18.4 s (17.0–20.1) | 2.7 |

| Arm | Passed | Sum of per-task mean cost | Sum of per-task mean agent time |
| --- | --- | --- | --- |
| **Jev-probe v3 → Luna**‡ | 8/12 | $0.0087 | 145.0 s |
| **Jev-probe v2 → Luna**‡ | 8/12 | $0.0093 | 144.9 s |
| Codex 0.155.1 / GPT-6 Luna‡ | 9/12 | $0.0095 | 190.4 s |
| Jev-probe → Luna‡ | 6/12 | $0.0100 | 120.1 s |
| **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | 12/12 | $0.1849 | 57.8 s |
| **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | 12/12 | $0.2008 | 67.4 s |
| **Jev-probe v2 → lean Opus 5.5, low effort** | 12/12 | $0.2273 | 57.2 s |
| Jev-probe → lean Opus 5.5, low effort | 12/12 | $0.2482 | 65.1 s |
| Claude Code 2.1.280 / Opus 5.5 | 12/12 | $0.4309 | 110.5 s |

One trial per task of the other delegates, on the panel tasks. Each cell is
**reward · total cost · agent time**:

| Arm | `fix-git` | `build-cython-ext` | `headless-terminal` | `fix-code-vulnerability` | Passed | Total cost | Total agent time |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Jev-probe → Luna, low effort‡ | 1.0 · $0.0035 · 39.4 s | 1.0 · $0.0173 · 194.4 s | 1.0 · $0.0026 · 55.1 s | 1.0 · $0.0044 · 30.0 s | 4/4 | $0.0278 | 318.9 s |
| Jev-probe → lean Sonnet 5, low effort | 1.0 · $0.1035 · 22.7 s | 1.0 · $0.2891 · 127.0 s | 1.0 · $0.0471 · 23.1 s | 1.0 · $0.0629 · 12.4 s | 4/4 | $0.5026 | 185.2 s |
| Jev-probe → lean Haiku 4.5 | 1.0 · $0.0391 · 26.2 s | 1.0 · $0.4886 · 223.8 s | 1.0 · $0.0392 · 39.8 s | 1.0 · $0.0684 · 40.3 s | 4/4 | $0.6352 | 330.0 s |

**What the Jev-probe runs show:**

- **Jev-probe v2 → lean Opus at low effort passed all 24 trials** and is
  the fastest configuration on both sets: 120.7 seconds on the panel (the
  earlier probe arm took 125.1) and 57.2 seconds on the new tasks, against
  110.5 seconds for Opus 5.5 alone at 47% lower cost ($0.2273 against
  $0.4309).
- **v2 is cheaper for Opus on the new tasks and dearer on
  `build-cython-ext`.** There the setup pack clones the repository before
  the survey, so the survey finds the source and the briefing grows from
  2,783 to about 9,500 characters, which Claude Code writes to its one-hour
  cache.
- **v2 made Luna faster where it passed, and cut Jev's cost by a quarter**
  ($0.00044 to $0.00033 a run). Luna's delegate turns fell on
  `build-cython-ext` (25.7 to 19.0) and `headless-terminal` (7.7 to 4.7).
- **v2 → Luna failed `build-cython-ext` in all three trials.** Each run
  left one NumPy alias wrong (`np.int` unreplaced, or replaced twice as
  `np.int6464`), and its own checks didn't reach the code path the
  verifier exercises. The earlier probe arm passed all three. The likely
  cause is v2's "run one final check" direction.
- **Luna is unreliable on the new tasks with or without Jev.** Luna direct
  passed 9 of 12, the probe arm 6 of 12, and v2 8 of 12. On
  `log-summary-date-ranges` the probe arms counted 414 `ERROR` lines where
  the verifier expects 370, reading the word anywhere in a line rather
  than the severity field.
- **v3 fixed Luna's `build-cython-ext`** (3 of 3, against 0 of 3 for v2)
  and made Jev-probe v3 → Luna the cheapest arm that passes the panel's
  hard task: $0.0198 across the four tasks, 11 of 12 trials. Its one miss
  was a `headless-terminal` run that never created `/app/vim.txt`.
- **The five-minute cache cut the Opus arm's cost 12%** ($0.3249 to
  $0.2853 on the panel, $0.2273 to $0.2008 on the new tasks), and 20% to
  27% on the short tasks. The new check directions made it slower,
  though: `build-cython-ext` took 111.9 seconds against 74.4, with 8.7
  turns against 6.3.
- **Jev-probe v2 → lean Opus with the five-minute cache is the best Opus
  configuration**: 24 of 24, $0.2433 and 133.8 seconds on the panel,
  $0.1849 and 57.8 seconds on the new tasks. Against Opus 5.5 alone that
  is 63% cheaper and 32% faster on the panel, and 57% cheaper and 48%
  faster on the new tasks. Against v2 on the one-hour cache it costs 25%
  and 19% less; its panel time is 13 seconds higher, all of it on
  `build-cython-ext`, within that task's spread.
- **No arm beats both winners yet.** The Luna arms are the cheap ones
  and the Opus arms the fast ones; no configuration is both below $0.0219
  and below 125.1 seconds on the panel.

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
| 4 | **Lean Jev-brief → Opus 5.5** (repetition 3) | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.0502 | 15.4 s |
| 5 | **Lean Jev-brief → Opus 5.5** (repetition 2) | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.0530 | 17.8 s |
| 6 | **Coder One**, no Jev | Gemini 3.8 Flash | 1.0 | $0.0557 | 71.9 s |
| 7 | **Coder One** | Gemini 3.8 Flash + Jev | 1.0 | $0.0604 | 73.8 s |
| 8 | **Jev-brief → Opus 5.5** (repetition 2) | Jev survey, then Opus 5.5 | 1.0 | $0.0614 | 16.2 s |
| 9 | **Coder One deep** (Jev survey) | Gemini 3.8 Flash + Jev | 1.0 | $0.0625 | 75.5 s |
| 10 | **Jev-brief → Opus 5.5** (repetition 3) | Jev survey, then Opus 5.5 | 1.0 | $0.0653 | 19.6 s |
| 11 | **Coder One v2** (cache-stable prompt) | Gemini 3.8 Flash + Jev | 1.0 | $0.0668 | 77.0 s |
| 12 | **Lean Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.0701 | 15.5 s |
| 13 | Claude Code 2.1.280 (repetition 3) | Opus 5.5 | 1.0 | $0.0819 | 22.8 s |
| 14 | **Coder One → Opus 5.5** (explore 2) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1023 | 26.0 s |
| 15 | **Coder One → Opus 5.5** (explore 8) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1055 | 47.4 s |
| 16 | Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.1056‡ | 64.8 s |
| 17 | **Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 | 1.0 | $0.1077 | 20.5 s |
| 18 | **Coder One → Opus 5.5** (`auto`) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1408 | 70.3 s |
| 19 | Claude Code 2.1.280 (repetition 2) | Opus 5.5 | 1.0 | $0.1408 | 23.9 s |
| 20 | Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.1420 | 23.8 s |
| 21 | Claude Code 2.1.278 | Sonnet 4.5 | 1.0 | $0.1463 | 42.6 s† |
| 22 | Codex 0.155.1 | GPT-6 Astra | 1.0 | $0.2370‡ | 47.8 s |
| 23 | Codex 0.153.3 | GPT-6 Astra | 1.0 | $0.2607 | 53.3 s |
| 24 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $0.3630 | 30.5 s† |
| — | Devin 3000.11.1 | swe-2-high | 1.0 | — | — |
| — | Devin 3000.11.1 | claude-sonnet-5-high | 0.0 (provider refusal) | — | — |

### `build-cython-ext`, cheapest first

| Rank | Arm | Model | Reward | Cost | Agent time |
| --- | --- | --- | --- | --- | --- |
| 1 | Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0174‡ | 204.7 s |
| 2 | **Jev-brief → GPT-6 Luna** | Jev survey, then GPT-6 Luna | 1.0 | $0.0199 | 209.6 s |
| 3 | **Coder One → GPT-6 Luna** (explore 8) | Gemini + Jev, then GPT-6 Luna | 1.0 | $0.0477 | 221.7 s |
| 4 | **Coder One v2** (cache-stable prompt) | Gemini 3.8 Flash + Jev | 1.0 | $0.1597§ | 237.2 s |
| 5 | **Lean Jev-brief → Opus 5.5** (repetition 3) | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.1911 | 82.7 s |
| 6 | **Coder One**, no Jev | Gemini 3.8 Flash | 1.0 | $0.2069 | 245.5 s |
| 7 | **Jev-brief → Opus 5.5** (repetition 3) | Jev survey, then Opus 5.5 | 1.0 | $0.2712 | 96.9 s |
| 8 | **Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 | 1.0 | $0.2724 | 87.1 s |
| 9 | **Lean Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.2856 | 98.1 s |
| 10 | Claude Code 2.1.280 (repetition 2) | Opus 5.5 | 1.0 | $0.3011 | 100.1 s |
| 11 | **Jev-brief → Opus 5.5** (repetition 2) | Jev survey, then Opus 5.5 | 1.0 | $0.3038 | 105.0 s |
| 12 | **Coder One deep** (Jev survey) | Gemini 3.8 Flash + Jev | 1.0 | $0.3155 | 292.3 s |
| 13 | Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.3195‡ | 357.7 s |
| 14 | Claude Code 2.1.280 (repetition 3) | Opus 5.5 | 1.0 | $0.3261 | 120.5 s |
| 15 | **Lean Jev-brief → Opus 5.5** (repetition 2) | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.3736 | 98.8 s |
| 16 | **Coder One** | Gemini 3.8 Flash + Jev | 1.0 | $0.3748 | 338.4 s |
| 17 | **Coder One → Opus 5.5** (explore 8) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.3905 | 128.9 s |
| 18 | **Coder One → Opus 5.5** (`auto`) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.4145 | 153.5 s |
| 19 | Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.4173 | 114.9 s |
| 20 | **Coder One → Opus 5.5** (explore 2) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.4681 | 138.3 s |
| 21 | Claude Code 2.1.278 | Sonnet 4.5 | 0.0 | $0.9172 | 263.7 s† |
| 22 | Codex 0.155.1 | GPT-6 Astra | 1.0 | $1.2447‡ | 223.6 s |
| 23 | Codex 0.153.3 | GPT-6 Astra | 1.0 | $1.4277 | 222.2 s† |
| 24 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $1.4420 | 201.8 s† |
| — | Devin 3000.11.1 | swe-2-high | 1.0 | — | — |

### `headless-terminal`, cheapest first

| Rank | Arm | Model | Reward | Cost | Agent time |
| --- | --- | --- | --- | --- | --- |
| 1 | Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0032‡ | 67.7 s |
| 2 | **Jev-brief → GPT-6 Luna** | Jev survey, then GPT-6 Luna | 1.0 | $0.0036 | 73.6 s |
| 3 | **Coder One → GPT-6 Luna** (explore 8) | Gemini + Jev, then GPT-6 Luna | 1.0 | $0.0202 | 95.5 s |
| 4 | **Lean Jev-brief → Opus 5.5** (repetition 2) | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.1048 | 38.7 s |
| 5 | **Jev-brief → Opus 5.5** (repetition 2) | Jev survey, then Opus 5.5 | 1.0 | $0.1091 | 49.3 s |
| 6 | **Lean Jev-brief → Opus 5.5** (repetition 3) | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.1115 | 43.9 s |
| 7 | **Lean Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.1176 | 45.3 s |
| 8 | Claude Code 2.1.280 (repetition 3) | Opus 5.5 | 1.0 | $0.1283 | 50.7 s |
| 9 | **Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 | 1.0 | $0.1323 | 35.6 s |
| 10 | **Jev-brief → Opus 5.5** (repetition 3) | Jev survey, then Opus 5.5 | 1.0 | $0.1404 | 50.3 s |
| 11 | Claude Code 2.1.280 (repetition 2) | Opus 5.5 | 1.0 | $0.1539 | 51.9 s |
| 12 | Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.1547 | 44.8 s |
| 13 | **Coder One → Opus 5.5** (explore 2) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1564 | 50.2 s |
| 14 | **Coder One → Opus 5.5** (explore 8) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1610 | 68.4 s |
| 15 | Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.2093‡ | 216.2 s |
| 16 | **Coder One**, no Jev | Gemini 3.8 Flash | 1.0 | $0.2950 | 361.8 s |
| 17 | **Coder One** | Gemini 3.8 Flash + Jev | 1.0 | $0.3149 | 598.1 s |
| 18 | **Coder One deep** (Jev survey) | Gemini 3.8 Flash + Jev | 1.0 | $0.3247 | 471.7 s |
| 19 | Codex 0.153.3 | GPT-6 Astra | 0.0 | $0.4597 | 172.7 s† |
| 20 | Codex 0.155.1 | GPT-6 Astra | 1.0 | $0.4843‡ | 185.6 s |
| 21 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $1.2035 | 166.4 s† |
| — | Devin 3000.11.1 | swe-2-high | No result | — | — |

### `fix-code-vulnerability`, cheapest first

| Rank | Arm | Model | Reward | Cost | Agent time |
| --- | --- | --- | --- | --- | --- |
| 1 | **Jev-brief → GPT-6 Luna** | Jev survey, then GPT-6 Luna | 1.0 | $0.0052 | 36.6 s |
| 2 | Codex 0.155.1 | GPT-6 Luna | 1.0 | $0.0079‡ | 49.7 s |
| 3 | **Lean Jev-brief → Opus 5.5** (repetition 2) | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.0248 | 11.9 s |
| 4 | Claude Code 2.1.280 (repetition 2) | Opus 5.5 | 1.0 | $0.0253 | 11.0 s |
| 5 | Claude Code 2.1.280 (repetition 3) | Opus 5.5 | 1.0 | $0.0255 | 11.1 s |
| 6 | **Lean Jev-brief → Opus 5.5** (repetition 3) | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.0261 | 11.5 s |
| 7 | **Jev-brief → Opus 5.5** (repetition 3) | Jev survey, then Opus 5.5 | 1.0 | $0.0291 | 11.8 s |
| 8 | **Coder One → GPT-6 Luna** (explore 8) | Gemini + Jev, then GPT-6 Luna | 1.0 | $0.0420 | 127.0 s |
| 9 | **Lean Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 (six tools) | 1.0 | $0.0576 | 13.3 s |
| 10 | Claude Code 2.1.280 | Opus 5.5 | 1.0 | $0.0693 | 12.2 s |
| 11 | **Jev-brief → Opus 5.5** (repetition 2) | Jev survey, then Opus 5.5 | 1.0 | $0.0840 | 11.8 s |
| 12 | **Jev-brief → Opus 5.5** | Jev survey, then Opus 5.5 | 1.0 | $0.0851 | 12.6 s |
| 13 | Codex 0.155.1 | GPT-6 Sol | 1.0 | $0.0877‡ | 51.6 s |
| 14 | **Coder One → Opus 5.5** (explore 8) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1136 | 73.4 s |
| 15 | **Coder One → Opus 5.5** (explore 2) | Gemini + Jev, then Opus 5.5 | 1.0 | $0.1246 | 50.6 s |
| 16 | **Coder One deep** (Jev survey) | Gemini 3.8 Flash + Jev | 0.0 | $0.2281§ | 419.7 s |
| 17 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $0.2333 | 24.7 s† |
| 18 | **Coder One** | Gemini 3.8 Flash + Jev | timeout | $0.2511 | 900.0 s |
| 19 | **Coder One**, no Jev | Gemini 3.8 Flash | 1.0 | $0.4119§ | 797.8 s |
| 20 | Codex 0.153.3 | GPT-6 Astra | 1.0 | $0.5094 | 44.2 s† |
| 21 | Codex 0.155.1 | GPT-6 Astra | 1.0 | $0.5293‡ | 67.0 s |
| — | Devin 3000.11.1 | swe-2-high | 1.0 | — | — |

### `cancel-async-tasks`, cheapest first

| Rank | Arm | Model | Reward | Cost | Agent time |
| --- | --- | --- | --- | --- | --- |
| 1 | Codex 0.153.3 | GPT-6 Astra | 1.0 | $0.1781 | 54.2 s† |
| 2 | Claude Code 2.1.278 | Fable 5.1 | 1.0 | $0.5988 | 92.8 s† |
| — | Devin 3000.11.1 | swe-2-high | No result | — | — |

### The `extended` tasks, cheapest first

Arm means over three trials each, from the Jev-probe runs above.

#### `cancel-async-tasks`, arm means, cheapest first

| Rank | Arm | Passed | Mean cost | Mean agent time |
| --- | --- | --- | --- | --- |
| 1 | Jev-probe → Luna‡ | 0/3 | $0.0012 | 24.1 s |
| 2 | **Jev-probe v3 → Luna**‡ | 2/3 | $0.0015 | 35.9 s |
| 3 | **Jev-probe v2 → Luna**‡ | 1/3 | $0.0016 | 37.1 s |
| 4 | Codex 0.155.1 / GPT-6 Luna‡ | 1/3 | $0.0016 | 37.9 s |
| 5 | **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | 3/3 | $0.0453 | 16.6 s |
| 6 | **Jev-probe v2 → lean Opus 5.5, low effort** | 3/3 | $0.0487 | 15.2 s |
| 7 | **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | 3/3 | $0.0489 | 17.7 s |
| 8 | Jev-probe → lean Opus 5.5, low effort | 3/3 | $0.0613 | 20.9 s |
| 9 | Claude Code 2.1.280 / Opus 5.5 | 3/3 | $0.1833 | 51.5 s |

#### `git-leak-recovery`, arm means, cheapest first

| Rank | Arm | Passed | Mean cost | Mean agent time |
| --- | --- | --- | --- | --- |
| 1 | **Jev-probe v3 → Luna**‡ | 3/3 | $0.0015 | 26.6 s |
| 2 | **Jev-probe v2 → Luna**‡ | 3/3 | $0.0016 | 22.9 s |
| 3 | Jev-probe → Luna‡ | 3/3 | $0.0019 | 27.8 s |
| 4 | Codex 0.155.1 / GPT-6 Luna‡ | 2/3 | $0.0028 | 58.4 s |
| 5 | **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | 3/3 | $0.0276 | 10.3 s |
| 6 | **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | 3/3 | $0.0277 | 10.6 s |
| 7 | **Jev-probe v2 → lean Opus 5.5, low effort** | 3/3 | $0.0348 | 10.0 s |
| 8 | Jev-probe → lean Opus 5.5, low effort | 3/3 | $0.0370 | 11.6 s |
| 9 | Claude Code 2.1.280 / Opus 5.5 | 3/3 | $0.0749 | 19.5 s |

#### `log-summary-date-ranges`, arm means, cheapest first

| Rank | Arm | Passed | Mean cost | Mean agent time |
| --- | --- | --- | --- | --- |
| 1 | Codex 0.155.1 / GPT-6 Luna‡ | 3/3 | $0.0016 | 26.3 s |
| 2 | **Jev-probe v3 → Luna**‡ | 0/3 | $0.0019 | 25.4 s |
| 3 | **Jev-probe v2 → Luna**‡ | 1/3 | $0.0022 | 27.8 s |
| 4 | Jev-probe → Luna‡ | 0/3 | $0.0027 | 24.6 s |
| 5 | **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | 3/3 | $0.0672 | 14.3 s |
| 6 | **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | 3/3 | $0.0710 | 19.8 s |
| 7 | **Jev-probe v2 → lean Opus 5.5, low effort** | 3/3 | $0.0882 | 13.6 s |
| 8 | Jev-probe → lean Opus 5.5, low effort | 3/3 | $0.0916 | 14.8 s |
| 9 | Claude Code 2.1.280 / Opus 5.5 | 3/3 | $0.0945 | 19.4 s |

#### `sqlite-db-truncate`, arm means, cheapest first

| Rank | Arm | Passed | Mean cost | Mean agent time |
| --- | --- | --- | --- | --- |
| 1 | Codex 0.155.1 / GPT-6 Luna‡ | 3/3 | $0.0035 | 67.9 s |
| 2 | **Jev-probe v3 → Luna**‡ | 3/3 | $0.0039 | 57.2 s |
| 3 | **Jev-probe v2 → Luna**‡ | 3/3 | $0.0039 | 57.0 s |
| 4 | Jev-probe → Luna‡ | 3/3 | $0.0042 | 43.5 s |
| 5 | **Jev-probe v2 → lean Opus 5.5, low effort, 5-minute cache** | 3/3 | $0.0447 | 16.2 s |
| 6 | **Jev-probe v3 → lean Opus 5.5, low effort, 5-minute cache** | 3/3 | $0.0532 | 19.5 s |
| 7 | **Jev-probe v2 → lean Opus 5.5, low effort** | 3/3 | $0.0556 | 18.4 s |
| 8 | Jev-probe → lean Opus 5.5, low effort | 3/3 | $0.0583 | 17.8 s |
| 9 | Claude Code 2.1.280 / Opus 5.5 | 3/3 | $0.0781 | 20.1 s |

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

### Claude pricing

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
rate + cache reads × the read rate + output × the output rate. Claude Code
caches for one hour. For example, Opus 5.5 on `headless-terminal`:

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

- Twelve `coder-one-jevprobe2-opus-lean-low-5m` attempts and four
  `coder-one-jevprobe3-*` attempts ended in `AgentSetupTimeoutError` with
  eight trials installing Claude Code at once. The runner retried each once;
  the three that timed out twice were rerun three at a time. None are
  results; the attempts are under `failed/`.

- Three `extended` trials on 2026-09-22 (`claude-code-opus` on
  `cancel-async-tasks` and `sqlite-db-truncate`, and
  `coder-one-jevprobe-opus-lean-low` on `cancel-async-tasks`) ended in
  `AgentSetupTimeoutError`: sixteen trials installed their agents at once
  and passed Harbor's 360-second setup limit. They are not results; they
  were moved to `failed/` and rerun with eight trials at a time.

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
