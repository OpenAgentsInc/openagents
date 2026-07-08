# Tassadar Run × Sakana Coordinator: Integration

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


*Analysis — 2026-06-22. Whether and how a Sakana-style learned coordinator
combines with the live Tassadar run. State grounded in the promise registry
(`/api/public/product-promises`,
`docs/promises/2026-06-20-verification-class-sampling-policy.md`,
`docs/promises/2026-06-18-training-monday-real-settlement-gate-met.md`).*

Companion to [`adapting-sakana-coordination.md`](adapting-sakana-coordination.md),
[`coordinator-as-verified-work.md`](coordinator-as-verified-work.md),
[`psionic-coordinator-roadmap.md`](psionic-coordinator-roadmap.md).

## Current run state

The run is `run.tassadar.executor.20260615`, GREEN under
`training.decentralized_training_launch.v1`. It is a **multi-class verification
registry**, not a single kernel. `training.verification_classes.v1`
(per-contribution sampling, owner-approved 2026-06-20) defines five verification
classes, each with a non-zero per-contribution sample-rate floor (aggregate-only
is deprecated as a sufficient grade):

| Class | Default sample rate | Maps to |
|---|---|---|
| `exact_trace_replay` | **1.0** | deterministic execution (the `loop_sum` kernel today) |
| `deterministic_recompute` | 0.25 of shards | tokenizer/filters/reward grading |
| `freivalds_merkle` | per-step + 0.20 | matrix work |
| `statistical_cross_check` | 0.15 | **benchmarks** |
| `seeded_replication` | 0.15 | **rollouts** |

`exact_trace_replay` is fully exercised — five distinct paid contributors, five
Verified challenges, five settlements on this run at sample rate 1.0.
`freivalds_merkle` has had a weak-device validator paid for a recheck (#4676).
**`seeded_replication` and `statistical_cross_check` are defined but not yet on
real dispatched work** — they adopt the policy the first time they run.

Settlement is native over Spark with public receipts: one 1,000-sat real Bitcoin
run-settlement to an independent contributor, backed by an independent validator
challenge (`docs/promises/2026-06-18-training-monday-real-settlement-gate-met.md`).

## Where it integrates

The run already has verification classes for **stochastic, variable-quality
work** — `seeded_replication` (rollouts) and `statistical_cross_check`
(benchmarks) — and those classes are defined but undispatched. Those are exactly
the work types a coordinator produces. So the integration is not "graft model
selection onto the deterministic `loop_sum` loop" (there is no choice to optimize
there). It is: **a learned coordinator is the natural first real dispatched work
for the rollout/benchmark verification classes the run already specifies**, with
the verdict as the reward signal.

## Three integrations

### 1. Reward oracle = the run's verification verdict (roadmap P4)

The coordinator's terminal reward is "did the assembled trajectory produce a
`Verified` verdict?" The run already produces that verdict, per contribution, on
real dispatched work, under a typed class registry. Wire the coordinator's
atomic-evaluation harness (roadmap P4) to emit a contribution into the
appropriate class and read the verdict back:

- **Deterministic / kernel-parity coordinator work** → `exact_trace_replay`
  (rate 1.0): a dense reward on every trajectory — ideal for sep-CMA-ES, which
  wants clean per-eval Bernoulli rewards.
- **Rollout-style coordinator work** (coordinate LLMs to solve a task, grade the
  answer) → `seeded_replication` (rate 0.15), or `statistical_cross_check` for
  benchmark-graded tasks.

The training reward is the **verdict**, which is proven, per-contribution, and
live. Settlement is the downstream economic layer; the coordinator can train
against verdicts without touching the payout path, and attach settlement when it
graduates to a paid labor lane (integration 3).

### 2. Kernel-optimization parity = first coordination target on the exact lane

`docs/tassadar/2026-06-19-agentic-kernel-optimization-work-definition-and-parity-protocol.md`
defines work where acceptance = throughput gain **and** output-parity verified by
replay — running on the same `exact_trace_replay` machinery the run exercises at
rate 1.0. This is the cleanest near-term coordination target with a dense,
non-gameable reward: a Conductor-style coordinator orchestrates
planner → kernel-writer → checker; the parity+throughput verdict is the reward;
and the deliverable serves Psionic's own kernel work directly. No new
verification class needed — it rides the fully-exercised one.

### 3. Coordinator as a new dispatched workload family / labor participant

Per [`coordinator-as-verified-work.md`](coordinator-as-verified-work.md),
*"coordinate models to solve task X for ≤ B sats, accepted iff the result
Verifies"* is a well-formed work definition. The run is the venue and settlement
rail. A coordinator lane would be the first real dispatched work for
`seeded_replication`/`statistical_cross_check`, lighting up classes the registry
already defines, and would render natively on the existing run visualization
(assigned → submitted → replay → verified → settled;
`docs/launch/2026-06-17-tassadar-training-run-visual-language.md`) — worker
selection and role assignment are legible on that grammar with no new visual
language. The labor-market promises already GREEN
(`labor.forum_work_requests.v1`, `labor.nostr_negotiation_market.v1`) provide a
proven settlement path to attach to.

## Reward-oracle wiring spec (ties to roadmap P4)

```
coordinator trajectory τ
  → emit contribution under verification class C
       C = exact_trace_replay        (deterministic / kernel-parity work)
       C = seeded_replication        (LLM rollout work)
       C = statistical_cross_check   (benchmark-graded work)
  → run samples a per-contribution challenge at class rate (floor > 0)
  → independent validator replays / recomputes / cross-checks
  → verdict ∈ {Verified, Rejected}
  → R(τ) = 1.0 if Verified else 0.0     ← coordinator terminal reward (P4)
  → (downstream) auto-stream settlement → sats   ← economic layer only
```

Two design consequences from the sampling policy:

- **Training wants a dense reward; sampled classes give a sparse one.** At rate
  0.15, only ~15% of `seeded_replication` contributions get a challenge, so a
  naive reward would be undefined on the rest. Options: (a) for the *training
  lane*, raise the sample rate toward 1.0 — rates are tunable config, not
  contract, with a non-zero floor; (b) accept a probabilistic but unbiased
  reward (challenge-or-skip) — fine for ES/RL in expectation, just noisier, so
  budget more evals. Start with (a) for fitness stability, fall to (b) at scale.
- **LLM nondeterminism breaks naive replay.** `seeded_replication` only
  re-verifies if the worker output is reproducible — decode at temperature 0 /
  fixed seed, or the replay won't match and everything Rejects. Pin decode
  params per worker as part of the contribution, exactly as `exact_trace_replay`
  pins the program.

## Caveats

- **`seeded_replication`/`statistical_cross_check` are undispatched.** A
  coordinator lane is their first real exercise, so expect class bring-up cost
  (validator implementation, sample-rate tuning) before the reward oracle is
  trustworthy.
- **Capability gating still applies** — the coordinator selects within the
  receipted capability-eligible worker set; it never overrides the dispatch
  gate.
- **Copy discipline.** Public claims stay scoped to proven facts (the canary
  settlement; the per-class verdict counts). A coordinator lane does not get to
  claim "the network coordinates frontier models at scale" until there are
  accepted-work receipts proving it.

## Build path (folds into the roadmap phases)

- **Roadmap P0 (offline reward)** validates the oracle against
  `exact_trace_replay` verdicts on a deterministic sub-task — dense, rate-1.0 —
  before any LLM nondeterminism enters.
- **Then** move the reward to `seeded_replication` for real LLM coordination
  work, raising the training-lane sample rate for fitness stability.
- **Kernel-parity (integration 2)** is the strongest first *live* target: dense
  exact-lane reward, serves Psionic, no new verification class.
- **Shadow-candidate ship** (roadmap P4/governance) is unchanged: the coordinator
  rides the existing promoted/candidate contract and confidence bands.

**Bottom line:** the run integrates through its **verification-class registry**,
not its current kernel workload. `exact_trace_replay` (dense, rate 1.0) is the
right reward oracle to bootstrap the coordinator offline and for kernel-parity
work; `seeded_replication`/`statistical_cross_check` (defined, undispatched) are
where a general LLM coordinator becomes the run's first stochastic dispatched
work. Train on verdicts; settlement is the downstream economic layer.
