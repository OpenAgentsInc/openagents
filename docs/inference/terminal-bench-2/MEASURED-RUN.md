# Terminal-Bench 2.0 — measured Khala black-box run (#6253)

This records the **isolated bounded measurement** produced by `run-khala-tb2.sh`
against the **public Khala API as a black box** in this environment. It is
public-safe: only aggregate counts + per-task reward + token totals. Raw Harbor
trajectories / prompts / responses stayed local in `.tmp/` and were **not**
committed.
The measurement is dated 2026-06-26 and does not assert the current backing lane
for public `openagents/khala`.

This bounded subset is **NOT** the decision-grade 89-task denominator. The
decision-grade replication number comes from the owner-armed full-89 Harbor run
on Hydralisk (see `replication-and-path-to-beat.md`). This run exists to (a)
prove the public-API → Terminal-Bench → real-verifier path works end-to-end as a
black box, and (b) localize where it does/doesn't break.

## Run

- Date: 2026-06-26
- Dataset: `terminal-bench/terminal-bench-2` — dataset resolution independently
  confirmed the official **89-task** denominator.
- Agent: `terminus-2` (Harbor's native ATIF agent).
- Model: `openagents/khala` via `https://openagents.com/api/v1` (free key from
  `POST /api/keys/free`). Demand self-tagged `internal` /
  `harbor_terminal_bench` so it does not pollute the external real-user corpus.
- Verifier: Harbor per-task `tests/test.sh` → float reward in
  `/logs/verifier/reward.txt`, executed by Harbor (`reward 1.0` = solved).
- Concurrency: 3. Total wall-clock: 3m59s.

## Result (Harbor authoritative table)

| metric | value |
|---|---:|
| trials | 3 |
| exceptions / errors | **0** |
| mean reward | **0.667** |
| solved (reward 1.0) | **2** |
| failed (reward 0.0) | 1 |
| prompt tokens | 86,086 |
| completion tokens | 7,481 |

Per task:

| task | reward | outcome |
|---|---:|---|
| `fix-git` | 1.0 | solved |
| `log-summary-date-ranges` | 1.0 | solved |
| `regex-log` | 0.0 | not solved (honest non-solve, not an infra error) |

## What this proves

1. **The public Khala API solved real Terminal-Bench 2.0 tasks end-to-end in
   this dated slice as a
   black box** — `fix-git` and `log-summary-date-ranges` both passed the executed
   verifier with reward 1.0. Tool-calling is *functional* on this path (a
   terminal coding task cannot be solved without working tool calls).
2. **Zero exceptions on this slice**, vs the then-live full-89 Khala run's 57
   errored trials. So the dated public route was *capable* of clean runs; the
   then-live run's errors concentrated in harder/heavier tasks and the Phase-0
   serving instability over a long 89-task session — i.e. the gap was **serving
   reliability**, not a fundamental inability to drive the benchmark.
3. **66.7% on a 3-task slice is not a score claim** — far too small a denominator
   to compare to 69.1%. It is a path/feasibility + error-localization signal only.

## Honesty / gating

- Decision-grade replication is **gated on owner-armed full-89 GLM-REAP
  evidence** (the 2026-06-26 snapshot was partial 19/89, 10 passed, trending
  ~50–70%).
- A decision-grade **Khala** TB-2.0 number is additionally gated on the Phase-0
  serving fixes (#6310 GLM tool-calling, #6319 fallback chain) so the full-89
  Khala run is not dominated by infra errors.
- To complete: run `run-khala-tb2.sh --all` only after explicit owner arming
  once serving is healthy, then publish the full-89 pass-rate + the
  inference-method comparison table from owner-armed per-profile runs.

## Reproduce

```sh
docs/inference/terminal-bench-2/run-khala-tb2.sh \
  --include fix-git --include log-summary-date-ranges --include regex-log \
  --concurrent 3
# summary -> docs/inference/terminal-bench-2/last-run-summary.json
```
