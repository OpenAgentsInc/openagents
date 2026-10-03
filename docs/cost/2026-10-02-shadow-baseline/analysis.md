## Per arm

| Arm | n | Passed (Wilson 95%) | Total cost | Median cost/run | Total wall | Median wall/run | Median input tokens | Unpriced runs |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| `raw-claude` | 21 | 21/21 (85%–100%) | $6.24 | $0.238 | 24.3 min | 45 s | 194,547 | 0 |
| `routed-claude-on` | 21 | 21/21 (85%–100%) | $10.52 | $0.285 | 33.9 min | 64 s | 52,452 | 0 |
| `routed-claude-off` | 21 | 20/21 (77%–99%) | $10.00 | $0.321 | 35.7 min | 84 s | 51,390 | 0 |
| `routed-codex-on` | 21 | 21/21 (85%–100%) | $5.45 | $0.119 | 63.6 min | 124 s | 58,564 | 0 |
| `routed-codex-off` | 21 | 21/21 (85%–100%) | $3.38 | $0.104 | 44.6 min | 99 s | 43,621 | 0 |
| `routed-claude-lean` | 21 | 21/21 (85%–100%) | $3.79 | $0.125 | 26.5 min | 46 s | 83,428 | 0 |

## Against raw Claude Code (sum over tasks of per-task means; 95% bootstrap over trials within task)

| Arm | Tasks | Cost ratio (CI) | Cost saving | Wall ratio (CI) | Time saving |
|---|---:|---|---:|---|---:|
| `routed-claude-on` | 7 | 1.68 (1.46–1.95) | -68% | 1.39 (1.02–1.89) | -39% |
| `routed-claude-off` | 7 | 1.60 (1.44–1.75) | -60% | 1.47 (1.15–1.87) | -47% |
| `routed-codex-on` | 7 | 0.87 (0.79–0.96) | +13% | 2.61 (2.05–3.31) | -161% |
| `routed-codex-off` | 7 | 0.54 (0.51–0.57) | +46% | 1.83 (1.41–2.37) | -83% |
| `routed-claude-lean` | 7 | 0.61 (0.57–0.65) | +39% | 1.09 (0.83–1.43) | -9% |

## Recipe on against off, same engine

| Engine | Tasks | Cost ratio on/off (CI) | Wall ratio on/off (CI) |
|---|---:|---|---|
| claude | 7 | 1.05 (0.88–1.25) | 0.95 (0.77–1.18) |
| codex | 7 | 1.61 (1.45–1.79) | 1.43 (1.23–1.66) |

## Lean session against the routed Claude loop (#10246)

| Base | Tasks | Cost ratio lean/base (CI) | Wall ratio lean/base (CI) |
|---|---:|---|---|
| `routed-claude-on` | 7 | 0.36 (0.31–0.42) | 0.78 (0.61–0.99) |
| `routed-claude-off` | 7 | 0.38 (0.34–0.43) | 0.74 (0.61–0.87) |

## Per task (passes/n · median cost · median wall)

| Task | `raw-claude` | `routed-claude-on` | `routed-claude-off` | `routed-codex-on` | `routed-codex-off` | `routed-claude-lean` |
|---|---|---|---|---|---|---|
| `fix-git` | 3/3 · $0.167 · 17 s | 3/3 · $0.259 · 46 s | 3/3 · $0.222 · 81 s | 3/3 · $0.086 · 81 s | 3/3 · $0.048 · 68 s | 3/3 · $0.112 · 37 s |
| `fix-code-vulnerability` | 3/3 · $0.186 · 21 s | 3/3 · $0.193 · 42 s | 3/3 · $0.214 · 32 s | 3/3 · $0.158 · 103 s | 3/3 · $0.104 · 114 s | 3/3 · $0.088 · 33 s |
| `headless-terminal` | 3/3 · $0.303 · 78 s | 3/3 · $0.477 · 107 s | 2/3 · $0.568 · 168 s | 3/3 · $0.097 · 194 s | 3/3 · $0.110 · 146 s | 3/3 · $0.169 · 71 s |
| `build-cython-ext` | 3/3 · $0.661 · 172 s | 3/3 · $1.353 · 227 s | 3/3 · $1.595 · 218 s | 3/3 · $0.971 · 475 s | 3/3 · $0.575 · 239 s | 3/3 · $0.506 · 291 s |
| `mi-seekable` | 3/3 · $0.228 · 40 s | 3/3 · $0.265 · 55 s | 3/3 · $0.222 · 64 s | 3/3 · $0.119 · 152 s | 3/3 · $0.067 · 90 s | 3/3 · $0.109 · 41 s |
| `mi-one` | 3/3 · $0.213 · 37 s | 3/3 · $0.171 · 34 s | 3/3 · $0.123 · 41 s | 3/3 · $0.098 · 114 s | 3/3 · $0.066 · 85 s | 3/3 · $0.103 · 32 s |
| `bottle-etag` | 3/3 · $0.357 · 57 s | 3/3 · $0.618 · 99 s | 3/3 · $0.484 · 92 s | 3/3 · $0.124 · 112 s | 3/3 · $0.140 · 92 s | 3/3 · $0.237 · 69 s |

## Recipe on: what it did

- `routed-claude-on`: endings {'finished': 14, 'bad_replies': 4, 'checks_passed': 3}; class {'hard': 18, 'change': 3}; runs with checks kept 5/21; with knowledge kept 9/21; Jev $0.0475 of $10.52 (0.45%).
- `routed-codex-on`: endings {'finished': 17, 'checks_passed': 4}; class {'hard': 18, 'change': 3}; runs with checks kept 5/21; with knowledge kept 8/21; Jev $0.0595 of $5.45 (1.09%).
- `routed-claude-lean`: endings {'answered': 21}; class {'change': 21}; runs with checks kept 4/21; with knowledge kept 9/21; Jev $0.0201 of $3.79 (0.53%).

## Study spend

Runs: 126; total list-price spend $39.39; agent wall time 3.8 h.

## The Codex arms (#10250)

Produced by `codex_session.py collected.jsonl` (binaries at `fbc68cfe86`).

| Arm | n | Passed (Wilson 95%) | Total cost | Median cost/run | Total wall | Median wall/run | Median input tokens | Cache read share |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| `raw-codex` | 21 | 21/21 (85%–100%) | $2.38 | $0.069 | 36.5 min | 62 s | 117,422 | 91% |
| `routed-codex-loop` | 21 | 21/21 (85%–100%) | $1.88 | $0.058 | 36.4 min | 78 s | 48,665 | 77% |
| `routed-codex-session` | 21 | 21/21 (85%–100%) | $2.24 | $0.075 | 38.5 min | 68 s | 107,574 | 88% |

| Arm | Against | Tasks | Cost ratio (95% CI) | Wall-time ratio (95% CI) |
|---|---|---:|---|---|
| `routed-codex-loop` | `raw-codex` | 7 | 0.79 (0.72–0.86) | 1.00 (0.93–1.07) |
| `routed-codex-session` | `raw-codex` | 7 | 0.94 (0.88–1.00) | 1.06 (0.99–1.14) |
| `routed-codex-session` | `routed-codex-loop` | 7 | 1.19 (1.09–1.31) | 1.06 (0.98–1.14) |
| `routed-codex-session` | `raw-claude` | 7 | 0.36 (0.34–0.38) | 1.58 (1.24–1.97) |
| `routed-codex-session` | `routed-claude-lean` | 7 | 0.59 (0.55–0.64) | 1.45 (1.27–1.75) |

| Task | `raw-codex` | `routed-codex-loop` | `routed-codex-session` |
|---|---|---|---|
| `fix-git` | 3/3 · $0.046 · 41 s | 3/3 · $0.062 · 73 s | 3/3 · $0.075 · 63 s |
| `fix-code-vulnerability` | 3/3 · $0.122 · 67 s | 3/3 · $0.053 · 49 s | 3/3 · $0.075 · 68 s |
| `headless-terminal` | 3/3 · $0.090 · 146 s | 3/3 · $0.058 · 104 s | 3/3 · $0.067 · 129 s |
| `build-cython-ext` | 3/3 · $0.344 · 309 s | 3/3 · $0.250 · 257 s | 3/3 · $0.307 · 279 s |
| `mi-seekable` | 3/3 · $0.060 · 62 s | 3/3 · $0.055 · 93 s | 3/3 · $0.098 · 82 s |
| `mi-one` | 3/3 · $0.044 · 53 s | 3/3 · $0.040 · 58 s | 3/3 · $0.059 · 61 s |
| `bottle-etag` | 3/3 · $0.062 · 56 s | 3/3 · $0.066 · 74 s | 3/3 · $0.072 · 60 s |

- `routed-codex-loop`: endings {'finished': 16, 'checks_passed': 5}; class {'change': 21}; runs with checks kept 5/21; Jev $0.0469 of $1.88 (2.5%); 77% of 2.28 M input tokens read from cache.
- `routed-codex-session`: endings {'answered': 21}; class {'change': 21}, effort medium on all 21; runs with checks kept 6/21; Jev $0.0163 of $2.24 (0.7%); 88% of 4.17 M input tokens read from cache.
- `raw-codex`: `codex exec` on gpt-6.1-sol at medium, standard tier; 91% of 5.14 M input tokens read from cache.
- Spend on the three Codex arms: $6.50 at list price (63 runs), 1.9 h of agent wall time.
