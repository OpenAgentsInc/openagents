# Terminal-Bench 2.0 — Khala black-box runner, replication, and path-to-beat (#6253)

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Status: **measured baseline + replication methodology + path-to-beat**, as of
2026-06-26. The measured values below are a **historical snapshot**, not current
liveness evidence. This subtree is an **isolated, Terminal-Bench-2.0-specific** lane.
It consumes the **public Khala API as a black box** and does **not** touch the
gateway / GLM-serving / Pylon code, the shared Gym/eval harness, the gym
leaderboard, or the Fireworks/Vertex sweep.

What's here:

- `run-khala-tb2.sh` — a Terminal-Bench 2.0 (Harbor) runner pointed at the
  public Khala endpoint (`https://openagents.com/api/v1`, model
  `openagents/khala`), with a free key from `POST /api/keys/free`. Produces a
  **public-safe** aggregate summary (counts + per-task reward + token totals).
  No prompts/responses/trajectories are emitted or committed.
- `tasks.bounded-subset.txt` — a fixed, deterministic 8-task slice of the
  official 89-task set for a bounded local run that cannot collide with the
  owner-armed full run on Hydralisk.
- `replication-and-path-to-beat.md` — the full writeup: (a) how to replicate
  REAP's claimed **69.1%**, (b) Khala's **measured** baseline, (c) the concrete
  **path to beat** it.

## The claim under test

`0xSero/GLM-5.2-504B` (Z.ai GLM-5.2, REAP-pruned keep-168, NVFP4, MIT) is
reported at **69.1% on Terminal-Bench 2.0** — claimed as the highest TB-2.0
score for a model that fits on 4× RTX PRO 6000. Owner-armed Hydralisk
replication lanes are the evidence surface for this checkpoint. The public
`openagents/khala` is a router and must be measured as a black box; docs and
runner output must not assert its current backing model/lane without fresh
owner-armed evidence. The question is whether we can (a) replicate 69.1%
honestly over the official 89-task denominator, (b) understand how
serving/inference method moves the number, and (c) **beat it with the Khala
orchestrator**.

## Historical snapshot (honest, as measured on 2026-06-26)

Two sources of truth, kept strictly separate:

1. **Owner-armed full-89 Harbor runs on Hydralisk** (decision-grade target;
   then-visible in `/api/public/gym/run-progress`). As of 2026-06-26 12:53 UTC:
   - **Raw GLM-5.2-REAP baseline** (`openagents/glm-5.2-reap-504b`, TP4, MTP-2
     speculative decoding, rep-penalty 1.05): **partial 19/89 completed, 10
     passed → 52.6% over completed so far** (6 errored). It was still running; trending
     toward the ~69% claim region but **not yet a complete decision-grade
     number**.
   - **Khala public route** (`openagents/khala`, heuristic public router):
     **88/89 completed, 22 passed → 25% pass-rate over completed**, with **57
     errored** trials. In that snapshot, the public route was **worse** than raw GLM —
     dominated by serving/tool-calling *errors* (Phase 0), not model quality.
2. **This subtree's isolated black-box probe** against the public Khala API —
   a bounded subset, used to independently confirm the end-to-end path works
   and to localize where it breaks. See `replication-and-path-to-beat.md` for
   the measured per-task results.

Neither number is presented as the final 69.1% replication or as current route
state. The decision-grade claim requires fresh **owner-armed full-89 GLM-REAP
evidence** over the official denominator.

## What the 2026-06-26 public Khala route snapshot showed

A black-box probe of the public endpoint on 2026-06-26 showed the **open lane
split routing by request shape**:

- A plain completion routed to `served_model: openagents/glm-5.2-reap-504b`
  (`supply_lane: hydralisk`).
- A **tool-bearing** completion routed to
  `served_model: accounts/fireworks/models/deepseek-v4-flash`
  (`supply_lane: fireworks`).

So that dated public `openagents/khala` snapshot was **not** a pure GLM-REAP run
for tool-bearing Terminal-Bench traffic. Treat this as historical evidence about
the 2026-06-26 route, not as a current serving claim. The durable finding is
that the measured TB-2.0 baseline was serving-limited before it could become an
orchestration claim.

## Run it

```sh
# bounded, isolated, public-safe (default 8-task subset)
docs/inference/terminal-bench-2/run-khala-tb2.sh \
  --tasks-file docs/inference/terminal-bench-2/tasks.bounded-subset.txt \
  --concurrent 2

# a single named task
docs/inference/terminal-bench-2/run-khala-tb2.sh --include fix-git

# the full official set (owner-armed capacity only; do not run until the owner
# explicitly arms a decision-grade run and confirms it will not collide)
docs/inference/terminal-bench-2/run-khala-tb2.sh --all --concurrent 4
```

Requires `harbor`, a running `docker`, `curl`, `python3`. The summary lands in
`last-run-summary.json` (public-safe; safe to inspect, not auto-committed).

## Boundary / non-goals

- Does NOT edit gateway, GLM-serving, Pylon, tool-call-parser, or fallback code.
- Does NOT edit the shared Gym/eval harness, the gym leaderboard ladder, or the
  Fireworks/Vertex sweep.
- Does NOT touch the owner-armed Hydralisk full-89 run.
- Consumes Khala strictly as a black box over the public API.

## Refs

- Issue #6253; master roadmap `docs/khala/2026-06-26-khala-open-issues-master-roadmap.md` (Phase 4).
- `docs/gym/2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md` (Harbor/Gym seam).
- `docs/gym/2026-06-25-khala-terminal-bench-through-openagents-run.md` (#6272 path smoke).
- `docs/inference/2026-06-25-khala-glm-52-reap-backing-lane.md`,
  `docs/inference/2026-06-25-glm-5.2-reap-504b-serving-audit.md`.
- Phase 0 serving deps: #6310 (GLM tool-calling), #6319 (fallback chain),
  #6323 (NVFP4 full-GLM pilot as a candidate tool-calling fix).
