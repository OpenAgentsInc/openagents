# ab-kinds (plans/kinds.json)

| Arm | Trials | Pass fix tests | Judge accepts | Accepted (both) | Total $ | $ / accepted | $ / judge-accepted | Median $ (sd) | Median s (sd) | Median turns | Same outcome all reps |
|---|---|---|---|---|---|---|---|---|---|---|---|
| ab-kinds/A | 12 | 4 | 9 | 3 | 15.42 | 5.14 | 1.71 | 0.888 (0.936) | 123.55 (209.065) | 12.5 | 5/6 |
| ab-kinds/B0 | 12 | 4 | 8 | 2 | 11.32 | 5.66 | 1.41 | 0.862 (0.53) | 188.65 (155.797) | 31.0 | 6/6 |

## B0 vs A

Issues: 6; trials: 24; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (B0 vs A) | A | B0 | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.250 | 0.167 | -0.083 | [-0.250, +0.000] |
| fix tests pass | 0.333 | 0.333 | +0.000 | [+0.000, +0.000] |
| judge accepts | 0.750 | 0.667 | -0.083 | [-0.333, +0.167] |

| Cost (list price, as the CLI reports) | A | B0 |
|---|---|---|
| $ per accepted (known costs) | 5.14 | 5.66 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | -10.1% (95% CI [-33.8%, +28.8%]) |

| Time (s) | A | B0 |
|---|---|---|
| median | 169.5 | 195.5 |
| p90 | 531.2 | 448.7 |

| Issue | A accepted | B0 accepted | A mean $ | B0 mean $ | A mean s | B0 mean s |
|---|---|---|---|---|---|---|
| #10065 | 1/2 | 0/2 | 3.01 | 0.74 | 248 | 132 |
| #10257 | 0/2 | 0/2 | 0.44 | 0.35 | 46 | 68 |
| #11190 | 0/2 | 0/2 | 1.85 | 1.78 | 571 | 483 |
| #11281 | 0/2 | 0/2 | 1.22 | 0.95 | 350 | 315 |
| #11283 | 2/2 | 2/2 | 0.37 | 0.51 | 30 | 181 |
| #11284 | 0/2 | 0/2 | 0.82 | 1.34 | 77 | 167 |

## Tools

| Arm | Tool | Calls/run | Share of runs | Tokens in/call | Tokens out/call | Median s | Acted on |
|---|---|---|---|---|---|---|---|
| ab-kinds/A | Bash | 23.25 | 1.0 | 164 | 721 | 0.0 | 0.03 |
| ab-kinds/A | Edit | 0.25 | 0.25 | 298 | 37 | 0.0 | 0.0 |
| ab-kinds/A | ToolSearch | 0.17 | 0.17 | 11 | 13 | 0.0 | None |
| ab-kinds/A | Read | 0.17 | 0.08 | 28 | 13080 | 0.0 | None |
| ab-kinds/A | Write | 0.17 | 0.08 | 984 | 49 | 0.0 | 0.0 |
| ab-kinds/B0 | Edit | 12.58 | 1.0 | 204 | 36 | 0.0 | 0.73 |
| ab-kinds/B0 | Read | 8.75 | 1.0 | 34 | 1441 | 0.0 | 0.1 |
| ab-kinds/B0 | Grep | 8.17 | 1.0 | 32 | 547 | 0.0 | 0.37 |
| ab-kinds/B0 | Glob | 1.83 | 0.5 | 16 | 584 | 0.0 | 0.19 |
| ab-kinds/B0 | mcp__oa__verify | 1.83 | 1.0 | 14 | 391 | 35.1 | 0.18 |
| ab-kinds/B0 | Write | 0.58 | 0.33 | 1729 | 49 | 0.0 | 0.43 |
