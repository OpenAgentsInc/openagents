# ab-levers (plans/ablate-levers.json)

## Decision (frozen keep rule)

The rule (`plans/ablate-levers.json` "decision"): keep a lever only when its success CI's lower bound is above -0.05 AND its cost per accepted is below B0's, with the lower bound of the reduction CI above 0.

**No arm passes, so the production levers stay at B0** (`common.LEVERS` as it stands: 8 files, 140-line excerpts, 2 past changes, template v3, default effort, Opus 5.5).

- Judge 1 (frozen, Claude Opus 5.5) has a verdict on all 378 trials (49 that first hit the login's usage limit were rerun on 2026-10-12).
- Bsmall comes closest: accepted +0.048 [+0.000, +0.127] clears the success bound, and it is 15.4% cheaper per accepted change, but the reduction CI is [-4.1%, +44.8%]. Its lower bound is below 0, so it is not shown.
- Blarge: accepted +0.032 [-0.032, +0.095], cost +3.9% [-24.7%, +27.6%]. Not shown.
- Bterse: accepted +0.000 [-0.079, +0.095] fails the success bound; judge acceptance falls 9.5 points [-0.175, -0.016].
- Blow and Bsonnet are cheaper per run but lose judge acceptance by 30 and 38 points (CIs entirely below 0), and accepted by 8 and 10 points. Both fail the success bound.
- Judge 2 (Gemini 3.1 Pro on Vertex) scored all 378 trials. With it as the judge no arm saves money either: Bsmall and Blarge match B0 on accepted at +0.000 [-0.048, +0.048]; Blow and Bsonnet lose 14 points with CIs below 0. The two judges agree on 82-86% of B0 and Bsmall verdicts (kappa 0.55-0.64).
- One harness duplicate: 10201-{Blow,Bsmall,Bterse}-1 ran twice. A shard machine was restored from a partial archive that lacked them. `merge_shards.py` keeps the first run of each.

Trials without a judge-1 verdict (counted as not accepted below): 0 of 378.

| Arm | Trials | Pass fix tests | Judge accepts | Accepted (both) | Total $ | $ / accepted | $ / judge-accepted | Median $ (sd) | Median s (sd) | Median turns | Same outcome all reps |
|---|---|---|---|---|---|---|---|---|---|---|---|
| ab-levers/B0 | 63 | 18 | 46 | 15 | 38.98 | 2.6 | 0.85 | 0.672 (0.302) | 151.8 (97.006) | 22 | 17/21 |
| ab-levers/Blarge | 63 | 17 | 45 | 17 | 42.44 | 2.5 | 0.94 | 0.686 (0.313) | 152.8 (81.456) | 23 | 19/21 |
| ab-levers/Blow | 63 | 11 | 27 | 10 | 20.54 | 2.05 | 0.76 | 0.303 (0.129) | 113.9 (54.791) | 14 | 18/21 |
| ab-levers/Bsmall | 63 | 18 | 49 | 18 | 39.57 | 2.2 | 0.81 | 0.615 (0.362) | 149.4 (86.002) | 25 | 16/21 |
| ab-levers/Bsonnet | 63 | 14 | 22 | 9 | 13.08 | 1.45 | 0.59 | 0.202 (0.085) | 94.9 (90.617) | 16 | 19/21 |
| ab-levers/Bterse | 63 | 17 | 40 | 15 | 35.46 | 2.36 | 0.89 | 0.587 (0.26) | 141.6 (74.227) | 21 | 19/21 |

## Bsmall vs B0

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Bsmall vs B0) | B0 | Bsmall | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.238 | 0.286 | +0.048 | [+0.000, +0.127] |
| fix tests pass | 0.286 | 0.286 | +0.000 | [-0.048, +0.048] |
| judge accepts | 0.730 | 0.778 | +0.048 | [-0.095, +0.206] |

| Cost (list price, as the CLI reports) | B0 | Bsmall |
|---|---|---|
| $ per accepted (known costs) | 2.60 | 2.20 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | +15.4% (95% CI [-4.1%, +44.8%]) |

| Time (s) | B0 | Bsmall |
|---|---|---|
| median | 151.8 | 149.4 |
| p90 | 295.2 | 259.1 |

| Issue | B0 accepted | Bsmall accepted | B0 mean $ | Bsmall mean $ | B0 mean s | Bsmall mean s |
|---|---|---|---|---|---|---|
| #10074 | 3/3 | 3/3 | 0.18 | 0.14 | 137 | 92 |
| #10167 | 0/3 | 0/3 | 0.30 | 0.25 | 56 | 56 |
| #10179 | 0/3 | 0/3 | 0.90 | 0.85 | 199 | 217 |
| #10181 | 0/3 | 2/3 | 0.86 | 0.75 | 148 | 142 |
| #10201 | 2/3 | 2/3 | 0.81 | 0.67 | 114 | 150 |
| #10228 | 3/3 | 3/3 | 0.17 | 0.18 | 115 | 92 |
| #10248 | 0/3 | 0/3 | 0.85 | 0.82 | 183 | 195 |
| #10258 | 0/3 | 0/3 | 0.61 | 0.53 | 124 | 140 |
| #10273 | 2/3 | 2/3 | 0.67 | 0.72 | 219 | 177 |
| #10283 | 0/3 | 0/3 | 0.98 | 1.17 | 142 | 182 |
| #10289 | 0/3 | 0/3 | 0.25 | 0.24 | 141 | 190 |
| #10295 | 0/3 | 0/3 | 0.45 | 0.53 | 124 | 121 |
| #10298 | 0/3 | 0/3 | 0.78 | 0.78 | 199 | 189 |
| #10299 | 0/3 | 0/3 | 0.64 | 0.57 | 150 | 103 |
| #10301 | 1/3 | 2/3 | 0.84 | 1.05 | 242 | 257 |
| #10349 | 0/3 | 0/3 | 0.87 | 0.81 | 389 | 251 |
| #10369 | 0/3 | 0/3 | 1.14 | 1.29 | 150 | 193 |
| #10370 | 0/3 | 0/3 | 0.58 | 0.44 | 188 | 87 |
| #10893 | 3/3 | 3/3 | 0.15 | 0.15 | 243 | 248 |
| #10986 | 1/3 | 1/3 | 0.37 | 0.27 | 223 | 182 |
| #11118 | 0/3 | 0/3 | 0.60 | 0.95 | 249 | 354 |

## Blarge vs B0

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Blarge vs B0) | B0 | Blarge | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.238 | 0.270 | +0.032 | [-0.032, +0.095] |
| fix tests pass | 0.286 | 0.270 | -0.016 | [-0.063, +0.032] |
| judge accepts | 0.730 | 0.714 | -0.016 | [-0.127, +0.079] |

| Cost (list price, as the CLI reports) | B0 | Blarge |
|---|---|---|
| $ per accepted (known costs) | 2.60 | 2.50 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | +3.9% (95% CI [-24.7%, +27.6%]) |

| Time (s) | B0 | Blarge |
|---|---|---|
| median | 151.8 | 152.8 |
| p90 | 295.2 | 261.9 |

| Issue | B0 accepted | Blarge accepted | B0 mean $ | Blarge mean $ | B0 mean s | Blarge mean s |
|---|---|---|---|---|---|---|
| #10074 | 3/3 | 3/3 | 0.18 | 0.29 | 137 | 110 |
| #10167 | 0/3 | 0/3 | 0.30 | 0.38 | 56 | 54 |
| #10179 | 0/3 | 0/3 | 0.90 | 0.99 | 199 | 253 |
| #10181 | 0/3 | 1/3 | 0.86 | 0.99 | 148 | 210 |
| #10201 | 2/3 | 3/3 | 0.81 | 0.82 | 114 | 119 |
| #10228 | 3/3 | 3/3 | 0.17 | 0.27 | 115 | 103 |
| #10248 | 0/3 | 0/3 | 0.85 | 1.00 | 183 | 244 |
| #10258 | 0/3 | 0/3 | 0.61 | 0.75 | 124 | 151 |
| #10273 | 2/3 | 3/3 | 0.67 | 0.78 | 219 | 242 |
| #10283 | 0/3 | 0/3 | 0.98 | 1.02 | 142 | 159 |
| #10289 | 0/3 | 0/3 | 0.25 | 0.27 | 141 | 126 |
| #10295 | 0/3 | 0/3 | 0.45 | 0.71 | 124 | 144 |
| #10298 | 0/3 | 0/3 | 0.78 | 0.60 | 199 | 143 |
| #10299 | 0/3 | 0/3 | 0.64 | 0.91 | 150 | 171 |
| #10301 | 1/3 | 0/3 | 0.84 | 0.65 | 242 | 237 |
| #10349 | 0/3 | 0/3 | 0.87 | 0.86 | 389 | 268 |
| #10369 | 0/3 | 0/3 | 1.14 | 1.27 | 150 | 190 |
| #10370 | 0/3 | 0/3 | 0.58 | 0.60 | 188 | 144 |
| #10893 | 3/3 | 3/3 | 0.15 | 0.20 | 243 | 242 |
| #10986 | 1/3 | 1/3 | 0.37 | 0.41 | 223 | 182 |
| #11118 | 0/3 | 0/3 | 0.60 | 0.39 | 249 | 120 |

## Bterse vs B0

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Bterse vs B0) | B0 | Bterse | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.238 | 0.238 | +0.000 | [-0.079, +0.095] |
| fix tests pass | 0.286 | 0.270 | -0.016 | [-0.111, +0.095] |
| judge accepts | 0.730 | 0.635 | -0.095 | [-0.175, -0.016] |

| Cost (list price, as the CLI reports) | B0 | Bterse |
|---|---|---|
| $ per accepted (known costs) | 2.60 | 2.36 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | +9.0% (95% CI [-45.7%, +40.5%]) |

| Time (s) | B0 | Bterse |
|---|---|---|
| median | 151.8 | 141.6 |
| p90 | 295.2 | 253.8 |

| Issue | B0 accepted | Bterse accepted | B0 mean $ | Bterse mean $ | B0 mean s | Bterse mean s |
|---|---|---|---|---|---|---|
| #10074 | 3/3 | 2/3 | 0.18 | 0.22 | 137 | 99 |
| #10167 | 0/3 | 0/3 | 0.30 | 0.30 | 56 | 52 |
| #10179 | 0/3 | 0/3 | 0.90 | 0.85 | 199 | 180 |
| #10181 | 0/3 | 0/3 | 0.86 | 0.85 | 148 | 145 |
| #10201 | 2/3 | 3/3 | 0.81 | 0.69 | 114 | 113 |
| #10228 | 3/3 | 3/3 | 0.17 | 0.19 | 115 | 107 |
| #10248 | 0/3 | 0/3 | 0.85 | 0.64 | 183 | 203 |
| #10258 | 0/3 | 0/3 | 0.61 | 0.63 | 124 | 135 |
| #10273 | 2/3 | 1/3 | 0.67 | 0.64 | 219 | 160 |
| #10283 | 0/3 | 0/3 | 0.98 | 1.01 | 142 | 148 |
| #10289 | 0/3 | 0/3 | 0.25 | 0.23 | 141 | 125 |
| #10295 | 0/3 | 0/3 | 0.45 | 0.61 | 124 | 119 |
| #10298 | 0/3 | 0/3 | 0.78 | 0.60 | 199 | 159 |
| #10299 | 0/3 | 0/3 | 0.64 | 0.59 | 150 | 127 |
| #10301 | 1/3 | 0/3 | 0.84 | 0.83 | 242 | 262 |
| #10349 | 0/3 | 0/3 | 0.87 | 0.65 | 389 | 251 |
| #10369 | 0/3 | 0/3 | 1.14 | 0.91 | 150 | 158 |
| #10370 | 0/3 | 0/3 | 0.58 | 0.49 | 188 | 84 |
| #10893 | 3/3 | 3/3 | 0.15 | 0.22 | 243 | 253 |
| #10986 | 1/3 | 3/3 | 0.37 | 0.36 | 223 | 259 |
| #11118 | 0/3 | 0/3 | 0.60 | 0.31 | 249 | 208 |

## Blow vs B0

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Blow vs B0) | B0 | Blow | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.238 | 0.159 | -0.079 | [-0.175, +0.000] |
| fix tests pass | 0.286 | 0.175 | -0.111 | [-0.222, -0.032] |
| judge accepts | 0.730 | 0.429 | -0.302 | [-0.460, -0.159] |

| Cost (list price, as the CLI reports) | B0 | Blow |
|---|---|---|
| $ per accepted (known costs) | 2.60 | 2.05 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | +21.0% (95% CI [-76.6%, +49.1%]) |

| Time (s) | B0 | Blow |
|---|---|---|
| median | 151.8 | 113.9 |
| p90 | 295.2 | 180.6 |

| Issue | B0 accepted | Blow accepted | B0 mean $ | Blow mean $ | B0 mean s | Blow mean s |
|---|---|---|---|---|---|---|
| #10074 | 3/3 | 2/3 | 0.18 | 0.20 | 137 | 89 |
| #10167 | 0/3 | 0/3 | 0.30 | 0.26 | 56 | 50 |
| #10179 | 0/3 | 0/3 | 0.90 | 0.58 | 199 | 124 |
| #10181 | 0/3 | 1/3 | 0.86 | 0.42 | 148 | 83 |
| #10201 | 2/3 | 0/3 | 0.81 | 0.33 | 114 | 83 |
| #10228 | 3/3 | 3/3 | 0.17 | 0.16 | 115 | 92 |
| #10248 | 0/3 | 0/3 | 0.85 | 0.31 | 183 | 168 |
| #10258 | 0/3 | 0/3 | 0.61 | 0.37 | 124 | 108 |
| #10273 | 2/3 | 1/3 | 0.67 | 0.36 | 219 | 132 |
| #10283 | 0/3 | 0/3 | 0.98 | 0.51 | 142 | 92 |
| #10289 | 0/3 | 0/3 | 0.25 | 0.21 | 141 | 252 |
| #10295 | 0/3 | 0/3 | 0.45 | 0.26 | 124 | 117 |
| #10298 | 0/3 | 0/3 | 0.78 | 0.27 | 199 | 136 |
| #10299 | 0/3 | 0/3 | 0.64 | 0.27 | 150 | 70 |
| #10301 | 1/3 | 0/3 | 0.84 | 0.44 | 242 | 180 |
| #10349 | 0/3 | 0/3 | 0.87 | 0.32 | 389 | 110 |
| #10369 | 0/3 | 0/3 | 1.14 | 0.52 | 150 | 111 |
| #10370 | 0/3 | 0/3 | 0.58 | 0.43 | 188 | 85 |
| #10893 | 3/3 | 3/3 | 0.15 | 0.13 | 243 | 155 |
| #10986 | 1/3 | 0/3 | 0.37 | 0.25 | 223 | 170 |
| #11118 | 0/3 | 0/3 | 0.60 | 0.23 | 249 | 127 |

## Bsonnet vs B0

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Bsonnet vs B0) | B0 | Bsonnet | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.238 | 0.143 | -0.095 | [-0.238, +0.032] |
| fix tests pass | 0.286 | 0.222 | -0.063 | [-0.222, +0.063] |
| judge accepts | 0.730 | 0.349 | -0.381 | [-0.556, -0.206] |

| Cost (list price, as the CLI reports) | B0 | Bsonnet |
|---|---|---|
| $ per accepted (known costs) | 2.60 | 1.45 |
| trials with unknown cost | 0 | 0 |
| cost reduction per accepted | | +44.1% (95% CI [-79.6%, +71.8%]) |

| Time (s) | B0 | Bsonnet |
|---|---|---|
| median | 151.8 | 94.9 |
| p90 | 295.2 | 230.9 |

| Issue | B0 accepted | Bsonnet accepted | B0 mean $ | Bsonnet mean $ | B0 mean s | Bsonnet mean s |
|---|---|---|---|---|---|---|
| #10074 | 3/3 | 0/3 | 0.18 | 0.11 | 137 | 80 |
| #10167 | 0/3 | 0/3 | 0.30 | 0.15 | 56 | 43 |
| #10179 | 0/3 | 0/3 | 0.90 | 0.33 | 199 | 267 |
| #10181 | 0/3 | 1/3 | 0.86 | 0.22 | 148 | 49 |
| #10201 | 2/3 | 0/3 | 0.81 | 0.25 | 114 | 66 |
| #10228 | 3/3 | 3/3 | 0.17 | 0.08 | 115 | 85 |
| #10248 | 0/3 | 0/3 | 0.85 | 0.34 | 183 | 214 |
| #10258 | 0/3 | 0/3 | 0.61 | 0.19 | 124 | 70 |
| #10273 | 2/3 | 0/3 | 0.67 | 0.24 | 219 | 128 |
| #10283 | 0/3 | 0/3 | 0.98 | 0.25 | 142 | 56 |
| #10289 | 0/3 | 0/3 | 0.25 | 0.11 | 141 | 155 |
| #10295 | 0/3 | 0/3 | 0.45 | 0.17 | 124 | 102 |
| #10298 | 0/3 | 0/3 | 0.78 | 0.28 | 199 | 103 |
| #10299 | 0/3 | 0/3 | 0.64 | 0.21 | 150 | 118 |
| #10301 | 1/3 | 0/3 | 0.84 | 0.25 | 242 | 132 |
| #10349 | 0/3 | 0/3 | 0.87 | 0.12 | 389 | 84 |
| #10369 | 0/3 | 0/3 | 1.14 | 0.34 | 150 | 110 |
| #10370 | 0/3 | 0/3 | 0.58 | 0.21 | 188 | 68 |
| #10893 | 3/3 | 3/3 | 0.15 | 0.13 | 243 | 245 |
| #10986 | 1/3 | 2/3 | 0.37 | 0.15 | 223 | 200 |
| #11118 | 0/3 | 0/3 | 0.60 | 0.23 | 249 | 203 |

## Two judges: Bsmall vs B0

Trials with both verdicts: 378; second judge est. cost $11.96

| Arm | Trials | Judge 1 accepts | Judge 2 accepts | Both accept | Agreement | Cohen's kappa |
|---|---|---|---|---|---|---|
| B0 | 63 | 0.730 | 0.714 | 0.651 | 0.857 | 0.64 |
| Bsmall | 63 | 0.778 | 0.698 | 0.651 | 0.825 | 0.55 |

| Judge acceptance (Bsmall vs B0) | B0 | Bsmall | Difference | 95% CI | Issues |
|---|---|---|---|---|---|
| judge 1 (Claude Opus 5.5) | 0.730 | 0.778 | +0.048 | [-0.095, +0.190] | 21 |
| judge 2 (Gemini) | 0.714 | 0.698 | -0.016 | [-0.175, +0.111] | 21 |
| both judges | 0.651 | 0.651 | +0.000 | [-0.175, +0.175] | 21 |
| judge 2 and fix tests | 0.270 | 0.270 | -0.000 | [-0.048, +0.048] | 21 |

## Two judges: Blarge vs B0

Trials with both verdicts: 378; second judge est. cost $11.96

| Arm | Trials | Judge 1 accepts | Judge 2 accepts | Both accept | Agreement | Cohen's kappa |
|---|---|---|---|---|---|---|
| B0 | 63 | 0.730 | 0.714 | 0.651 | 0.857 | 0.64 |
| Blarge | 63 | 0.714 | 0.635 | 0.587 | 0.825 | 0.61 |

| Judge acceptance (Blarge vs B0) | B0 | Blarge | Difference | 95% CI | Issues |
|---|---|---|---|---|---|
| judge 1 (Claude Opus 5.5) | 0.730 | 0.714 | -0.016 | [-0.127, +0.079] | 21 |
| judge 2 (Gemini) | 0.714 | 0.635 | -0.079 | [-0.206, +0.016] | 21 |
| both judges | 0.651 | 0.587 | -0.063 | [-0.206, +0.063] | 21 |
| judge 2 and fix tests | 0.270 | 0.270 | +0.000 | [-0.048, +0.048] | 21 |

## Two judges: Bterse vs B0

Trials with both verdicts: 378; second judge est. cost $11.96

| Arm | Trials | Judge 1 accepts | Judge 2 accepts | Both accept | Agreement | Cohen's kappa |
|---|---|---|---|---|---|---|
| B0 | 63 | 0.730 | 0.714 | 0.651 | 0.857 | 0.64 |
| Bterse | 63 | 0.635 | 0.683 | 0.571 | 0.825 | 0.61 |

| Judge acceptance (Bterse vs B0) | B0 | Bterse | Difference | 95% CI | Issues |
|---|---|---|---|---|---|
| judge 1 (Claude Opus 5.5) | 0.730 | 0.635 | -0.095 | [-0.175, -0.016] | 21 |
| judge 2 (Gemini) | 0.714 | 0.683 | -0.032 | [-0.159, +0.095] | 21 |
| both judges | 0.651 | 0.571 | -0.079 | [-0.206, +0.032] | 21 |
| judge 2 and fix tests | 0.270 | 0.222 | -0.048 | [-0.143, +0.063] | 21 |

## Two judges: Blow vs B0

Trials with both verdicts: 378; second judge est. cost $11.96

| Arm | Trials | Judge 1 accepts | Judge 2 accepts | Both accept | Agreement | Cohen's kappa |
|---|---|---|---|---|---|---|
| B0 | 63 | 0.730 | 0.714 | 0.651 | 0.857 | 0.64 |
| Blow | 63 | 0.429 | 0.397 | 0.302 | 0.778 | 0.54 |

| Judge acceptance (Blow vs B0) | B0 | Blow | Difference | 95% CI | Issues |
|---|---|---|---|---|---|
| judge 1 (Claude Opus 5.5) | 0.730 | 0.429 | -0.302 | [-0.460, -0.159] | 21 |
| judge 2 (Gemini) | 0.714 | 0.397 | -0.317 | [-0.492, -0.159] | 21 |
| both judges | 0.651 | 0.302 | -0.349 | [-0.508, -0.206] | 21 |
| judge 2 and fix tests | 0.270 | 0.127 | -0.143 | [-0.270, -0.048] | 21 |

## Two judges: Bsonnet vs B0

Trials with both verdicts: 378; second judge est. cost $11.96

| Arm | Trials | Judge 1 accepts | Judge 2 accepts | Both accept | Agreement | Cohen's kappa |
|---|---|---|---|---|---|---|
| B0 | 63 | 0.730 | 0.714 | 0.651 | 0.857 | 0.64 |
| Bsonnet | 63 | 0.349 | 0.270 | 0.270 | 0.921 | 0.82 |

| Judge acceptance (Bsonnet vs B0) | B0 | Bsonnet | Difference | 95% CI | Issues |
|---|---|---|---|---|---|
| judge 1 (Claude Opus 5.5) | 0.730 | 0.349 | -0.381 | [-0.556, -0.206] | 21 |
| judge 2 (Gemini) | 0.714 | 0.270 | -0.444 | [-0.603, -0.286] | 21 |
| both judges | 0.651 | 0.270 | -0.381 | [-0.524, -0.238] | 21 |
| judge 2 and fix tests | 0.270 | 0.127 | -0.143 | [-0.302, -0.016] | 21 |

## Bsmall vs B0 with judge 2 (Gemini) as the judge

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Bsmall vs B0) | B0 | Bsmall | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.270 | 0.270 | +0.000 | [-0.048, +0.048] |
| fix tests pass | 0.286 | 0.286 | +0.000 | [-0.048, +0.048] |
| judge accepts | 0.714 | 0.698 | -0.016 | [-0.175, +0.111] |

| Cost (list price, as the CLI reports) | B0 | Bsmall |
|---|---|---|
| $ per accepted (known costs) | 2.29 | 2.33 |
| trials with unknown cost | 0 | 0 |

## Blarge vs B0 with judge 2 (Gemini) as the judge

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Blarge vs B0) | B0 | Blarge | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.270 | 0.270 | +0.000 | [-0.048, +0.048] |
| fix tests pass | 0.286 | 0.270 | -0.016 | [-0.063, +0.032] |
| judge accepts | 0.714 | 0.635 | -0.079 | [-0.206, +0.032] |

| Cost (list price, as the CLI reports) | B0 | Blarge |
|---|---|---|
| $ per accepted (known costs) | 2.29 | 2.50 |
| trials with unknown cost | 0 | 0 |

## Bterse vs B0 with judge 2 (Gemini) as the judge

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Bterse vs B0) | B0 | Bterse | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.270 | 0.222 | -0.048 | [-0.143, +0.063] |
| fix tests pass | 0.286 | 0.270 | -0.016 | [-0.111, +0.095] |
| judge accepts | 0.714 | 0.683 | -0.032 | [-0.159, +0.079] |

| Cost (list price, as the CLI reports) | B0 | Bterse |
|---|---|---|
| $ per accepted (known costs) | 2.29 | 2.53 |
| trials with unknown cost | 0 | 0 |

## Blow vs B0 with judge 2 (Gemini) as the judge

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Blow vs B0) | B0 | Blow | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.270 | 0.127 | -0.143 | [-0.270, -0.048] |
| fix tests pass | 0.286 | 0.175 | -0.111 | [-0.222, -0.032] |
| judge accepts | 0.714 | 0.397 | -0.317 | [-0.492, -0.143] |

| Cost (list price, as the CLI reports) | B0 | Blow |
|---|---|---|
| $ per accepted (known costs) | 2.29 | 2.57 |
| trials with unknown cost | 0 | 0 |

## Bsonnet vs B0 with judge 2 (Gemini) as the judge

Issues: 21; trials: 378; bootstrap: 10000 issue-level resamples, seed 11211.

| Success (Bsonnet vs B0) | B0 | Bsonnet | Difference | 95% CI |
|---|---|---|---|---|
| accepted (fix tests and judge) | 0.270 | 0.127 | -0.143 | [-0.302, -0.016] |
| fix tests pass | 0.286 | 0.222 | -0.063 | [-0.222, +0.063] |
| judge accepts | 0.714 | 0.270 | -0.444 | [-0.603, -0.286] |

| Cost (list price, as the CLI reports) | B0 | Bsonnet |
|---|---|---|
| $ per accepted (known costs) | 2.29 | 1.63 |
| trials with unknown cost | 0 | 0 |
