# ab-cold (plans/ablate-cold.json)

| Arm | Trials | Pass fix tests | Judge accepts | Accepted (both) | Total $ | $ / accepted | $ / judge-accepted | Median $ (sd) | Median s (sd) | Median turns | Same outcome all reps |
|---|---|---|---|---|---|---|---|---|---|---|---|
| ab-cold/B0 | 21 | 11 | 19 | 11 | 11.18 | 1.02 | 0.59 | 0.548 (0.302) | 116.7 (36.675) | 21 | 5/7 |
| ab-cold/Bcold | 21 | 11 | 15 | 10 | 10.28 | 1.03 | 0.69 | 0.439 (0.278) | 131.8 (40.28) | 15 | 5/7 |

## Bcold vs B0

Issues: 7; trials: 42; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Bcold vs B0) | B0 | Bcold | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.524 | 0.476 | -0.048 | [-0.190, +0.095] |
| fix tests pass | 0.524 | 0.524 | +0.000 | [-0.143, +0.143] |
| judge accepts | 0.905 | 0.714 | -0.190 | [-0.381, -0.048] |

| Cost (list price, as the CLI reports) | B0 | Bcold |
|---|---|---|
| $ per accepted (known costs) | 1.02 | 1.03 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | -1.1% (95% CI [-59.4%, +26.4%]) |

| Time (s) | B0 | Bcold |
|---|---|---|
| median | 116.7 | 131.8 |
| p90 | 156.4 | 196.2 |

| Issue | B0 accepted | Bcold accepted | B0 mean $ | Bcold mean $ | B0 mean s | Bcold mean s |
|---|---|---|---|---|---|---|
| #10074 | 3/3 | 3/3 | 0.24 | 0.29 | 64 | 102 |
| #10228 | 3/3 | 3/3 | 0.16 | 0.18 | 64 | 84 |
| #10258 | 0/3 | 0/3 | 0.65 | 0.72 | 138 | 151 |
| #10273 | 3/3 | 2/3 | 0.82 | 0.54 | 134 | 140 |
| #10295 | 0/3 | 0/3 | 0.60 | 0.45 | 113 | 126 |
| #10301 | 1/3 | 0/3 | 0.94 | 0.91 | 140 | 182 |
| #10986 | 1/3 | 2/3 | 0.33 | 0.34 | 133 | 167 |

## Tools

| Arm | Tool | Calls/run | Share of runs | Tokens in/call | Tokens out/call | Median s | Acted on |
|---|---|---|---|---|---|---|---|
| ab-cold/B0 | Read | 6.29 | 0.9 | 34 | 1334 | 0.0 | 0.0 |
| ab-cold/B0 | Grep | 5.81 | 0.95 | 28 | 609 | 0.0 | 0.44 |
| ab-cold/B0 | Edit | 5.29 | 1.0 | 309 | 34 | 0.0 | 0.53 |
| ab-cold/B0 | mcp__oa__verify | 1.05 | 1.0 | 17 | 158 | 34.0 | 0.0 |
| ab-cold/B0 | Glob | 0.14 | 0.14 | 10 | 4 | 0.0 | 0.0 |
| ab-cold/Bcold | Read | 5.48 | 0.86 | 34 | 1304 | 0.0 | 0.0 |
| ab-cold/Bcold | Edit | 4.86 | 1.0 | 318 | 34 | 0.0 | 0.51 |
| ab-cold/Bcold | Grep | 4.57 | 0.9 | 32 | 916 | 0.0 | 0.4 |
| ab-cold/Bcold | mcp__oa__verify | 1.1 | 1.0 | 14 | 171 | 52.0 | 0.04 |
| ab-cold/Bcold | Glob | 0.19 | 0.14 | 10 | 7 | 0.0 | 1.0 |
