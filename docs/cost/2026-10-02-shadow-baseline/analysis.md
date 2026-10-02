## Per arm

| Arm | n | Passed (Wilson 95%) | Total cost | Median cost/run | Total wall | Median wall/run | Median input tokens | Unpriced runs |
|---|---:|---|---:|---:|---:|---:|---:|---:|
| `raw-claude` | 21 | 21/21 (85%–100%) | $6.24 | $0.238 | 24.3 min | 45 s | 194,547 | 0 |
| `routed-claude-on` | 21 | 21/21 (85%–100%) | $10.52 | $0.285 | 33.9 min | 64 s | 52,452 | 0 |
| `routed-claude-off` | 21 | 20/21 (77%–99%) | $10.00 | $0.321 | 35.7 min | 84 s | 51,390 | 0 |
| `routed-codex-on` | 21 | 21/21 (85%–100%) | $5.45 | $0.119 | 63.6 min | 124 s | 58,564 | 0 |
| `routed-codex-off` | 21 | 21/21 (85%–100%) | $3.38 | $0.104 | 44.6 min | 99 s | 43,621 | 0 |

## Against raw Claude Code (sum over tasks of per-task means; 95% bootstrap over trials within task)

| Arm | Tasks | Cost ratio (CI) | Cost saving | Wall ratio (CI) | Time saving |
|---|---:|---|---:|---|---:|
| `routed-claude-on` | 7 | 1.68 (1.46–1.95) | -68% | 1.39 (1.02–1.89) | -39% |
| `routed-claude-off` | 7 | 1.60 (1.44–1.75) | -60% | 1.47 (1.15–1.87) | -47% |
| `routed-codex-on` | 7 | 0.87 (0.79–0.96) | +13% | 2.61 (2.05–3.31) | -161% |
| `routed-codex-off` | 7 | 0.54 (0.51–0.57) | +46% | 1.83 (1.41–2.37) | -83% |

## Recipe on against off, same engine

| Engine | Tasks | Cost ratio on/off (CI) | Wall ratio on/off (CI) |
|---|---:|---|---|
| claude | 7 | 1.05 (0.88–1.25) | 0.95 (0.77–1.18) |
| codex | 7 | 1.61 (1.45–1.79) | 1.43 (1.23–1.66) |

## Per task (passes/n · median cost · median wall)

| Task | `raw-claude` | `routed-claude-on` | `routed-claude-off` | `routed-codex-on` | `routed-codex-off` |
|---|---|---|---|---|---|
| `fix-git` | 3/3 · $0.167 · 17 s | 3/3 · $0.259 · 46 s | 3/3 · $0.222 · 81 s | 3/3 · $0.086 · 81 s | 3/3 · $0.048 · 68 s |
| `fix-code-vulnerability` | 3/3 · $0.186 · 21 s | 3/3 · $0.193 · 42 s | 3/3 · $0.214 · 32 s | 3/3 · $0.158 · 103 s | 3/3 · $0.104 · 114 s |
| `headless-terminal` | 3/3 · $0.303 · 78 s | 3/3 · $0.477 · 107 s | 2/3 · $0.568 · 168 s | 3/3 · $0.097 · 194 s | 3/3 · $0.110 · 146 s |
| `build-cython-ext` | 3/3 · $0.661 · 172 s | 3/3 · $1.353 · 227 s | 3/3 · $1.595 · 218 s | 3/3 · $0.971 · 475 s | 3/3 · $0.575 · 239 s |
| `mi-seekable` | 3/3 · $0.228 · 40 s | 3/3 · $0.265 · 55 s | 3/3 · $0.222 · 64 s | 3/3 · $0.119 · 152 s | 3/3 · $0.067 · 90 s |
| `mi-one` | 3/3 · $0.213 · 37 s | 3/3 · $0.171 · 34 s | 3/3 · $0.123 · 41 s | 3/3 · $0.098 · 114 s | 3/3 · $0.066 · 85 s |
| `bottle-etag` | 3/3 · $0.357 · 57 s | 3/3 · $0.618 · 99 s | 3/3 · $0.484 · 92 s | 3/3 · $0.124 · 112 s | 3/3 · $0.140 · 92 s |

## Recipe on: what it did

- `routed-claude-on`: endings {'finished': 14, 'bad_replies': 4, 'checks_passed': 3}; class {'hard': 18, 'change': 3}; runs with checks kept 5/21; with knowledge kept 9/21; Jev $0.0475 of $10.52 (0.45%).
- `routed-codex-on`: endings {'finished': 17, 'checks_passed': 4}; class {'hard': 18, 'change': 3}; runs with checks kept 5/21; with knowledge kept 8/21; Jev $0.0595 of $5.45 (1.09%).

## Study spend

Runs: 105; total list-price spend $35.60; agent wall time 3.4 h.
