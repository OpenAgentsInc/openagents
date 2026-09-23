# Terminal-Bench 4.0 leaderboard reference, 2026-09-23

[Current status](README.md) · [Local TB4 results](tb4-results.md) · [Measurement](measurement.md)

This is a retained historical snapshot, fetched at 05:18:54 UTC. It is not
a live leaderboard.

The public leaderboard on the Harbor Hub, fetched on 2026-09-23
into [`reference/tb4-leaderboard.json`](../../bench/terminal-bench/reference/tb4-leaderboard.json)
with per-task successes, trials, cost, and agent time for every row. Every
row is 330 trials: 66 tasks, five each. The runs used the Hub's hosted
Modal environments, not this host.

| Rank | Agent | Model | Effort | Passes | Accuracy | Total cost | Mean trial time |
| ---: | --- | --- | --- | ---: | ---: | ---: | ---: |
| 1 | Codex | GPT-6 Astra | max | 192/330 | 58.2% ± 2.8 | $3,267.18 | 47 min |
| 2 | Claude Code | Fable 5.1 | max | 191/330 | 57.9% ± 3.8 | $6,243.50 | 65 min |
| 2 | Codex | GPT-6 Astra | xhigh | 191/330 | 57.9% ± 2.7 | $2,350.51 | 37 min |
| 2 | Codex | GPT-6 Astra | high | 191/330 | 57.9% ± 3.0 | $2,269.42 | 35 min |
| 2 | Claude Code | Fable 5.1 | xhigh | 191/330 | 57.9% ± 3.4 | $4,872.04 | 53 min |
| 6 | Claude Code | Fable 5.1 | high | 180/330 | 54.5% ± 3.4 | $3,985.38 | 50 min |
| 7 | Codex | GPT-6 Astra | medium | 179/330 | 54.2% ± 2.7 | $1,914.80 | 31 min |
| 8 | Claude Code | Fable 5.1 | medium | 178/330 | 53.9% ± 3.4 | $2,832.90 | 42 min |
| 8 | Claude Code | Opus 5 | xhigh | 178/330 | 53.9% ± 3.2 | $6,086.22 | 75 min |
| 10 | Claude Code | Opus 5 | max | 171/330 | 51.8% ± 3.4 | $5,969.11 | 80 min |
| 11 | Codex | GPT-6 Astra | low | 167/330 | 50.6% ± 2.8 | $1,557.30 | 28 min |
| 12 | Claude Code | Opus 5 | high | 166/330 | 50.3% ± 3.7 | $4,662.27 | 64 min |
| 13 | Claude Code | Opus 5 | medium | 148/330 | 44.9% ± 3.8 | $3,191.59 | 55 min |
| 14 | Claude Code | Fable 5 | max | 147/330 | 44.5% ± 3.9 | $7,265.01 | 70 min |
| 15 | Claude Code | Fable 5.1 | low | 143/330 | 43.3% ± 3.6 | $2,358.72 | 38 min |
| 16 | Claude Code | GLM-5.3 | max | 138/330 | 41.8% ± 3.2 | $2,727.63 | 97 min |
| 17 | Grok Build | Grok 4.7 | xhigh | 124/330 | 37.6% ± 3.5 | $3,683.29 | 95 min |
| 18 | Codex | GPT-5.6 Sol | max | 123/330 | 37.3% ± 3.8 | $2,541.70 | 40 min |
| 19 | Claude Code | Opus 5 | low | 115/330 | 34.9% ± 3.9 | $2,393.88 | 49 min |
| 20 | Claude Code | Opus 4.8 | max | 78/330 | 23.6% ± 3.6 | $6,481.26 | 86 min |
| 21 | Codex | GPT-5.6 Terra | max | 71/330 | 21.5% ± 3.2 | $1,733.52 | 42 min |
| 22 | Grok Build | Grok 4.6 | high | 67/330 | 20.3% ± 3.1 | $3,591.58 | 39 min |
| 23 | mini-SWE-agent | Gemini 3.8 Flash | high | 63/330 | 19.1% ± 3.4 | $1,828.77 | 33 min |
| 24 | Codex | GPT-5.6 Luna | max | 57/330 | 17.3% ± 2.9 | $346.67 | 68 min |
| 25 | Grok Build | Grok 4.5 | high | 41/330 | 12.4% ± 2.6 | $2,094.11 | 53 min |
| 25 | Claude Code | Sonnet 5 | max | 41/330 | 12.4% ± 3.1 | $9,603.86 | 109 min |
| 27 | mini-SWE-agent | Gemini 3.7 Flash | high | 37/330 | 11.2% ± 2.5 | $1,261.87 | 28 min |

Across all 27 rows, 59 of the 66 tasks were solved at least once, and 47
were solved in five of five trials by some row. Seven tasks were never
solved: `bun-sourcemap-leak`, `cargo-flight-dispatch`,
`data-anonymization`, `foodstuff-beta-activity`, `freight-dispatch-shift`,
`glycan-ms2-elucidation`, and `ontology-kg-querying`.

Two rows don't reconcile exactly. The Opus 5 (max) row's trial list counts
173 passes against its published 171, and the per-task costs of the Opus 5
(max) and GPT-5.6 Sol (max) rows sum above their published totals ($6,060.00
against $5,969.11, and $2,598.11 against $2,541.70), because the source
job's aggregates include retried attempts the row dropped. The reference
file flags both (`per_task.consistent`, `per_task.cost_consistent`).

See the leaderboard beside this host's arms, per task:

```sh
gym terminal-bench overview --profile tb4
gym coder matrix --profile tb4
```
