# hb1 (plans/haiku-briefer.json)

| Arm | Trials | Pass fix tests | Judge accepts | Accepted (both) | Total $ | $ / accepted | $ / judge-accepted | Median $ (sd) | Median s (sd) | Median turns | Same outcome all reps |
|---|---|---|---|---|---|---|---|---|---|---|---|
| hb1/A | 12 | 8 | 8 | 7 | 9.81 | 1.4 | 1.23 | 0.68 (0.345) | 156.55 (65.682) | 12.5 | 5/6 |
| hb1/B0 | 12 | 6 | 10 | 6 | 6.48 | 1.08 | 0.65 | 0.51 (0.234) | 130.85 (96.838) | 19.0 | 4/6 |
| hb1/BH | 12 | 2 | 7 | 2 | 5.22 | 2.61 | 0.75 | 0.323 (0.281) | 198.0 (59.126) | 12.5 | 4/6 |

## A vs B0

Issues: 6; trials: 36; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (B0 vs A) | A | B0 | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.583 | 0.500 | -0.083 | [-0.333, +0.167] |
| fix tests pass | 0.667 | 0.500 | -0.167 | [-0.333, +0.000] |
| judge accepts | 0.667 | 0.833 | +0.167 | [+0.000, +0.500] |

| Cost (list price, as the CLI reports) | A | B0 |
|---|---|---|
| $ per accepted (known costs) | 1.40 | 1.08 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | +22.9% (95% CI [-33.8%, +59.8%]) |

| Time (s) | A | B0 |
|---|---|---|
| median | 159.6 | 140.4 |
| p90 | 263.9 | 213.2 |

| Issue | A accepted | B0 accepted | A mean $ | B0 mean $ | A mean s | B0 mean s |
|---|---|---|---|---|---|---|
| #10074 | 2/2 | 2/2 | 0.44 | 0.31 | 105 | 95 |
| #10181 | 0/2 | 1/2 | 1.22 | 0.84 | 183 | 177 |
| #10201 | 2/2 | 2/2 | 0.88 | 0.76 | 167 | 167 |
| #10289 | 1/2 | 0/2 | 0.67 | 0.25 | 101 | 80 |
| #10301 | 0/2 | 0/2 | 1.19 | 0.67 | 232 | 121 |
| #10986 | 2/2 | 1/2 | 0.50 | 0.41 | 231 | 313 |

## B0 vs BH

Issues: 6; trials: 36; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (BH vs B0) | B0 | BH | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.500 | 0.167 | -0.333 | [-0.667, -0.083] |
| fix tests pass | 0.500 | 0.167 | -0.333 | [-0.667, -0.083] |
| judge accepts | 0.833 | 0.583 | -0.250 | [-0.417, -0.083] |

| Cost (list price, as the CLI reports) | B0 | BH |
|---|---|---|
| $ per accepted (known costs) | 1.08 | 2.61 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | -141.6% (95% CI [-441.8%, -21.8%]) |

| Time (s) | B0 | BH |
|---|---|---|
| median | 140.4 | 204.9 |
| p90 | 213.2 | 236.3 |

| Issue | B0 accepted | BH accepted | B0 mean $ | BH mean $ | B0 mean s | BH mean s |
|---|---|---|---|---|---|---|
| #10074 | 2/2 | 0/2 | 0.31 | 0.14 | 95 | 105 |
| #10181 | 1/2 | 1/2 | 0.84 | 0.84 | 177 | 268 |
| #10201 | 2/2 | 1/2 | 0.76 | 0.59 | 167 | 214 |
| #10289 | 0/2 | 0/2 | 0.25 | 0.21 | 80 | 176 |
| #10301 | 0/2 | 0/2 | 0.67 | 0.61 | 121 | 178 |
| #10986 | 1/2 | 0/2 | 0.41 | 0.22 | 313 | 200 |

## A vs BH

Issues: 6; trials: 36; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (BH vs A) | A | BH | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.583 | 0.167 | -0.417 | [-0.833, +0.000] |
| fix tests pass | 0.667 | 0.167 | -0.500 | [-0.833, -0.167] |
| judge accepts | 0.667 | 0.583 | -0.083 | [-0.333, +0.167] |

| Cost (list price, as the CLI reports) | A | BH |
|---|---|---|
| $ per accepted (known costs) | 1.40 | 2.61 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | -86.4% (95% CI [-373.6%, +42.7%]) |

| Time (s) | A | BH |
|---|---|---|
| median | 159.6 | 204.9 |
| p90 | 263.9 | 236.3 |

| Issue | A accepted | BH accepted | A mean $ | BH mean $ | A mean s | BH mean s |
|---|---|---|---|---|---|---|
| #10074 | 2/2 | 0/2 | 0.44 | 0.14 | 105 | 105 |
| #10181 | 0/2 | 1/2 | 1.22 | 0.84 | 183 | 268 |
| #10201 | 2/2 | 1/2 | 0.88 | 0.59 | 167 | 214 |
| #10289 | 1/2 | 0/2 | 0.67 | 0.21 | 101 | 176 |
| #10301 | 0/2 | 0/2 | 1.19 | 0.61 | 232 | 178 |
| #10986 | 2/2 | 0/2 | 0.50 | 0.22 | 231 | 200 |
